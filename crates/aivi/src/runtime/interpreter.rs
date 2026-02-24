use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Datelike, NaiveDate, Timelike, TimeZone as ChronoTimeZone};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{default_libcall_names, Linkage, Module};
use regex::RegexBuilder;
use url::Url;

use crate::cg_type::CgType;
use crate::hir::{
    HirBlockItem, HirExpr, HirListItem, HirLiteral, HirMatchArm, HirPathSegment, HirPattern,
    HirProgram, HirRecordField, HirTextPart,
};
use crate::i18n::{parse_message_template, validate_key_text, MessagePart};
use crate::native_rust_backend::typed_cranelift;
use crate::{kernel, rust_ir};
use crate::AiviError;

mod builtins;
mod environment;
mod http;
#[cfg(test)]
mod tests;
mod values;

use self::builtins::register_builtins;
use self::environment::{Env, MachineEdge, RuntimeContext};
use self::values::{
    BuiltinImpl, BuiltinValue, ClosureValue, EffectValue, KeyValue, ResourceValue, SourceValue,
    TaggedValue, ThunkValue, Value, shape_record,
};

#[derive(Debug)]
struct CancelToken {
    local: AtomicBool,
    parent: Option<Arc<CancelToken>>,
}

impl CancelToken {
    fn root() -> Arc<Self> {
        Arc::new(Self {
            local: AtomicBool::new(false),
            parent: None,
        })
    }

    fn child(parent: Arc<CancelToken>) -> Arc<Self> {
        Arc::new(Self {
            local: AtomicBool::new(false),
            parent: Some(parent),
        })
    }

    fn cancel(&self) {
        self.local.store(true, Ordering::Release);
    }

    fn parent(&self) -> Option<Arc<CancelToken>> {
        self.parent.clone()
    }

    fn is_cancelled(&self) -> bool {
        if self.local.load(Ordering::Relaxed) {
            return true;
        }
        self.parent
            .as_ref()
            .is_some_and(|parent| parent.is_cancelled())
    }
}

struct Runtime {
    ctx: Arc<RuntimeContext>,
    cancel: Arc<CancelToken>,
    cancel_mask: usize,
    fuel: Option<u64>,
    rng_state: u64,
    debug_stack: Vec<DebugFrame>,
    /// Counter used to amortize cancel-token checks (checked every 64 evals).
    check_counter: u32,
}

#[derive(Clone)]
struct DebugFrame {
    fn_name: String,
    call_id: u64,
    start: Option<std::time::Instant>,
}

#[derive(Clone)]
enum RuntimeError {
    Error(Value),
    Cancelled,
    Message(String),
}

#[derive(Debug, Clone)]
pub struct TestFailure {
    pub name: String,
    pub description: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct TestSuccess {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct TestReport {
    pub passed: usize,
    pub failed: usize,
    pub failures: Vec<TestFailure>,
    pub successes: Vec<TestSuccess>,
}

pub fn run_native(program: HirProgram) -> Result<(), AiviError> {
    let mut runtime = build_runtime_from_program(program)?;
    run_main_effect(&mut runtime)
}

pub fn run_native_jit(
    program: HirProgram,
    cg_types: HashMap<String, HashMap<String, CgType>>,
) -> Result<(), AiviError> {
    let mut runtime = build_runtime_from_program(program.clone())?;
    let jitted = build_jitted_globals(program, cg_types)?;
    for (name, value) in jitted {
        runtime.ctx.globals.set(name, value);
    }
    run_main_effect(&mut runtime)
}

/// Runs `main` with a simple "fuel" limit to prevent hangs in fuzzers/tests.
///
/// If fuel is exhausted, execution is cancelled and treated as success (the program is considered
/// non-terminating within the provided budget).
pub fn run_native_with_fuel(program: HirProgram, fuel: u64) -> Result<(), AiviError> {
    let mut runtime = build_runtime_from_program(program)?;
    runtime.fuel = Some(fuel);

    let main = runtime
        .ctx
        .globals
        .get("main")
        .ok_or_else(|| AiviError::Runtime("missing main definition".to_string()))?;
    let main_value = match runtime.force_value(main) {
        Ok(value) => value,
        Err(RuntimeError::Cancelled) => return Ok(()),
        Err(err) => return Err(AiviError::Runtime(format_runtime_error(err))),
    };
    let effect = match main_value {
        Value::Effect(effect) => Value::Effect(effect),
        other => {
            return Err(AiviError::Runtime(format!(
                "main must be an Effect value, got {}",
                format_value(&other)
            )))
        }
    };

    match runtime.run_effect_value(effect) {
        Ok(_) => Ok(()),
        Err(RuntimeError::Cancelled) => Ok(()),
        Err(err) => Err(AiviError::Runtime(format_runtime_error(err))),
    }
}

fn run_main_effect(runtime: &mut Runtime) -> Result<(), AiviError> {
    let main = runtime
        .ctx
        .globals
        .get("main")
        .ok_or_else(|| AiviError::Runtime("missing main definition".to_string()))?;
    let main_value = match runtime.force_value(main) {
        Ok(value) => value,
        Err(err) => return Err(AiviError::Runtime(format_runtime_error(err))),
    };
    let effect = match main_value {
        Value::Effect(effect) => Value::Effect(effect),
        other => {
            return Err(AiviError::Runtime(format!(
                "main must be an Effect value, got {}",
                format_value(&other)
            )))
        }
    };

    match runtime.run_effect_value(effect) {
        Ok(_) => Ok(()),
        Err(err) => Err(AiviError::Runtime(format_runtime_error(err))),
    }
}

fn build_jitted_globals(
    program: HirProgram,
    cg_types: HashMap<String, HashMap<String, CgType>>,
) -> Result<HashMap<String, Value>, AiviError> {
    let kernel_program = kernel::lower_hir(program);
    let mut rust_program = rust_ir::lower_kernel(kernel_program)?;

    for module in &mut rust_program.modules {
        if let Some(mod_types) = cg_types.get(&module.name) {
            for def in &mut module.defs {
                if let Some(cg) = mod_types.get(&def.name) {
                    def.cg_type = Some(cg.clone());
                }
            }
        }
    }

    let mut global_cg_types: HashMap<String, CgType> = HashMap::new();
    for module in &rust_program.modules {
        for def in &module.defs {
            if let Some(cg_ty) = &def.cg_type {
                if cg_ty.is_closed() {
                    global_cg_types.entry(def.name.clone()).or_insert(cg_ty.clone());
                }
            }
        }
    }

    let mut jitted = HashMap::new();
    for module in rust_program.modules {
        for def in module.defs {
            let Some(cg_ty) = &def.cg_type else {
                continue;
            };
            let Some(param_tys) = scalar_int_param_types(cg_ty) else {
                continue;
            };
            let Some(ret_ty) = scalar_int_return_type(cg_ty) else {
                continue;
            };
            let (params, body) = peel_lambda_params(&def.expr);
            if params.len() != param_tys.len() {
                continue;
            }
            let locals: Vec<(String, CgType)> = params
                .iter()
                .cloned()
                .zip(param_tys.iter().cloned())
                .collect();
            let Some(lowered) =
                typed_cranelift::lower_for_jit(body, ret_ty, &global_cg_types, &locals)
            else {
                continue;
            };
            if lowered.param_names.len() > 6 {
                continue;
            }

            let local_positions: HashMap<String, usize> = params
                .iter()
                .enumerate()
                .map(|(idx, name)| (name.clone(), idx))
                .collect();
            if lowered
                .param_names
                .iter()
                .any(|name| name == &def.name && !local_positions.contains_key(name))
            {
                continue;
            }

            let symbol = format!("__aivi_jit_{}_{}", module.name.replace('.', "_"), def.name);
            let Ok(code_ptr) = compile_jit_i64_function(&symbol, lowered.function) else {
                continue;
            };
            let code_addr = code_ptr as usize;
            let arity = params.len();
            if arity == 0 {
                continue;
            }
            let param_order = lowered.param_names;
            let builtin = runtime_builtin(&format!("__jit|{}|{}", module.name, def.name), arity, {
                move |args, runtime| {
                    let mut call_args = Vec::with_capacity(param_order.len());
                    for name in &param_order {
                        if let Some(idx) = local_positions.get(name).copied() {
                            let Some(arg) = args.get(idx) else {
                                return Err(RuntimeError::Message(format!(
                                    "missing argument {idx} for jitted function"
                                )));
                            };
                            match arg {
                                Value::Int(value) => call_args.push(*value),
                                other => {
                                    return Err(RuntimeError::Message(format!(
                                        "jitted function expected Int arg, got {}",
                                        format_value(other)
                                    )))
                                }
                            }
                        } else {
                            let Some(global_value) = runtime.ctx.globals.get(name) else {
                                return Err(RuntimeError::Message(format!(
                                    "missing global {name} for jitted function"
                                )));
                            };
                            let forced = runtime.force_value(global_value)?;
                            match forced {
                                Value::Int(value) => call_args.push(value),
                                other => {
                                    return Err(RuntimeError::Message(format!(
                                        "jitted global {name} expected Int, got {}",
                                        format_value(&other)
                                    )))
                                }
                            }
                        }
                    }
                    let Some(result) = call_jitted_i64(code_addr, &call_args) else {
                        return Err(RuntimeError::Message(
                            "unsupported jitted function arity".to_string(),
                        ));
                    };
                    Ok(Value::Int(result))
                }
            });

            jitted.insert(def.name.clone(), builtin.clone());
            jitted.insert(format!("{}.{}", module.name, def.name), builtin);
        }
    }

    Ok(jitted)
}

fn scalar_int_param_types(ty: &CgType) -> Option<Vec<CgType>> {
    let mut params = Vec::new();
    let mut cursor = ty;
    while let CgType::Func(arg, ret) = cursor {
        if !matches!(&**arg, CgType::Int) {
            return None;
        }
        params.push((**arg).clone());
        cursor = ret;
    }
    Some(params)
}

fn scalar_int_return_type(ty: &CgType) -> Option<&CgType> {
    let mut cursor = ty;
    while let CgType::Func(_, ret) = cursor {
        cursor = ret;
    }
    if matches!(cursor, CgType::Int) {
        Some(cursor)
    } else {
        None
    }
}

fn peel_lambda_params<'a>(expr: &'a rust_ir::RustIrExpr) -> (Vec<String>, &'a rust_ir::RustIrExpr) {
    let mut params = Vec::new();
    let mut cursor = expr;
    while let rust_ir::RustIrExpr::Lambda { param, body, .. } = cursor {
        params.push(param.clone());
        cursor = body;
    }
    (params, cursor)
}

fn compile_jit_i64_function(
    symbol: &str,
    function: cranelift_codegen::ir::Function,
) -> Result<*const u8, String> {
    let builder = JITBuilder::new(default_libcall_names())
        .map_err(|err| format!("jit builder init failed for {symbol}: {err}"))?;
    let mut module = JITModule::new(builder);
    let mut ctx = module.make_context();
    ctx.func = function;
    let func_id = module
        .declare_function(symbol, Linkage::Local, &ctx.func.signature)
        .map_err(|err| format!("jit declare_function failed for {symbol}: {err}"))?;
    module
        .define_function(func_id, &mut ctx)
        .map_err(|err| format!("jit define_function failed for {symbol}: {err}"))?;
    module.clear_context(&mut ctx);
    module
        .finalize_definitions()
        .map_err(|err| format!("jit finalize_definitions failed for {symbol}: {err}"))?;
    let ptr = module.get_finalized_function(func_id);
    std::mem::forget(module);
    Ok(ptr)
}

fn call_jitted_i64(code_addr: usize, args: &[i64]) -> Option<i64> {
    let code = code_addr as *const u8;
    match args.len() {
        0 => {
            let f: extern "C" fn() -> i64 = unsafe { std::mem::transmute(code) };
            Some(f())
        }
        1 => {
            let f: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(code) };
            Some(f(args[0]))
        }
        2 => {
            let f: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(code) };
            Some(f(args[0], args[1]))
        }
        3 => {
            let f: extern "C" fn(i64, i64, i64) -> i64 = unsafe { std::mem::transmute(code) };
            Some(f(args[0], args[1], args[2]))
        }
        4 => {
            let f: extern "C" fn(i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(code) };
            Some(f(args[0], args[1], args[2], args[3]))
        }
        5 => {
            let f: extern "C" fn(i64, i64, i64, i64, i64) -> i64 =
                unsafe { std::mem::transmute(code) };
            Some(f(args[0], args[1], args[2], args[3], args[4]))
        }
        6 => {
            let f: extern "C" fn(i64, i64, i64, i64, i64, i64) -> i64 =
                unsafe { std::mem::transmute(code) };
            Some(f(args[0], args[1], args[2], args[3], args[4], args[5]))
        }
        _ => None,
    }
}

pub fn run_test_suite(
    program: HirProgram,
    test_entries: &[(String, String)],
    surface_modules: &[crate::surface::Module],
) -> Result<TestReport, AiviError> {
    const TEST_FUEL_BUDGET: u64 = 500_000;
    let mut runtime = build_runtime_from_program_scoped(program, surface_modules)?;
    let mut report = TestReport {
        passed: 0,
        failed: 0,
        failures: Vec::new(),
        successes: Vec::new(),
    };

    for (name, description) in test_entries {
        // Keep a runaway test from exhausting the thread stack; each test gets a fresh budget.
        runtime.fuel = Some(TEST_FUEL_BUDGET);
        let Some(value) = runtime.ctx.globals.get(name) else {
            report.failed += 1;
            report.failures.push(TestFailure {
                name: name.clone(),
                description: description.clone(),
                message: "missing definition".to_string(),
            });
            continue;
        };

        let value = match runtime.force_value(value) {
            Ok(value) => value,
            Err(err) => {
                report.failed += 1;
                report.failures.push(TestFailure {
                    name: name.clone(),
                    description: description.clone(),
                    message: format_runtime_error(err),
                });
                continue;
            }
        };

        let effect = match value {
            Value::Effect(effect) => Value::Effect(effect),
            other => {
                report.failed += 1;
                report.failures.push(TestFailure {
                    name: name.clone(),
                    description: description.clone(),
                    message: format!("test must be an Effect value, got {}", format_value(&other)),
                });
                continue;
            }
        };

        match runtime.run_effect_value(effect) {
            Ok(_) => {
                report.passed += 1;
                report.successes.push(TestSuccess {
                    name: name.clone(),
                    description: description.clone(),
                });
            }
            Err(err) => {
                report.failed += 1;
                report.failures.push(TestFailure {
                    name: name.clone(),
                    description: description.clone(),
                    message: format_runtime_error(err),
                });
            }
        }
    }

    Ok(report)
}

fn build_runtime_from_program(program: HirProgram) -> Result<Runtime, AiviError> {
    if program.modules.is_empty() {
        return Err(AiviError::Runtime("no modules to run".to_string()));
    }

    let mut grouped: HashMap<String, Vec<HirExpr>> = HashMap::new();
    for module in program.modules {
        let module_name = module.name.clone();
        for def in module.defs {
            // Unqualified entry (legacy/global namespace).
            grouped
                .entry(def.name.clone())
                .or_default()
                .push(def.expr.clone());

            // Qualified entry enables disambiguation (e.g. `aivi.database.load`) without relying
            // on wildcard imports to win against builtins like `load`.
            grouped
                .entry(format!("{module_name}.{}", def.name))
                .or_default()
                .push(def.expr);
        }
    }
    if grouped.is_empty() {
        return Err(AiviError::Runtime("no definitions to run".to_string()));
    }

    let globals = Env::new(None);
    register_builtins(&globals);
    globals.set("__machine_on".to_string(), make_machine_on_builtin());
    for (name, exprs) in grouped {
        // Builtins are the "runtime stdlib" today; don't let parsed source overwrite them.
        if globals.get(&name).is_some() {
            continue;
        }
        if exprs.len() == 1 {
            let thunk = ThunkValue {
                expr: Arc::new(exprs.into_iter().next().unwrap()),
                env: globals.clone(),
                cached: Mutex::new(None),
                in_progress: AtomicBool::new(false),
            };
            globals.set(name, Value::Thunk(Arc::new(thunk)));
        } else {
            let mut clauses = Vec::new();
            for expr in exprs {
                let thunk = ThunkValue {
                    expr: Arc::new(expr),
                    env: globals.clone(),
                    cached: Mutex::new(None),
                    in_progress: AtomicBool::new(false),
                };
                clauses.push(Value::Thunk(Arc::new(thunk)));
            }
            globals.set(name, Value::MultiClause(clauses));
        }
    }

    let ctx = Arc::new(RuntimeContext::new_with_constructor_ordinals(
        globals,
        core_constructor_ordinals(),
    ));
    let cancel = CancelToken::root();
    Ok(Runtime::new(ctx, cancel))
}

fn build_runtime_from_program_scoped(
    program: HirProgram,
    surface_modules: &[crate::surface::Module],
) -> Result<Runtime, AiviError> {
    if program.modules.is_empty() {
        return Err(AiviError::Runtime("no modules to run".to_string()));
    }

    let globals = Env::new(None);
    register_builtins(&globals);
    globals.set("__machine_on".to_string(), make_machine_on_builtin());

    // Build a map of surface module metadata for import scoping.
    let mut surface_by_name: HashMap<String, &crate::surface::Module> = HashMap::new();
    for module in surface_modules {
        surface_by_name.insert(module.name.name.clone(), module);
    }
    let mut value_exports: HashMap<String, Vec<String>> = HashMap::new();
    let mut domain_members: HashMap<(String, String), Vec<String>> = HashMap::new();
    let mut method_names: HashSet<String> = HashSet::new();
    for module in surface_modules {
        value_exports.insert(
            module.name.name.clone(),
            module
                .exports
                .iter()
                .filter(|e| e.kind == crate::surface::ScopeItemKind::Value)
                .map(|e| e.name.name.clone())
                .collect(),
        );
        for export in &module.exports {
            if export.kind != crate::surface::ScopeItemKind::Domain {
                continue;
            }
            let domain_name = export.name.name.clone();
            let mut members = Vec::new();
            for item in &module.items {
                let crate::surface::ModuleItem::DomainDecl(domain) = item else {
                    continue;
                };
                if domain.name.name != domain_name {
                    continue;
                }
                for domain_item in &domain.items {
                    match domain_item {
                        crate::surface::DomainItem::Def(def)
                        | crate::surface::DomainItem::LiteralDef(def) => {
                            members.push(def.name.name.clone());
                        }
                        crate::surface::DomainItem::TypeAlias(_)
                        | crate::surface::DomainItem::TypeSig(_) => {}
                    }
                }
            }
            domain_members.insert((module.name.name.clone(), domain_name), members);
        }

        // Methods (class members) behave like open multi-clause functions at runtime: instances can
        // add new clauses. When importing, we merge method bindings instead of overwriting locals.
        for item in &module.items {
            let crate::surface::ModuleItem::ClassDecl(class_decl) = item else {
                continue;
            };
            for member in &class_decl.members {
                method_names.insert(member.name.name.clone());
            }
        }
    }

    fn merge_method_binding(existing: Value, imported: Value) -> Value {
        fn flatten(value: Value, out: &mut Vec<Value>) {
            match value {
                Value::MultiClause(clauses) => out.extend(clauses),
                other => out.push(other),
            }
        }

        let mut clauses = Vec::new();
        // Keep local clauses first so user-defined instances override defaults.
        flatten(existing, &mut clauses);
        flatten(imported, &mut clauses);
        Value::MultiClause(clauses)
    }

    // Create a per-module environment rooted at the global environment. Each top-level def thunk
    // captures its module env so runtime evaluation respects lexical imports and avoids global
    // collisions (especially for operator names like `(+)`).
    let mut module_envs: HashMap<String, Env> = HashMap::new();
    for module in &program.modules {
        module_envs.insert(module.name.clone(), Env::new(Some(globals.clone())));
    }

    // First pass: register qualified globals for every definition, preserving multi-clause
    // functions (same qualified name defined multiple times).
    let mut grouped: HashMap<String, (Env, Vec<HirExpr>)> = HashMap::new();
    for module in &program.modules {
        let module_name = module.name.clone();
        let module_env = module_envs
            .get(&module_name)
            .cloned()
            .unwrap_or_else(|| Env::new(Some(globals.clone())));
        for def in &module.defs {
            let name = format!("{module_name}.{}", def.name);
            grouped
                .entry(name)
                .or_insert_with(|| (module_env.clone(), Vec::new()))
                .1
                .push(def.expr.clone());
        }
    }
    for (name, (module_env, exprs)) in grouped {
        if globals.get(&name).is_some() {
            continue;
        }
        if exprs.len() == 1 {
            let thunk = ThunkValue {
                expr: Arc::new(exprs.into_iter().next().unwrap()),
                env: module_env,
                cached: Mutex::new(None),
                in_progress: AtomicBool::new(false),
            };
            globals.set(name, Value::Thunk(Arc::new(thunk)));
        } else {
            let mut clauses = Vec::new();
            for expr in exprs {
                let thunk = ThunkValue {
                    expr: Arc::new(expr),
                    env: module_env.clone(),
                    cached: Mutex::new(None),
                    in_progress: AtomicBool::new(false),
                };
                clauses.push(Value::Thunk(Arc::new(thunk)));
            }
            globals.set(name, Value::MultiClause(clauses));
        }
    }

    let mut machine_specs: Vec<(String, String, HashMap<String, Vec<MachineEdge>>)> = Vec::new();

    // Second pass: populate each module env with its local defs and imports.
    for module in &program.modules {
        let module_name = module.name.clone();
        let module_env = module_envs
            .get(&module_name)
            .cloned()
            .unwrap_or_else(|| Env::new(Some(globals.clone())));

        // Local defs in the module are always in scope unqualified.
        for def in &module.defs {
            let qualified = format!("{module_name}.{}", def.name);
            if let Some(value) = globals.get(&qualified) {
                module_env.set(def.name.clone(), value);
            }
        }

        // Import exported values and domain members.
        let Some(surface_module) = surface_by_name.get(&module_name).copied() else {
            continue;
        };
        for use_decl in &surface_module.uses {
            let imported_mod = use_decl.module.name.clone();
            if use_decl.wildcard {
                if let Some(names) = value_exports.get(&imported_mod) {
                    for name in names {
                        let qualified = format!("{imported_mod}.{name}");
                        if let Some(value) = globals.get(&qualified) {
                            if let Some(existing) = module_env.get(name) {
                                if method_names.contains(name) {
                                    module_env.set(
                                        name.clone(),
                                        merge_method_binding(existing, value),
                                    );
                                    continue;
                                }
                            }
                            // Non-methods: last import wins (allows more-specific modules to shadow)
                            module_env.set(name.clone(), value);
                        }
                    }
                }
                continue;
            }
            for item in &use_decl.items {
                match item.kind {
                    crate::surface::ScopeItemKind::Value => {
                        let name = item.name.name.clone();
                        let qualified = format!("{imported_mod}.{name}");
                        if let Some(value) = globals.get(&qualified) {
                            if let Some(existing) = module_env.get(&name) {
                                if method_names.contains(&name) {
                                    module_env.set(
                                        name.clone(),
                                        merge_method_binding(existing, value),
                                    );
                                    continue;
                                }
                            }
                            module_env.set(name, value);
                        }
                    }
                    crate::surface::ScopeItemKind::Domain => {
                        let domain_name = item.name.name.clone();
                        let key = (imported_mod.clone(), domain_name);
                        if let Some(members) = domain_members.get(&key) {
                            for member in members {
                                let qualified = format!("{imported_mod}.{member}");
                                if let Some(value) = globals.get(&qualified) {
                                    if let Some(existing) = module_env.get(member) {
                                        if method_names.contains(member) {
                                            module_env.set(
                                                member.clone(),
                                                merge_method_binding(existing, value),
                                            );
                                            continue;
                                        }
                                    }
                                    module_env.set(member.clone(), value);
                                }
                            }
                        }
                    }
                }
            }
        }

        bind_module_machine_values(
            surface_module,
            &module_name,
            &module_env,
            &globals,
            &mut machine_specs,
        );

        // Re-apply local defs after imports so that local definitions always
        // shadow imported names (including domain members).  Without this,
        // a wildcard `use` that brings in a domain method with the same name
        // as a local binding would silently overwrite the local definition.
        for def in &module.defs {
            let qualified = format!("{module_name}.{}", def.name);
            if let Some(value) = globals.get(&qualified) {
                module_env.set(def.name.clone(), value);
            }
        }

        // Re-export forwarding: a module can `export x` where `x` is brought into scope via `use`
        // (e.g. facade modules like `aivi.linalg`). Ensure qualified access `Module.x` resolves by
        // registering exported bindings that exist in the module env, even when they aren't local
        // definitions.
        for export in &surface_module.exports {
            if export.kind != crate::surface::ScopeItemKind::Value {
                continue;
            }
            let name = export.name.name.clone();
            let qualified = format!("{module_name}.{name}");
            if globals.get(&qualified).is_some() {
                continue;
            }
            if let Some(value) = module_env.get(&name) {
                globals.set(qualified, value);
            }
        }
    }

    let mut constructor_ordinals = core_constructor_ordinals();
    for (name, ordinal) in collect_surface_constructor_ordinals(surface_modules) {
        match ordinal {
            Some(idx) => insert_constructor_ordinal(&mut constructor_ordinals, name, idx),
            None => {
                constructor_ordinals.insert(name, None);
            }
        }
    }
    let ctx = Arc::new(RuntimeContext::new_with_constructor_ordinals(
        globals,
        constructor_ordinals,
    ));
    for (machine_name, initial_state, transitions) in machine_specs {
        ctx.register_machine(machine_name, initial_state, transitions);
    }
    let cancel = CancelToken::root();
    Ok(Runtime::new(ctx, cancel))
}

fn runtime_builtin(
    name: &str,
    arity: usize,
    func: impl Fn(Vec<Value>, &mut Runtime) -> Result<Value, RuntimeError> + Send + Sync + 'static,
) -> Value {
    Value::Builtin(BuiltinValue {
        imp: Arc::new(BuiltinImpl {
            name: name.to_string(),
            arity,
            func: Arc::new(func),
        }),
        args: Vec::new(),
        tagged_args: Some(Vec::new()),
    })
}

fn machine_transition_builtin_name(machine_name: &str, event: &str) -> String {
    format!("__machine_transition|{machine_name}|{event}")
}

fn parse_machine_transition_ref(value: &Value) -> Option<(String, String)> {
    let Value::Builtin(builtin) = value else {
        return None;
    };
    if !builtin.args.is_empty() {
        return None;
    }
    let name = &builtin.imp.name;
    let mut parts = name.splitn(3, '|');
    let prefix = parts.next()?;
    if prefix != "__machine_transition" {
        return None;
    }
    let machine = parts.next()?.to_string();
    let event = parts.next()?.to_string();
    Some((machine, event))
}

fn make_machine_on_builtin() -> Value {
    runtime_builtin("__machine_on", 2, |mut args, _| {
        let handler = args.pop().unwrap_or(Value::Unit);
        let transition = args.pop().unwrap_or(Value::Unit);
        if let Some((machine_name, event_name)) = parse_machine_transition_ref(&transition) {
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    runtime
                        .ctx
                        .register_machine_handler(&machine_name, &event_name, handler.clone());
                    Ok(Value::Unit)
                }),
            };
            return Ok(Value::Effect(Arc::new(effect)));
        }

        match handler {
            Value::Effect(_) | Value::Source(_) => Ok(handler),
            other => Err(RuntimeError::Message(format!(
                "`on` handler must be an Effect, got {}",
                format_value(&other)
            ))),
        }
    })
}

fn make_machine_transition_builtin(machine_name: String, event_name: String) -> Value {
    let builtin_name = machine_transition_builtin_name(&machine_name, &event_name);
    runtime_builtin(&builtin_name, 1, move |mut args, _| {
        let _payload = args.pop().unwrap_or(Value::Unit);
        let machine_name = machine_name.clone();
        let event_name = event_name.clone();
        let effect = EffectValue::Thunk {
            func: Arc::new(move |runtime| {
                runtime
                    .ctx
                    .apply_machine_transition(&machine_name, &event_name)
                    .map_err(|err| RuntimeError::Error(err.into_value()))?;
                for handler in runtime.ctx.machine_handlers(&machine_name, &event_name) {
                    runtime.run_effect_value(handler)?;
                }
                Ok(Value::Unit)
            }),
        };
        Ok(Value::Effect(Arc::new(effect)))
    })
}

fn make_machine_current_state_builtin(machine_name: String) -> Value {
    runtime_builtin(
        &format!("__machine_current_state|{machine_name}"),
        1,
        move |mut args, runtime| {
            let _ = args.pop();
            let Some(state) = runtime.ctx.machine_current_state(&machine_name) else {
                return Err(RuntimeError::Message(format!(
                    "unknown machine state for {machine_name}"
                )));
            };
            Ok(Value::Constructor {
                name: state,
                args: Vec::new(),
            })
        },
    )
}

fn make_machine_can_builtin(machine_name: String, event_name: String) -> Value {
    runtime_builtin(
        &format!("__machine_can|{machine_name}|{event_name}"),
        1,
        move |mut args, runtime| {
            let _ = args.pop();
            Ok(Value::Bool(
                runtime
                    .ctx
                    .machine_can_transition(&machine_name, &event_name),
            ))
        },
    )
}

fn bind_module_machine_values(
    surface_module: &crate::surface::Module,
    module_name: &str,
    module_env: &Env,
    globals: &Env,
    machine_specs: &mut Vec<(String, String, HashMap<String, Vec<MachineEdge>>)>,
) {
    for item in &surface_module.items {
        let crate::surface::ModuleItem::MachineDecl(machine_decl) = item else {
            continue;
        };

        let runtime_machine_name = format!("{module_name}.{}", machine_decl.name.name);
        let mut transitions: HashMap<String, Vec<MachineEdge>> = HashMap::new();
        let mut initial_state = machine_decl
            .transitions
            .iter()
            .find(|transition| transition.source.name.is_empty())
            .map(|transition| transition.target.name.clone())
            .or_else(|| {
                machine_decl
                    .transitions
                    .first()
                    .map(|transition| transition.target.name.clone())
            })
            .or_else(|| machine_decl.states.first().map(|state| state.name.name.clone()))
            .unwrap_or_else(|| "Closed".to_string());

        for transition in &machine_decl.transitions {
            let source = if transition.source.name.is_empty() {
                None
            } else {
                Some(transition.source.name.clone())
            };
            if source.is_none() {
                initial_state = transition.target.name.clone();
            }
            transitions
                .entry(transition.name.name.clone())
                .or_default()
                .push(MachineEdge {
                    source,
                    target: transition.target.name.clone(),
                });
        }

        let mut state_names = machine_decl
            .states
            .iter()
            .map(|state| state.name.name.clone())
            .collect::<Vec<_>>();
        state_names.sort();
        state_names.dedup();
        for state_name in state_names {
            let state_ctor = Value::Constructor {
                name: state_name.clone(),
                args: Vec::new(),
            };
            module_env.set(state_name.clone(), state_ctor.clone());
            let qualified = format!("{module_name}.{state_name}");
            if globals.get(&qualified).is_none() {
                globals.set(qualified, state_ctor);
            }
        }

        let mut machine_fields: HashMap<String, Value> = HashMap::new();
        let mut can_fields: HashMap<String, Value> = HashMap::new();
        let mut event_names = transitions.keys().cloned().collect::<Vec<_>>();
        event_names.sort();
        for event_name in event_names {
            let transition_value =
                make_machine_transition_builtin(runtime_machine_name.clone(), event_name.clone());
            machine_fields.insert(event_name.clone(), transition_value.clone());
            module_env.set(event_name.clone(), transition_value.clone());
            let qualified_transition = format!("{module_name}.{event_name}");
            if globals.get(&qualified_transition).is_none() {
                globals.set(qualified_transition, transition_value);
            }
            can_fields.insert(
                event_name.clone(),
                make_machine_can_builtin(runtime_machine_name.clone(), event_name),
            );
        }

        machine_fields.insert(
            "currentState".to_string(),
            make_machine_current_state_builtin(runtime_machine_name.clone()),
        );
        machine_fields.insert("can".to_string(), Value::Record(Arc::new(can_fields)));
        let machine_value = Value::Record(Arc::new(machine_fields));
        module_env.set(machine_decl.name.name.clone(), machine_value.clone());
        let qualified_machine = format!("{module_name}.{}", machine_decl.name.name);
        if globals.get(&qualified_machine).is_none() {
            globals.set(qualified_machine, machine_value);
        }

        machine_specs.push((runtime_machine_name, initial_state, transitions));
    }
}

fn format_runtime_error(err: RuntimeError) -> String {
    match err {
        RuntimeError::Cancelled => "execution cancelled".to_string(),
        RuntimeError::Message(message) => message,
        RuntimeError::Error(value) => format!("runtime error: {}", format_value(&value)),
    }
}

fn insert_constructor_ordinal(
    ordinals: &mut HashMap<String, Option<usize>>,
    name: String,
    ordinal: usize,
) {
    match ordinals.get(&name) {
        None => {
            ordinals.insert(name, Some(ordinal));
        }
        Some(Some(existing)) if *existing == ordinal => {}
        _ => {
            ordinals.insert(name, None);
        }
    }
}

fn core_constructor_ordinals() -> HashMap<String, Option<usize>> {
    let mut ordinals = HashMap::new();
    insert_constructor_ordinal(&mut ordinals, "True".to_string(), 0);
    insert_constructor_ordinal(&mut ordinals, "False".to_string(), 1);
    insert_constructor_ordinal(&mut ordinals, "None".to_string(), 0);
    insert_constructor_ordinal(&mut ordinals, "Some".to_string(), 1);
    insert_constructor_ordinal(&mut ordinals, "Err".to_string(), 0);
    insert_constructor_ordinal(&mut ordinals, "Ok".to_string(), 1);
    insert_constructor_ordinal(&mut ordinals, "Closed".to_string(), 0);
    ordinals
}

fn collect_surface_constructor_ordinals(
    surface_modules: &[crate::surface::Module],
) -> HashMap<String, Option<usize>> {
    let mut ordinals = HashMap::new();
    for module in surface_modules {
        for item in &module.items {
            match item {
                crate::surface::ModuleItem::TypeDecl(decl) => {
                    for (ordinal, ctor) in decl.constructors.iter().enumerate() {
                        insert_constructor_ordinal(&mut ordinals, ctor.name.name.clone(), ordinal);
                    }
                }
                crate::surface::ModuleItem::DomainDecl(domain) => {
                    for domain_item in &domain.items {
                        let crate::surface::DomainItem::TypeAlias(decl) = domain_item else {
                            continue;
                        };
                        for (ordinal, ctor) in decl.constructors.iter().enumerate() {
                            insert_constructor_ordinal(
                                &mut ordinals,
                                ctor.name.name.clone(),
                                ordinal,
                            );
                        }
                    }
                }
                crate::surface::ModuleItem::MachineDecl(machine_decl) => {
                    for (ordinal, state) in machine_decl.states.iter().enumerate() {
                        insert_constructor_ordinal(&mut ordinals, state.name.name.clone(), ordinal);
                    }
                }
                _ => {}
            }
        }
    }
    ordinals
}

include!("runtime_impl/lifecycle_and_cancel.rs");
include!("runtime_impl/eval_and_apply.rs");
include!("runtime_impl/resources.rs");
include!("runtime_impl/trampoline.rs");

impl BuiltinValue {
    fn apply(&self, arg: Value, runtime: &mut Runtime) -> Result<Value, RuntimeError> {
        let mut args = self.args.clone();
        let mut tagged_args = self.tagged_args.clone();
        let mut pending_arg = Some(arg);
        if let Some(existing) = tagged_args.as_mut() {
            if let Some(tagged) = TaggedValue::from_value(pending_arg.as_ref().expect("pending arg")) {
                existing.push(tagged);
                pending_arg = None;
            } else {
                args = existing.iter().copied().map(TaggedValue::to_value).collect();
                tagged_args = None;
            }
        }
        if let Some(arg) = pending_arg {
            args.push(arg);
        }
        if args.is_empty() {
            if let Some(existing) = tagged_args.as_ref() {
                if existing.len() == self.imp.arity {
                    args = existing.iter().copied().map(TaggedValue::to_value).collect();
                } else {
                    return Ok(Value::Builtin(BuiltinValue {
                        imp: self.imp.clone(),
                        args,
                        tagged_args,
                    }));
                }
            }
        }
        if args.len() == self.imp.arity {
            (self.imp.func)(args, runtime)
        } else {
            Ok(Value::Builtin(BuiltinValue {
                imp: self.imp.clone(),
                args,
                tagged_args,
            }))
        }
    }
}

fn collect_pattern_bindings(pattern: &HirPattern, value: &Value) -> Option<HashMap<String, Value>> {
    let mut bindings = HashMap::new();
    if match_pattern(pattern, value, &mut bindings) {
        Some(bindings)
    } else {
        None
    }
}

fn match_pattern(
    pattern: &HirPattern,
    value: &Value,
    bindings: &mut HashMap<String, Value>,
) -> bool {
    match pattern {
        HirPattern::Wildcard { .. } => true,
        HirPattern::Var { name, .. } => {
            bindings.insert(name.clone(), value.clone());
            true
        }
        HirPattern::At { name, pattern, .. } => {
            bindings.insert(name.clone(), value.clone());
            match_pattern(pattern, value, bindings)
        }
        HirPattern::Literal { value: lit, .. } => match (lit, value) {
            (HirLiteral::Number(text), Value::Int(num)) => parse_number_literal(text) == Some(*num),
            (HirLiteral::Number(text), Value::Float(num)) => text.parse::<f64>().ok() == Some(*num),
            (HirLiteral::String(text), Value::Text(val)) => text == val,
            (HirLiteral::Sigil { tag, body, flags }, Value::Record(map)) => {
                let tag_ok = matches!(map.get("tag"), Some(Value::Text(val)) if val == tag);
                let body_ok = matches!(map.get("body"), Some(Value::Text(val)) if val == body);
                let flags_ok = matches!(map.get("flags"), Some(Value::Text(val)) if val == flags);
                tag_ok && body_ok && flags_ok
            }
            (HirLiteral::Bool(flag), Value::Bool(val)) => *flag == *val,
            (HirLiteral::DateTime(text), Value::DateTime(val)) => text == val,
            _ => false,
        },
        HirPattern::Constructor { name, args, .. } => match value {
            Value::Constructor {
                name: value_name,
                args: value_args,
            } => {
                if name != value_name || args.len() != value_args.len() {
                    return false;
                }
                for (pat, val) in args.iter().zip(value_args.iter()) {
                    if !match_pattern(pat, val, bindings) {
                        return false;
                    }
                }
                true
            }
            _ => false,
        },
        HirPattern::Tuple { items, .. } => match value {
            Value::Tuple(values) => {
                if items.len() != values.len() {
                    return false;
                }
                for (pat, val) in items.iter().zip(values.iter()) {
                    if !match_pattern(pat, val, bindings) {
                        return false;
                    }
                }
                true
            }
            _ => false,
        },
        HirPattern::List { items, rest, .. } => match value {
            Value::List(values) => {
                if values.len() < items.len() {
                    return false;
                }
                for (pat, val) in items.iter().zip(values.iter()) {
                    if !match_pattern(pat, val, bindings) {
                        return false;
                    }
                }
                if let Some(rest) = rest {
                    let tail = values[items.len()..].to_vec();
                    match_pattern(rest, &Value::List(Arc::new(tail)), bindings)
                } else {
                    values.len() == items.len()
                }
            }
            _ => false,
        },
        HirPattern::Record { fields, .. } => match value {
            Value::Record(map) => {
                for field in fields {
                    let Some(value) = record_get_path(map, &field.path) else {
                        return false;
                    };
                    if !match_pattern(&field.pattern, value, bindings) {
                        return false;
                    }
                }
                true
            }
            _ => false,
        },
    }
}
