//! Cranelift compilation pipeline (shared between JIT and AOT).
//!
//! `run_cranelift_jit` is the JIT entrypoint that compiles and executes in-memory.
//! `compile_to_object` is the AOT entrypoint that emits a native object file.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use cranelift_codegen::ir::{types, AbiParam, Function, InstBuilder};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{Linkage, Module};

use crate::cg_type::CgType;
use crate::hir::HirProgram;
use crate::runtime::json_schema::cg_type_to_json_schema;
use crate::runtime::values::Value;
use crate::runtime::{
    build_runtime_from_program, build_runtime_from_program_with_cancel,
    collect_surface_constructor_ordinals, format_value, run_main_effect, CancelToken,
    ReactiveCellKind, Runtime, RuntimeError,
};
use crate::rust_ir::{
    RustIrDef, RustIrExpr, RustIrListItem, RustIrPathSegment, RustIrPattern, RustIrRecordField,
    RustIrTextPart,
};
use crate::AiviError;
use crate::{kernel, rust_ir};

use super::abi::JitRuntimeCtx;

const SOURCE_CONSTRUCTOR_SCHEMA_SUFFIX: &str = "::source_sites";
use super::jit_module::create_jit_module;
use super::lower::{
    declare_helpers, decompose_func_type, CompiledLambda, DeclaredHelpers, JitFuncDecl,
    JitFuncInfo, LowerCtx,
};

/// Pointer type used throughout.
const PTR: cranelift_codegen::ir::Type = types::I64;
const MAX_JIT_ARITY: usize = 32;

/// JIT-compile all definitions in a program and register them into the runtime.
///
/// This is the shared compilation pipeline used by both `run_cranelift_jit` and
/// `run_test_suite_jit`. The caller is responsible for running main or tests
/// after this returns.
///
/// Returns the JIT module (must be kept alive while JIT code is running).
fn jit_compile_into_runtime(
    program: HirProgram,
    cg_types: HashMap<String, HashMap<String, CgType>>,
    monomorph_plan: HashMap<String, Vec<CgType>>,
    source_schemas: HashMap<String, Vec<CgType>>,
    runtime: &mut Runtime,
    rebound_names: &HashSet<String>,
) -> Result<cranelift_jit::JITModule, AiviError> {
    let trace = std::env::var("AIVI_TRACE_TIMING").is_ok_and(|v| v == "1");
    macro_rules! timed {
        ($label:expr, $block:expr) => {{
            let _t0 = if trace { Some(Instant::now()) } else { None };
            let r = $block;
            if let Some(t0) = _t0 {
                eprintln!(
                    "[AIVI_TIMING] {:40} {:>8.1}ms",
                    $label,
                    t0.elapsed().as_secs_f64() * 1000.0
                );
            }
            r
        }};
    }

    // Lower HIR → desugar blocks → RustIR
    let desugared_program = timed!("desugar blocks (HIR)", kernel::desugar_blocks(program));
    let mut rust_program = timed!(
        "lower HIR → RustIR",
        rust_ir::lower_kernel(desugared_program)?
    );

    // Annotate each def with its CgType
    timed!("annotate CgTypes", {
        for module in &mut rust_program.modules {
            if let Some(module_types) = cg_types.get(&module.name) {
                for def in &mut module.defs {
                    let cg_ty = module_types.get(&def.name).or_else(|| {
                        def.name
                            .rsplit('.')
                            .next()
                            .and_then(|short| module_types.get(short))
                    });
                    if let Some(cg_ty) = cg_ty {
                        def.cg_type = Some(cg_ty.clone());
                    }
                }
            }
        }
    });

    // Inject JSON validation schemas at `load` call sites
    timed!("inject source schemas", {
        inject_source_schemas(&mut rust_program.modules, &source_schemas);
    });

    // Monomorphize
    let spec_map = timed!("monomorphize", {
        if std::env::var("AIVI_DEBUG_MONO").is_ok() {
            for (k, v) in &monomorph_plan {
                if k.contains("gtkApp") || k.contains("gtk") {
                    eprintln!("[DEBUG MONO] {} => {:?}", k, v);
                }
            }
        }
        monomorphize_program(&mut rust_program.modules, &monomorph_plan)
    });

    // Inline small functions
    timed!(
        "inline_program",
        super::inline::inline_program(&mut rust_program.modules)
    );
    let duplicate_trivial_self_aliases =
        duplicate_trivial_self_alias_qualifieds(&rust_program.modules);

    let total_defs: usize = rust_program.modules.iter().map(|m| m.defs.len()).sum();
    if trace {
        eprintln!("[AIVI_TIMING] total defs to JIT-compile: {}", total_defs);
    }

    // Create JIT module with runtime helpers registered
    let mut module = timed!(
        "cranelift jit init",
        create_jit_module()
            .map_err(|e| AiviError::runtime_message(format!("cranelift jit init: {e}")))?
    );

    // Declare runtime helper imports in the module
    let helpers = timed!(
        "declare_helpers",
        declare_helpers(&mut module)
            .map_err(|e| AiviError::runtime_message(format!("cranelift declare helpers: {e}")))?
    );

    // Two-pass compilation for direct calls between JIT functions.
    //    Pass 1: Declare all function signatures and build a registry.
    //    Pass 2: Compile function bodies with the registry for direct calls.

    #[allow(dead_code)]
    struct DeclaredDef<'a> {
        def: &'a RustIrDef,
        module_name: String,
        qualified: String,
        func_name: String,
        func_id: cranelift_module::FuncId,
        arity: usize,
        param_types: Vec<Option<CgType>>,
        return_type: Option<CgType>,
        is_effect_block: bool,
    }
    let mut declared_defs: Vec<DeclaredDef> = Vec::new();
    let mut declared_names: HashSet<String> = HashSet::new();
    let mut jit_func_ids: HashMap<String, JitFuncDecl> = HashMap::new();
    // Counter per qualified name to generate unique Cranelift function names for
    // multi-clause definitions (e.g., domain operators with several pattern clauses).
    let mut clause_counters: HashMap<String, usize> = HashMap::new();

    // Pass 1: Declare all eligible functions
    let _pass1_t0 = if trace { Some(Instant::now()) } else { None };
    for ir_module in &rust_program.modules {
        let module_dot = format!("{}.", ir_module.name);
        for def in &ir_module.defs {
            // Skip qualified aliases emitted by the Kernel (e.g. name="aivi.generator.fromList"
            // in module "aivi.generator"). The bare def + Pass B alias is sufficient.
            if def.name.starts_with(&module_dot) {
                continue;
            }
            let qualified = format!("{}.{}", ir_module.name, def.name);
            if duplicate_trivial_self_aliases.contains(&qualified) && is_trivial_self_alias_def(def)
            {
                continue;
            }
            let (params, body) = peel_params(&def.expr);
            let is_stdlib_module = ir_module.name.starts_with("aivi.");
            if params.len() > MAX_JIT_ARITY {
                if is_stdlib_module {
                    continue;
                }
                return Err(AiviError::runtime_message(format!(
                    "cranelift compile {}: unsupported arity {} (max {})",
                    qualified,
                    params.len(),
                    MAX_JIT_ARITY
                )));
            }
            if !expr_supported(body) {
                if is_stdlib_module {
                    continue;
                }
                return Err(AiviError::runtime_message(format!(
                    "cranelift compile {}: unsupported expression shape",
                    qualified
                )));
            }
            let base_func_name = format!("__aivi_jit_{}", sanitize_name(&qualified));
            // Give each clause a unique Cranelift function name so all clauses
            // of a multi-clause def (like domain operators) get compiled.
            let func_name = if declared_names.contains(&base_func_name) {
                let counter = clause_counters.entry(qualified.clone()).or_insert(1);
                let name = format!("{}_{}", base_func_name, counter);
                *counter += 1;
                name
            } else {
                base_func_name.clone()
            };
            declared_names.insert(func_name.clone());

            let arity = params.len();
            let mut sig = module.make_signature();
            sig.params.push(AbiParam::new(PTR)); // ctx
            for _ in 0..arity {
                sig.params.push(AbiParam::new(PTR));
            }
            sig.returns.push(AbiParam::new(PTR));

            let func_id = module
                .declare_function(&func_name, Linkage::Local, &sig)
                .map_err(|e| AiviError::runtime_message(format!("declare {}: {e}", func_name)))?;

            // Extract typed param/return info from CgType
            let (param_types, return_type) = if let Some(cg_ty) = &def.cg_type {
                decompose_func_type(cg_ty, arity)
            } else {
                (vec![None; arity], None)
            };

            let is_effect_block = false;

            declared_defs.push(DeclaredDef {
                def,
                module_name: ir_module.name.clone(),
                qualified: qualified.clone(),
                func_name,
                func_id,
                arity,
                param_types: param_types.clone(),
                return_type: return_type.clone(),
                is_effect_block,
            });

            // Register only under the qualified name — short names collide across modules
            jit_func_ids.insert(
                qualified,
                JitFuncDecl {
                    func_id,
                    arity,
                    param_types,
                    return_type,
                },
            );
        }
    }
    if let Some(t0) = _pass1_t0 {
        eprintln!(
            "[AIVI_TIMING] {:40} {:>8.1}ms  ({} fns declared)",
            "JIT pass 1: declare functions",
            t0.elapsed().as_secs_f64() * 1000.0,
            declared_defs.len()
        );
    }

    // Pass 2: Compile function bodies.
    // Track successfully-compiled functions so later functions can direct-call them.
    struct PendingDef {
        name: String,
        qualified: String,
        func_id: cranelift_module::FuncId,
        arity: usize,
        is_effect_block: bool,
    }
    let mut pending: Vec<PendingDef> = Vec::new();
    let mut pending_lambdas: Vec<CompiledLambdaInfo> = Vec::new();
    let mut lambda_counter: usize = 0;
    let mut compiled_decls: HashMap<String, JitFuncDecl> = HashMap::new();
    let mut str_counter: usize = 0;

    let _pass2_t0 = if trace { Some(Instant::now()) } else { None };
    for dd in &declared_defs {
        // Pre-register so recursive self-calls resolve as direct JIT calls
        // instead of falling through to rt_get_global (which may find the
        // wrong function when the bare name is ambiguous across modules).
        compiled_decls.insert(
            dd.qualified.clone(),
            JitFuncDecl {
                func_id: dd.func_id,
                arity: dd.arity,
                param_types: dd.param_types.clone(),
                return_type: dd.return_type.clone(),
            },
        );
        match compile_definition_body(
            &mut module,
            &helpers,
            dd.def,
            &dd.module_name,
            &dd.qualified,
            dd.func_id,
            dd.arity,
            &dd.param_types,
            &dd.return_type,
            &compiled_decls,
            &mut lambda_counter,
            &spec_map,
            &mut str_counter,
            true,
        ) {
            Ok(lambdas) => {
                pending_lambdas.extend(lambdas);
                pending.push(PendingDef {
                    name: dd.def.name.clone(),
                    qualified: dd.qualified.clone(),
                    func_id: dd.func_id,
                    arity: dd.arity,
                    is_effect_block: dd.is_effect_block,
                });
            }
            Err(e) => {
                return Err(AiviError::runtime_message(format!(
                    "cranelift compile {}: {e}",
                    dd.qualified
                )))
            }
        }
    }
    if let Some(t0) = _pass2_t0 {
        eprintln!(
            "[AIVI_TIMING] {:40} {:>8.1}ms",
            "JIT pass 2: compile bodies",
            t0.elapsed().as_secs_f64() * 1000.0
        );
    }

    // Finalize all definitions at once, then extract pointers
    timed!(
        "finalize_definitions",
        module
            .finalize_definitions()
            .map_err(|e| AiviError::runtime_message(format!("cranelift finalize: {e}")))?
    );

    for pending_lambda in &pending_lambdas {
        let ptr = module.get_finalized_function(pending_lambda.func_id);
        let jit_value = make_jit_builtin(
            &pending_lambda.global_name,
            pending_lambda.total_arity,
            ptr as usize,
        );
        runtime
            .ctx
            .globals
            .set(pending_lambda.global_name.clone(), jit_value);
    }

    let mut compiled_globals: HashMap<String, Value> = HashMap::new();

    // Insert-or-merge: when the same name appears multiple times (multi-clause
    // domain operators), wrap all clauses in Value::MultiClause so the runtime
    // can try each clause in order via apply_multi_clause.
    fn insert_or_merge(map: &mut HashMap<String, Value>, key: String, value: Value) {
        use std::collections::hash_map::Entry;
        // Flatten: if `value` is itself a MultiClause, extract its inner clauses.
        let new_clauses: Vec<Value> = match value {
            Value::MultiClause(cs) => cs,
            other => vec![other],
        };
        match map.entry(key) {
            Entry::Vacant(e) => {
                if new_clauses.len() == 1 {
                    e.insert(new_clauses.into_iter().next().unwrap());
                } else {
                    e.insert(Value::MultiClause(new_clauses));
                }
            }
            Entry::Occupied(mut e) => {
                let existing = e.get_mut();
                match existing {
                    Value::MultiClause(clauses) => clauses.extend(new_clauses),
                    _ => {
                        let prev = std::mem::replace(existing, Value::Unit);
                        let mut all = vec![prev];
                        all.extend(new_clauses);
                        *existing = Value::MultiClause(all);
                    }
                }
            }
        }
    }

    // Pass A: register values by qualified name, merging multi-clause defs.
    // The kernel emits both a short def (name="(+)") and a qualified def
    // (name="aivi.duration.(+)") for each source def. Their pd.qualified values
    // are distinct, so insert_or_merge groups clauses correctly without
    // cross-contamination.
    for pd in &pending {
        let ptr = module.get_finalized_function(pd.func_id);
        if pd.is_effect_block {
            let def_name = pd.qualified.clone();
            let func_ptr = ptr as usize;
            let effect = Value::Effect(std::sync::Arc::new(
                crate::runtime::values::EffectValue::Thunk {
                    func: std::sync::Arc::new(move |runtime: &mut crate::runtime::Runtime| {
                        let ctx = unsafe { JitRuntimeCtx::from_runtime(runtime) };
                        let ctx_ptr = &ctx as *const JitRuntimeCtx as usize;
                        let call_args = [ctx_ptr as i64];
                        let result_ptr = unsafe { call_jit_function(func_ptr, &call_args) };
                        if result_ptr == 0 {
                            eprintln!("aivi: JIT effect '{}' returned null pointer", def_name);
                            Ok(Value::Unit)
                        } else {
                            let result =
                                unsafe { super::abi::unbox_value(result_ptr as *mut Value) };
                            match result {
                                Value::Effect(_) | Value::Source(_) => {
                                    runtime.run_effect_value(result)
                                }
                                other => Ok(other),
                            }
                        }
                    }),
                },
            ));
            insert_or_merge(&mut compiled_globals, pd.qualified.clone(), effect);
        } else {
            let jit_value = make_jit_builtin(&pd.qualified, pd.arity, ptr as usize);
            insert_or_merge(&mut compiled_globals, pd.qualified.clone(), jit_value);
        }
    }

    // Pass B: register short-name (bare) aliases.
    //
    // Three categories of bare names require different treatment:
    //
    //  • Domain operators — names like `(+)` that appear in multiple domains
    //    (calendar, duration, color, …) use `insert_or_merge` so each domain's
    //    implementation accumulates into a single MultiClause.
    //
    //  • HKT instance methods — MultiClauses from `aivi.logic.*` already
    //    contain all per-type clauses (List/Option/Result/Map).  They REPLACE
    //    whatever is currently at the bare name, and later non-HKT defs must
    //    not overwrite them.  Zero-arity HKT methods (e.g. `empty`) are
    //    skipped: they cannot dispatch on arguments, so users must qualify
    //    (List.empty, Map.empty).
    //
    //  • Everything else — last-writer-wins (plain insert), which is the
    //    historical default. Imported bare names should be qualified earlier by
    //    `resolve_import_names`, so this aliasing only serves truly bare global
    //    lookups.
    let mut seen_qualified: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut hkt_bare_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for pd in &pending {
        if !seen_qualified.insert(pd.qualified.clone()) {
            continue;
        }
        if let Some(value) = compiled_globals.get(&pd.qualified).cloned() {
            let is_hkt_bundle =
                pd.qualified.starts_with("aivi.logic.") && matches!(&value, Value::MultiClause(_));
            let is_operator = pd.name.starts_with('(') && pd.name.ends_with(')');
            if is_hkt_bundle {
                if pd.arity == 0 {
                    // Zero-arg HKT methods can't dispatch — skip bare name.
                    continue;
                }
                compiled_globals.insert(pd.name.clone(), value);
                hkt_bare_names.insert(pd.name.clone());
            } else if hkt_bare_names.contains(&pd.name) {
                // Bare name owned by HKT bundle — don't pollute it.
                continue;
            } else if is_operator {
                insert_or_merge(&mut compiled_globals, pd.name.clone(), value);
            } else {
                compiled_globals.insert(pd.name.clone(), value);
            }
        }
    }

    // Install compiled globals into the runtime.
    for (name, value) in compiled_globals {
        if !rebound_names.is_empty() {
            if let Some(existing) = runtime.ctx.globals.get(&name) {
                let is_rebound = rebound_names.contains(&name)
                    || name
                        .rsplit('.')
                        .next()
                        .is_some_and(|short| rebound_names.contains(short));
                let incoming_is_jit_arity0 = matches!(value, Value::Builtin(ref builtin)
                    if builtin.imp.arity == 0
                        && builtin.args.is_empty()
                        && builtin.imp.name.starts_with("__jit|"));
                let existing_is_jit_arity0 = matches!(existing, Value::Builtin(ref builtin)
                    if builtin.imp.arity == 0
                        && builtin.args.is_empty()
                        && builtin.imp.name.starts_with("__jit|"));
                if !is_rebound && incoming_is_jit_arity0 && !existing_is_jit_arity0 {
                    continue;
                }
            }
        }
        // Short (unqualified) names cannot shadow builtins, but qualified names
        // (e.g. `aivi.database.load`) coexist — they are looked up explicitly
        // when import resolution has rewritten a bare name to its qualified form.
        if !name.contains('.') {
            if let Some(existing) = runtime.ctx.globals.get(&name) {
                if matches!(existing, Value::Builtin(_) | Value::Record(_)) {
                    continue;
                }
            }
        }
        runtime.ctx.globals.set(name, value);
    }

    Ok(module)
}

/// Compile and execute an AIVI program entirely via Cranelift JIT.
///
/// This replaces `run_native_jit`: every definition is compiled to native
/// machine code, then `main` is executed.
pub fn run_cranelift_jit(
    program: HirProgram,
    cg_types: HashMap<String, HashMap<String, CgType>>,
    monomorph_plan: HashMap<String, Vec<CgType>>,
    source_schemas: HashMap<String, Vec<CgType>>,
    surface_modules: &[crate::surface::Module],
) -> Result<(), AiviError> {
    let crate_natives = crate::pm::native_bridge::collect_crate_natives(surface_modules);
    let crate_native_names = crate_natives
        .iter()
        .map(|binding| binding.aivi_name.clone())
        .collect::<Vec<_>>();
    run_cranelift_jit_prepared(
        program,
        cg_types,
        monomorph_plan,
        source_schemas,
        collect_surface_constructor_ordinals(surface_modules),
        crate_native_names,
    )
}

pub fn run_cranelift_jit_prepared(
    program: HirProgram,
    cg_types: HashMap<String, HashMap<String, CgType>>,
    monomorph_plan: HashMap<String, Vec<CgType>>,
    source_schemas: HashMap<String, Vec<CgType>>,
    constructor_ordinals: HashMap<String, Option<usize>>,
    crate_native_names: Vec<String>,
) -> Result<(), AiviError> {
    if !crate_native_names.is_empty() {
        return Err(AiviError::Codegen(format!(
            "E1527: crate-native binding(s) {} require `aivi build` (AOT). \
             They cannot run in JIT mode (`aivi run`).",
            crate_native_names.join(", ")
        )));
    }

    let trace = std::env::var("AIVI_TRACE_TIMING").is_ok_and(|v| v == "1");
    let t0 = if trace { Some(Instant::now()) } else { None };
    let mut runtime = build_runtime_from_program(&program)?;
    if let Some(ctx) = Arc::get_mut(&mut runtime.ctx) {
        ctx.merge_constructor_ordinals(constructor_ordinals);
    }
    let module = jit_compile_into_runtime(
        program,
        cg_types,
        monomorph_plan,
        source_schemas,
        &mut runtime,
        &HashSet::new(),
    )?;
    if let Some(t0) = t0 {
        eprintln!(
            "[AIVI_TIMING] {:40} {:>8.1}ms  ← TOTAL JIT",
            "JIT pipeline total",
            t0.elapsed().as_secs_f64() * 1000.0
        );
    }
    let result = run_main_effect(&mut runtime);
    drop(runtime);
    drop(module);
    result
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedBinding {
    pub value_text: String,
    pub was_effect: bool,
    pub effect_ran: bool,
    pub stdout_text: String,
    pub stderr_text: String,
}

pub struct ReplJitSession {
    source_signal_values: HashMap<String, Value>,
}

impl Default for ReplJitSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplJitSession {
    pub fn new() -> Self {
        Self {
            source_signal_values: HashMap::new(),
        }
    }

    pub fn reset(&mut self) {
        self.source_signal_values.clear();
    }

    pub fn forget_bindings(&mut self, binding_names: &[String]) {
        for name in binding_names {
            self.source_signal_values.remove(name);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_binding_detailed(
        &mut self,
        program: HirProgram,
        cg_types: HashMap<String, HashMap<String, CgType>>,
        monomorph_plan: HashMap<String, Vec<CgType>>,
        source_schemas: HashMap<String, Vec<CgType>>,
        surface_modules: &[crate::surface::Module],
        binding_name: &str,
        autorun_effects: bool,
        capture_binding_names: &[String],
    ) -> Result<EvaluatedBinding, AiviError> {
        let crate_natives = crate::pm::native_bridge::collect_crate_natives(surface_modules);
        if !crate_natives.is_empty() {
            let names: Vec<String> = crate_natives.iter().map(|b| b.aivi_name.clone()).collect();
            return Err(AiviError::Codegen(format!(
                "E1527: crate-native binding(s) {} require `aivi build` (AOT). \
                 They cannot run in JIT mode (`aivi run`).",
                names.join(", ")
            )));
        }

        let module_names: Vec<String> = program
            .modules
            .iter()
            .map(|module| module.name.clone())
            .collect();
        let mut runtime = build_runtime_from_program(&program)?;
        {
            let surface_ordinals = collect_surface_constructor_ordinals(surface_modules);
            if let Some(ctx) = Arc::get_mut(&mut runtime.ctx) {
                ctx.merge_constructor_ordinals(surface_ordinals);
            }
        }
        let module = jit_compile_into_runtime(
            program,
            cg_types,
            monomorph_plan,
            source_schemas,
            &mut runtime,
            &HashSet::new(),
        )?;
        let result = (|| {
            restore_repl_source_signals(&mut runtime, &module_names, &self.source_signal_values)
                .map_err(|err| runtime.aivi_runtime_error(err))?;
            runtime.clear_pending_runtime_error();

            let value = runtime.ctx.globals.get(binding_name).ok_or_else(|| {
                AiviError::runtime_message(format!("missing evaluated binding `{binding_name}`"))
            })?;
            let value = runtime
                .force_value(value)
                .map_err(|err| runtime.aivi_runtime_error(err))?;
            if let Some(err) = runtime.take_pending_aivi_error() {
                return Err(err);
            }

            let was_effect = matches!(value, Value::Effect(_));
            if autorun_effects && was_effect {
                runtime.ctx.begin_console_capture();
                let result = runtime
                    .run_effect_value(value)
                    .map_err(|err| runtime.aivi_runtime_error(err))?;
                let capture = runtime.ctx.take_console_capture();
                if let Some(err) = runtime.take_pending_aivi_error() {
                    return Err(err);
                }
                let binding = EvaluatedBinding {
                    value_text: format_value(&result),
                    was_effect: true,
                    effect_ran: true,
                    stdout_text: capture.stdout,
                    stderr_text: capture.stderr,
                };
                self.source_signal_values =
                    capture_repl_source_signals(&mut runtime, &module_names, capture_binding_names)
                        .map_err(|err| runtime.aivi_runtime_error(err))?;
                Ok(binding)
            } else {
                let binding = EvaluatedBinding {
                    value_text: format_value(&value),
                    was_effect,
                    effect_ran: false,
                    stdout_text: String::new(),
                    stderr_text: String::new(),
                };
                self.source_signal_values =
                    capture_repl_source_signals(&mut runtime, &module_names, capture_binding_names)
                        .map_err(|err| runtime.aivi_runtime_error(err))?;
                Ok(binding)
            }
        })();
        drop(runtime);
        drop(module);
        result
    }
}

fn repl_binding_candidates(module_names: &[String], binding_name: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    for module_name in module_names {
        let qualified = format!("{module_name}.{binding_name}");
        if !candidates.contains(&qualified) {
            candidates.push(qualified);
        }
    }
    candidates.push(binding_name.to_string());
    candidates
}

fn force_repl_binding(
    runtime: &mut Runtime,
    candidates: &[String],
) -> Result<Option<Value>, RuntimeError> {
    let Some(value) = candidates
        .iter()
        .find_map(|candidate| runtime.ctx.globals.get(candidate))
    else {
        return Ok(None);
    };
    let forced = runtime.force_value(value)?;
    for candidate in candidates {
        if runtime.ctx.globals.get(candidate).is_some() {
            runtime.ctx.globals.set(candidate.clone(), forced.clone());
        }
    }
    Ok(Some(forced))
}

fn is_source_signal(runtime: &Runtime, value: &Value) -> bool {
    let Value::Signal(signal) = value else {
        return false;
    };
    let graph = runtime.reactive_graph.lock();
    matches!(
        graph.signals.get(&signal.id).map(|entry| &entry.kind),
        Some(ReactiveCellKind::Source)
    )
}

fn restore_repl_source_signals(
    runtime: &mut Runtime,
    module_names: &[String],
    stored_values: &HashMap<String, Value>,
) -> Result<(), RuntimeError> {
    for (binding_name, stored_value) in stored_values {
        let candidates = repl_binding_candidates(module_names, binding_name);
        let Some(signal) = force_repl_binding(runtime, &candidates)? else {
            continue;
        };
        if !is_source_signal(runtime, &signal) {
            continue;
        }
        runtime.reactive_set_signal(signal, stored_value.clone())?;
    }
    Ok(())
}

fn capture_repl_source_signals(
    runtime: &mut Runtime,
    module_names: &[String],
    binding_names: &[String],
) -> Result<HashMap<String, Value>, RuntimeError> {
    let mut captured = HashMap::new();
    for binding_name in binding_names {
        let candidates = repl_binding_candidates(module_names, binding_name);
        let Some(signal) = force_repl_binding(runtime, &candidates)? else {
            continue;
        };
        if !is_source_signal(runtime, &signal) {
            continue;
        }
        let value = runtime.reactive_peek_signal(signal)?;
        captured.insert(binding_name.clone(), value);
    }
    Ok(captured)
}

fn evaluate_binding_jit_internal(
    program: HirProgram,
    cg_types: HashMap<String, HashMap<String, CgType>>,
    monomorph_plan: HashMap<String, Vec<CgType>>,
    source_schemas: HashMap<String, Vec<CgType>>,
    surface_modules: &[crate::surface::Module],
    binding_name: &str,
    autorun_effects: bool,
) -> Result<EvaluatedBinding, AiviError> {
    let mut session = ReplJitSession::new();
    session.evaluate_binding_detailed(
        program,
        cg_types,
        monomorph_plan,
        source_schemas,
        surface_modules,
        binding_name,
        autorun_effects,
        &[],
    )
}

/// JIT-compile a program and return the formatted runtime value of one binding.
pub fn evaluate_binding_jit(
    program: HirProgram,
    cg_types: HashMap<String, HashMap<String, CgType>>,
    monomorph_plan: HashMap<String, Vec<CgType>>,
    source_schemas: HashMap<String, Vec<CgType>>,
    surface_modules: &[crate::surface::Module],
    binding_name: &str,
) -> Result<String, AiviError> {
    Ok(evaluate_binding_jit_internal(
        program,
        cg_types,
        monomorph_plan,
        source_schemas,
        surface_modules,
        binding_name,
        false,
    )?
    .value_text)
}

pub fn evaluate_binding_jit_detailed(
    program: HirProgram,
    cg_types: HashMap<String, HashMap<String, CgType>>,
    monomorph_plan: HashMap<String, Vec<CgType>>,
    source_schemas: HashMap<String, Vec<CgType>>,
    surface_modules: &[crate::surface::Module],
    binding_name: &str,
    autorun_effects: bool,
) -> Result<EvaluatedBinding, AiviError> {
    evaluate_binding_jit_internal(
        program,
        cg_types,
        monomorph_plan,
        source_schemas,
        surface_modules,
        binding_name,
        autorun_effects,
    )
}

/// Like [`run_cranelift_jit`] but accepts an external cancel token so the
/// caller can cancel execution from another thread (used by `--watch`).
pub(crate) fn run_cranelift_jit_cancellable(
    program: HirProgram,
    cg_types: HashMap<String, HashMap<String, CgType>>,
    monomorph_plan: HashMap<String, Vec<CgType>>,
    source_schemas: HashMap<String, Vec<CgType>>,
    cancel: Arc<CancelToken>,
    surface_modules: &[crate::surface::Module],
) -> Result<(), AiviError> {
    // E1527: crate-native bindings require AOT build
    let crate_natives = crate::pm::native_bridge::collect_crate_natives(surface_modules);
    if !crate_natives.is_empty() {
        let names: Vec<String> = crate_natives.iter().map(|b| b.aivi_name.clone()).collect();
        return Err(AiviError::Codegen(format!(
            "E1527: crate-native binding(s) {} require `aivi build` (AOT). \
             They cannot run in JIT mode (`aivi run`).",
            names.join(", ")
        )));
    }

    let mut runtime = build_runtime_from_program_with_cancel(&program, cancel)?;
    {
        let surface_ordinals = collect_surface_constructor_ordinals(surface_modules);
        if let Some(ctx) = Arc::get_mut(&mut runtime.ctx) {
            ctx.merge_constructor_ordinals(surface_ordinals);
        }
    }
    let module = jit_compile_into_runtime(
        program,
        cg_types,
        monomorph_plan,
        source_schemas,
        &mut runtime,
        &HashSet::new(),
    )?;
    let result = run_main_effect(&mut runtime);
    drop(runtime);
    drop(module);
    result
}

/// JIT-compile an AIVI program and run its test suite.
///
/// Like `run_cranelift_jit` but executes the named test entries instead of `main`.
pub fn run_test_suite_jit(
    program: HirProgram,
    test_entries: &[(String, String)],
    surface_modules: &[crate::surface::Module],
    update_snapshots: bool,
    project_root: Option<std::path::PathBuf>,
) -> Result<crate::runtime::TestReport, AiviError> {
    use crate::runtime::{format_value, TestFailure, TestReport, TestSuccess};

    let infer_result = aivi_core::infer_value_types_full(surface_modules);
    let mut runtime = build_runtime_from_program(&program)?;
    // Register user-defined constructor ordinals so constructorOrdinal/constructorName
    // work for ADTs declared in the program (not just core types).
    {
        let surface_ordinals = collect_surface_constructor_ordinals(surface_modules);
        if let Some(ctx) = Arc::get_mut(&mut runtime.ctx) {
            ctx.merge_constructor_ordinals(surface_ordinals);
        }
    }
    runtime.update_snapshots = update_snapshots;
    runtime.project_root = project_root;
    let _module = jit_compile_into_runtime(
        program,
        infer_result.cg_types,
        infer_result.monomorph_plan,
        infer_result.source_schemas,
        &mut runtime,
        &HashSet::new(),
    )?;
    runtime.clear_pending_runtime_error();

    const TEST_FUEL_BUDGET: u64 = 500_000;
    let mut report = TestReport {
        passed: 0,
        failed: 0,
        failures: Vec::new(),
        successes: Vec::new(),
    };

    for (name, description) in test_entries {
        runtime.fuel = Some(TEST_FUEL_BUDGET);
        runtime.current_test_name = Some(name.clone());
        runtime.snapshot_recordings.clear();
        runtime.snapshot_replay_cursors.clear();
        runtime.snapshot_failure = None;
        let Some(value) = runtime.ctx.globals.get(name) else {
            report.failed += 1;
            report.failures.push(TestFailure {
                name: name.clone(),
                description: description.clone(),
                message: "missing definition".to_string(),
            });
            continue;
        };

        runtime.clear_pending_runtime_error();

        let value = match runtime.force_value(value) {
            Ok(value) => value,
            Err(err) => {
                report.failed += 1;
                report.failures.push(TestFailure {
                    name: name.clone(),
                    description: description.clone(),
                    message: aivi_driver::render_runtime_report(
                        &runtime.runtime_report(err),
                        false,
                    ),
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

        runtime.clear_pending_runtime_error();

        match runtime.run_effect_value(effect) {
            Ok(_) => {
                if let Some(msg) = runtime.snapshot_failure.take() {
                    runtime.clear_pending_runtime_error();
                    report.failed += 1;
                    report.failures.push(TestFailure {
                        name: name.clone(),
                        description: description.clone(),
                        message: msg,
                    });
                } else if let Some((err, snapshot)) = runtime.take_pending_runtime_error() {
                    report.failed += 1;
                    report.failures.push(TestFailure {
                        name: name.clone(),
                        description: description.clone(),
                        message: aivi_driver::render_runtime_report(
                            &runtime.runtime_report_with_snapshot(err, snapshot),
                            false,
                        ),
                    });
                } else {
                    report.passed += 1;
                    report.successes.push(TestSuccess {
                        name: name.clone(),
                        description: description.clone(),
                    });
                }
            }
            Err(err) => {
                runtime.clear_pending_runtime_error();
                report.failed += 1;
                report.failures.push(TestFailure {
                    name: name.clone(),
                    description: description.clone(),
                    message: aivi_driver::render_runtime_report(
                        &runtime.runtime_report(err),
                        false,
                    ),
                });
            }
        }
    }

    let result = Ok(report);
    drop(runtime);
    result
}

/// Walk all RustIR modules and inject recorded source schemas both at `load`
/// boundaries and at file source constructor call sites.
fn inject_source_schemas(
    modules: &mut [rust_ir::RustIrModule],
    source_schemas: &HashMap<String, Vec<CgType>>,
) {
    for module in modules.iter_mut() {
        for def in &mut module.defs {
            let module_key = format!("{}.{}", module.name, def.name);
            let load_schemas = source_schemas.get(&module_key).or_else(|| {
                def.name
                    .contains('.')
                    .then(|| source_schemas.get(&def.name))
                    .flatten()
            });
            let source_site_key = format!("{module_key}{SOURCE_CONSTRUCTOR_SCHEMA_SUFFIX}");
            let source_site_schemas = source_schemas.get(&source_site_key).or_else(|| {
                def.name
                    .contains('.')
                    .then(|| {
                        source_schemas
                            .get(&format!("{}{SOURCE_CONSTRUCTOR_SCHEMA_SUFFIX}", def.name))
                    })
                    .flatten()
            });
            if load_schemas.is_some() || source_site_schemas.is_some() {
                let mut load_idx = 0;
                let mut source_site_idx = 0;
                inject_in_expr(
                    &mut def.expr,
                    load_schemas.map_or(&[], Vec::as_slice),
                    &mut load_idx,
                    source_site_schemas.map_or(&[], Vec::as_slice),
                    &mut source_site_idx,
                );
            }
        }
    }
}

/// Recursively walk a RustIR expression. `load` applications and file source
/// constructor calls are wrapped with `__set_source_schema` using the
/// typechecker-recorded schema order.
fn inject_in_expr(
    expr: &mut RustIrExpr,
    load_schemas: &[CgType],
    load_idx: &mut usize,
    source_site_schemas: &[CgType],
    source_site_idx: &mut usize,
) {
    // First recurse into children so inner source sites are found in order.
    match expr {
        RustIrExpr::Lambda { body, .. } => inject_in_expr(
            body,
            load_schemas,
            load_idx,
            source_site_schemas,
            source_site_idx,
        ),
        RustIrExpr::App { func, arg, .. } => {
            inject_in_expr(
                func,
                load_schemas,
                load_idx,
                source_site_schemas,
                source_site_idx,
            );
            inject_in_expr(
                arg,
                load_schemas,
                load_idx,
                source_site_schemas,
                source_site_idx,
            );
        }
        RustIrExpr::Call { func, args, .. } => {
            inject_in_expr(
                func,
                load_schemas,
                load_idx,
                source_site_schemas,
                source_site_idx,
            );
            for a in args.iter_mut() {
                inject_in_expr(
                    a,
                    load_schemas,
                    load_idx,
                    source_site_schemas,
                    source_site_idx,
                );
            }
        }
        RustIrExpr::List { items, .. } => {
            for item in items {
                inject_in_expr(
                    &mut item.expr,
                    load_schemas,
                    load_idx,
                    source_site_schemas,
                    source_site_idx,
                );
            }
        }
        RustIrExpr::Tuple { items, .. } => {
            for item in items {
                inject_in_expr(
                    item,
                    load_schemas,
                    load_idx,
                    source_site_schemas,
                    source_site_idx,
                );
            }
        }
        RustIrExpr::Record { fields, .. } => {
            for field in fields {
                inject_in_expr(
                    &mut field.value,
                    load_schemas,
                    load_idx,
                    source_site_schemas,
                    source_site_idx,
                );
            }
        }
        RustIrExpr::Patch { target, fields, .. } => {
            inject_in_expr(
                target,
                load_schemas,
                load_idx,
                source_site_schemas,
                source_site_idx,
            );
            for field in fields {
                inject_in_expr(
                    &mut field.value,
                    load_schemas,
                    load_idx,
                    source_site_schemas,
                    source_site_idx,
                );
            }
        }
        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            inject_in_expr(
                scrutinee,
                load_schemas,
                load_idx,
                source_site_schemas,
                source_site_idx,
            );
            for arm in arms {
                inject_in_expr(
                    &mut arm.body,
                    load_schemas,
                    load_idx,
                    source_site_schemas,
                    source_site_idx,
                );
            }
        }
        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            inject_in_expr(
                cond,
                load_schemas,
                load_idx,
                source_site_schemas,
                source_site_idx,
            );
            inject_in_expr(
                then_branch,
                load_schemas,
                load_idx,
                source_site_schemas,
                source_site_idx,
            );
            inject_in_expr(
                else_branch,
                load_schemas,
                load_idx,
                source_site_schemas,
                source_site_idx,
            );
        }
        RustIrExpr::Binary { left, right, .. } => {
            inject_in_expr(
                left,
                load_schemas,
                load_idx,
                source_site_schemas,
                source_site_idx,
            );
            inject_in_expr(
                right,
                load_schemas,
                load_idx,
                source_site_schemas,
                source_site_idx,
            );
        }
        RustIrExpr::TextInterpolate { parts, .. } => {
            for part in parts {
                if let RustIrTextPart::Expr { expr: e } = part {
                    inject_in_expr(
                        e,
                        load_schemas,
                        load_idx,
                        source_site_schemas,
                        source_site_idx,
                    );
                }
            }
        }
        RustIrExpr::DebugFn { body, .. } => inject_in_expr(
            body,
            load_schemas,
            load_idx,
            source_site_schemas,
            source_site_idx,
        ),
        RustIrExpr::Pipe { func, arg, .. } => {
            inject_in_expr(
                func,
                load_schemas,
                load_idx,
                source_site_schemas,
                source_site_idx,
            );
            inject_in_expr(
                arg,
                load_schemas,
                load_idx,
                source_site_schemas,
                source_site_idx,
            );
        }
        RustIrExpr::FieldAccess { base, .. } | RustIrExpr::Index { base, .. } => {
            inject_in_expr(
                base,
                load_schemas,
                load_idx,
                source_site_schemas,
                source_site_idx,
            );
        }
        RustIrExpr::Mock { body, .. } => inject_in_expr(
            body,
            load_schemas,
            load_idx,
            source_site_schemas,
            source_site_idx,
        ),
        // Leaves: Local, Global, Builtin, ConstructorValue, Lit*, Raw, etc.
        _ => {}
    }

    if is_file_source_call(expr) {
        if let Some(cg_type) = source_site_schemas.get(*source_site_idx) {
            wrap_expr_with_schema(expr, cg_type);
            *source_site_idx += 1;
        }
    }

    // After recursing, check if this node is a `load` application.
    let is_load_app = match expr {
        RustIrExpr::App { func, .. } => matches!(
            func.as_ref(),
            RustIrExpr::Global { name, .. } | RustIrExpr::Builtin { builtin: name, .. }
                if name == "load"
        ),
        RustIrExpr::Call { func, args, .. } if args.len() == 1 => matches!(
            func.as_ref(),
            RustIrExpr::Global { name, .. } | RustIrExpr::Builtin { builtin: name, .. }
                if name == "load"
        ),
        _ => false,
    };

    if is_load_app {
        if let Some(cg_type) = load_schemas.get(*load_idx) {
            match expr {
                RustIrExpr::App { arg, .. } => wrap_boxed_source_expr(arg, cg_type),
                RustIrExpr::Call { args, .. } if args.len() == 1 => {
                    wrap_list_source_expr(&mut args[0], cg_type);
                }
                _ => {}
            }
        }
        *load_idx += 1;
    }
}

fn is_file_source_call(expr: &RustIrExpr) -> bool {
    match expr {
        RustIrExpr::App { func, .. } => is_file_source_callee(func.as_ref()),
        RustIrExpr::Call { func, args, .. } if args.len() == 1 => {
            is_file_source_callee(func.as_ref())
        }
        _ => false,
    }
}

fn is_file_source_callee(expr: &RustIrExpr) -> bool {
    match expr {
        RustIrExpr::Global { name, .. } => matches!(name.as_str(), "file.json" | "file.csv"),
        RustIrExpr::Builtin { builtin, .. } => {
            matches!(builtin.as_str(), "file.json" | "file.csv")
        }
        _ => false,
    }
}

fn wrap_expr_with_schema(expr: &mut RustIrExpr, cg_type: &CgType) {
    if *cg_type == CgType::Dynamic {
        return;
    }
    let schema = cg_type_to_json_schema(cg_type);
    let Ok(schema_json) = serde_json::to_string(&schema) else {
        return;
    };
    let original = std::mem::replace(
        expr,
        RustIrExpr::LitBool {
            id: 0,
            value: false,
        },
    );
    *expr = wrap_source_expr(original, schema_json);
}

fn wrap_boxed_source_expr(expr: &mut Box<RustIrExpr>, cg_type: &CgType) {
    if *cg_type == CgType::Dynamic {
        return;
    }
    let schema = cg_type_to_json_schema(cg_type);
    let Ok(schema_json) = serde_json::to_string(&schema) else {
        return;
    };
    let original = std::mem::replace(
        expr,
        Box::new(RustIrExpr::LitBool {
            id: 0,
            value: false,
        }),
    );
    **expr = wrap_source_expr(*original, schema_json);
}

fn wrap_list_source_expr(expr: &mut RustIrExpr, cg_type: &CgType) {
    if *cg_type == CgType::Dynamic {
        return;
    }
    let schema = cg_type_to_json_schema(cg_type);
    let Ok(schema_json) = serde_json::to_string(&schema) else {
        return;
    };
    let original = std::mem::replace(
        expr,
        RustIrExpr::LitBool {
            id: 0,
            value: false,
        },
    );
    *expr = wrap_source_expr(original, schema_json);
}

fn wrap_source_expr(source: RustIrExpr, schema_json: String) -> RustIrExpr {
    RustIrExpr::Call {
        id: 0,
        func: Box::new(RustIrExpr::Global {
            id: 0,
            name: "__set_source_schema".to_string(),
            location: None,
        }),
        args: vec![
            RustIrExpr::LitString {
                id: 0,
                text: schema_json,
            },
            source,
        ],
        location: None,
    }
}

include!("compile/support.rs");
include!("compile/body.rs");
include!("compile/aot.rs");

#[cfg(test)]
mod runtime_warning_tests {
    use std::collections::{HashMap, HashSet};
    use std::path::Path;

    use super::*;
    use crate::hir::{HirDef, HirExpr, HirModule, HirProgram};
    use crate::surface::parse_modules;
    use crate::{Position, SourceOrigin, Span};

    fn origin(path: &str, line: usize, column: usize) -> SourceOrigin {
        SourceOrigin::new(
            path.to_string(),
            Span {
                start: Position { line, column },
                end: Position {
                    line,
                    column: column + 1,
                },
            },
        )
    }

    #[test]
    fn missing_global_warning_reports_source_location() {
        let program = HirProgram {
            modules: vec![HirModule {
                name: "demo.main".to_string(),
                defs: vec![
                    HirDef {
                        name: "helper".to_string(),
                        location: None,
                        expr: HirExpr::LitBool { id: 1, value: true },
                    },
                    HirDef {
                        name: "main".to_string(),
                        location: None,
                        expr: HirExpr::Binary {
                            id: 2,
                            op: "+".to_string(),
                            left: Box::new(HirExpr::Var {
                                id: 3,
                                name: "helper".to_string(),
                                location: Some(origin("src/demo/main.aivi", 7, 11)),
                            }),
                            right: Box::new(HirExpr::LitNumber {
                                id: 4,
                                text: "1".to_string(),
                            }),
                            location: None,
                        },
                    },
                ],
            }],
        };

        let mut runtime = build_runtime_from_program(&program).expect("build runtime");
        let module = jit_compile_into_runtime(
            program,
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            &mut runtime,
            &HashSet::new(),
        )
        .expect("compile runtime");

        runtime.ctx.globals.remove("demo.main.helper");
        runtime.ctx.begin_console_capture();
        let value = runtime
            .ctx
            .globals
            .get("demo.main.main")
            .expect("main binding should exist");
        let _ = runtime.force_value(value).unwrap_or_else(|err| {
            panic!(
                "force main binding: {}",
                crate::runtime::format_runtime_error(err)
            )
        });
        let capture = runtime.ctx.take_console_capture();

        assert!(
            capture.stderr.contains("warning[RT]"),
            "expected runtime warning, got stderr:\n{}",
            capture.stderr
        );
        assert!(
            capture.stderr.contains("src/demo/main.aivi:7:11"),
            "expected source location in warning, got stderr:\n{}",
            capture.stderr
        );

        drop(module);
    }

    fn lit_int(id: u32, text: &str) -> HirExpr {
        HirExpr::LitNumber {
            id,
            text: text.to_string(),
        }
    }

    fn tuple_else_branch(start_id: u32) -> HirExpr {
        HirExpr::Tuple {
            id: start_id,
            items: vec![
                lit_int(start_id + 1, "2"),
                lit_int(start_id + 2, "3"),
                lit_int(start_id + 3, "4"),
                lit_int(start_id + 4, "5"),
                lit_int(start_id + 5, "6"),
                lit_int(start_id + 6, "7"),
                lit_int(start_id + 7, "8"),
                lit_int(start_id + 8, "9"),
            ],
        }
    }

    #[test]
    fn runtime_stack_frames_inherit_pending_call_locations() {
        let program = HirProgram {
            modules: vec![HirModule {
                name: "demo.main".to_string(),
                defs: vec![
                    HirDef {
                        name: "helper".to_string(),
                        location: None,
                        expr: HirExpr::Lambda {
                            id: 10,
                            param: "ignored".to_string(),
                            body: Box::new(HirExpr::If {
                                id: 11,
                                cond: Box::new(HirExpr::LitBool {
                                    id: 12,
                                    value: true,
                                }),
                                then_branch: Box::new(HirExpr::App {
                                    id: 13,
                                    func: Box::new(HirExpr::LitBool {
                                        id: 14,
                                        value: true,
                                    }),
                                    arg: Box::new(lit_int(15, "1")),
                                    location: None,
                                }),
                                else_branch: Box::new(tuple_else_branch(20)),
                                location: None,
                            }),
                            location: None,
                        },
                    },
                    HirDef {
                        name: "apply".to_string(),
                        location: None,
                        expr: HirExpr::Lambda {
                            id: 40,
                            param: "f".to_string(),
                            body: Box::new(HirExpr::If {
                                id: 41,
                                cond: Box::new(HirExpr::LitBool {
                                    id: 42,
                                    value: true,
                                }),
                                then_branch: Box::new(HirExpr::App {
                                    id: 43,
                                    func: Box::new(HirExpr::Var {
                                        id: 44,
                                        name: "f".to_string(),
                                        location: None,
                                    }),
                                    arg: Box::new(lit_int(45, "1")),
                                    location: Some(origin("src/demo/main.aivi", 5, 11)),
                                }),
                                else_branch: Box::new(tuple_else_branch(50)),
                                location: None,
                            }),
                            location: None,
                        },
                    },
                    HirDef {
                        name: "entry".to_string(),
                        location: None,
                        expr: HirExpr::Lambda {
                            id: 70,
                            param: "_".to_string(),
                            body: Box::new(HirExpr::App {
                                id: 71,
                                func: Box::new(HirExpr::Var {
                                    id: 72,
                                    name: "apply".to_string(),
                                    location: Some(origin("src/demo/main.aivi", 8, 5)),
                                }),
                                arg: Box::new(HirExpr::Lambda {
                                    id: 73,
                                    param: "x".to_string(),
                                    body: Box::new(HirExpr::App {
                                        id: 74,
                                        func: Box::new(HirExpr::LitBool {
                                            id: 75,
                                            value: true,
                                        }),
                                        arg: Box::new(HirExpr::LitNumber {
                                            id: 76,
                                            text: "1".to_string(),
                                        }),
                                        location: None,
                                    }),
                                    location: None,
                                }),
                                location: None,
                            }),
                            location: None,
                        },
                    },
                    HirDef {
                        name: "safe".to_string(),
                        location: None,
                        expr: HirExpr::Lambda {
                            id: 90,
                            param: "value".to_string(),
                            body: Box::new(HirExpr::Var {
                                id: 91,
                                name: "value".to_string(),
                                location: Some(origin("src/demo/main.aivi", 11, 9)),
                            }),
                            location: None,
                        },
                    },
                ],
            }],
        };

        let mut runtime = build_runtime_from_program(&program).expect("build runtime");
        let module = jit_compile_into_runtime(
            program,
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            &mut runtime,
            &HashSet::new(),
        )
        .expect("compile runtime");

        let value = runtime
            .ctx
            .globals
            .get("demo.main.entry")
            .expect("entry binding should exist")
            .clone();
        let err = runtime
            .apply(value, Value::Unit)
            .expect_err("entry should fail");
        let safe = runtime
            .ctx
            .globals
            .get("demo.main.safe")
            .expect("safe binding should exist")
            .clone();
        let safe_result = runtime.apply(safe, Value::Int(1)).unwrap_or_else(|err| {
            panic!(
                "safe call after failure: {}",
                crate::runtime::format_runtime_error(err)
            )
        });
        match safe_result {
            Value::Int(1) => {}
            other => panic!("expected safe call to return 1, got {other:?}"),
        }
        let rendered = aivi_driver::render_runtime_report(&runtime.runtime_report(err), false);

        assert!(
            rendered.contains("demo.main.entry (lambda) at src/demo/main.aivi:5:11"),
            "expected lambda frame location, got:\n{rendered}"
        );

        drop(module);
    }

    #[test]
    fn runtime_stack_frames_fallback_to_definition_and_lambda_locations() {
        let program = HirProgram {
            modules: vec![HirModule {
                name: "demo.main".to_string(),
                defs: vec![
                    HirDef {
                        name: "zero".to_string(),
                        location: Some(origin("src/demo/main.aivi", 3, 1)),
                        expr: HirExpr::App {
                            id: 1,
                            func: Box::new(HirExpr::LitBool { id: 2, value: true }),
                            arg: Box::new(lit_int(3, "1")),
                            location: None,
                        },
                    },
                    HirDef {
                        name: "makeClosure".to_string(),
                        location: Some(origin("src/demo/main.aivi", 6, 1)),
                        expr: HirExpr::Lambda {
                            id: 10,
                            param: "_".to_string(),
                            body: Box::new(HirExpr::If {
                                id: 11,
                                cond: Box::new(HirExpr::LitBool {
                                    id: 12,
                                    value: true,
                                }),
                                then_branch: Box::new(HirExpr::Lambda {
                                    id: 13,
                                    param: "value".to_string(),
                                    body: Box::new(HirExpr::App {
                                        id: 14,
                                        func: Box::new(HirExpr::LitBool {
                                            id: 15,
                                            value: true,
                                        }),
                                        arg: Box::new(lit_int(16, "1")),
                                        location: None,
                                    }),
                                    location: Some(origin("src/demo/main.aivi", 7, 5)),
                                }),
                                else_branch: Box::new(HirExpr::Lambda {
                                    id: 17,
                                    param: "value".to_string(),
                                    body: Box::new(HirExpr::Var {
                                        id: 18,
                                        name: "value".to_string(),
                                        location: None,
                                    }),
                                    location: Some(origin("src/demo/main.aivi", 8, 5)),
                                }),
                                location: None,
                            }),
                            location: None,
                        },
                    },
                ],
            }],
        };

        let mut runtime = build_runtime_from_program(&program).expect("build runtime");
        let module = jit_compile_into_runtime(
            program,
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            &mut runtime,
            &HashSet::new(),
        )
        .expect("compile runtime");

        let zero = runtime
            .ctx
            .globals
            .get("demo.main.zero")
            .expect("zero binding should exist");
        let zero_err = runtime.force_value(zero).expect_err("zero should fail");
        let zero_rendered =
            aivi_driver::render_runtime_report(&runtime.runtime_report(zero_err), false);
        assert!(
            zero_rendered.contains("demo.main.zero at src/demo/main.aivi:3:1"),
            "expected nullary def fallback location, got:\n{zero_rendered}"
        );

        let make_closure = runtime
            .ctx
            .globals
            .get("demo.main.makeClosure")
            .expect("makeClosure binding should exist")
            .clone();
        let closure = runtime
            .apply(make_closure, Value::Unit)
            .unwrap_or_else(|err| {
                panic!(
                    "makeClosure should return a closure, got error: {}",
                    crate::runtime::format_runtime_error(err)
                )
            });
        let lambda_err = runtime
            .apply(closure, Value::Int(1))
            .expect_err("returned closure should fail");
        let lambda_rendered =
            aivi_driver::render_runtime_report(&runtime.runtime_report(lambda_err), false);
        assert!(
            lambda_rendered.contains("demo.main.makeClosure (lambda) at src/demo/main.aivi:7:5"),
            "expected lambda fallback location, got:\n{lambda_rendered}"
        );

        drop(module);
    }

    #[test]
    fn jit_errors_keep_renderable_context_after_later_jit_calls() {
        let program = HirProgram {
            modules: vec![HirModule {
                name: "demo.main".to_string(),
                defs: vec![
                    HirDef {
                        name: "helper".to_string(),
                        location: None,
                        expr: HirExpr::Lambda {
                            id: 10,
                            param: "ignored".to_string(),
                            body: Box::new(HirExpr::If {
                                id: 11,
                                cond: Box::new(HirExpr::LitBool {
                                    id: 12,
                                    value: true,
                                }),
                                then_branch: Box::new(HirExpr::App {
                                    id: 13,
                                    func: Box::new(HirExpr::LitBool {
                                        id: 14,
                                        value: true,
                                    }),
                                    arg: Box::new(lit_int(15, "1")),
                                    location: None,
                                }),
                                else_branch: Box::new(tuple_else_branch(20)),
                                location: None,
                            }),
                            location: None,
                        },
                    },
                    HirDef {
                        name: "entry".to_string(),
                        location: None,
                        expr: HirExpr::Lambda {
                            id: 40,
                            param: "_".to_string(),
                            body: Box::new(HirExpr::App {
                                id: 41,
                                func: Box::new(HirExpr::Var {
                                    id: 42,
                                    name: "helper".to_string(),
                                    location: Some(origin("src/demo/main.aivi", 6, 7)),
                                }),
                                arg: Box::new(HirExpr::LitBool {
                                    id: 43,
                                    value: true,
                                }),
                                location: None,
                            }),
                            location: None,
                        },
                    },
                    HirDef {
                        name: "safe".to_string(),
                        location: None,
                        expr: HirExpr::Lambda {
                            id: 50,
                            param: "value".to_string(),
                            body: Box::new(HirExpr::Var {
                                id: 51,
                                name: "value".to_string(),
                                location: Some(origin("src/demo/main.aivi", 9, 9)),
                            }),
                            location: None,
                        },
                    },
                ],
            }],
        };

        let mut runtime = build_runtime_from_program(&program).expect("build runtime");
        let module = jit_compile_into_runtime(
            program,
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            &mut runtime,
            &HashSet::new(),
        )
        .expect("compile runtime");

        let entry = runtime
            .ctx
            .globals
            .get("demo.main.entry")
            .expect("entry binding should exist")
            .clone();
        let err = runtime
            .apply(entry, Value::Unit)
            .expect_err("entry should fail");
        let safe = runtime
            .ctx
            .globals
            .get("demo.main.safe")
            .expect("safe binding should exist")
            .clone();
        let safe_result = runtime.apply(safe, Value::Int(1)).unwrap_or_else(|err| {
            panic!(
                "safe call after failure: {}",
                crate::runtime::format_runtime_error(err)
            )
        });
        match safe_result {
            Value::Int(1) => {}
            other => panic!("expected safe call to return 1, got {other:?}"),
        }

        let rendered = crate::runtime::format_runtime_error(err);
        assert!(
            rendered.contains("src/demo/main.aivi:6:7"),
            "expected call-site location to survive later JIT work, got:\n{rendered}"
        );
        assert!(
            rendered.contains("1: demo.main.entry"),
            "expected outer user frame to survive later JIT work, got:\n{rendered}"
        );

        drop(module);
    }

    #[test]
    fn if_condition_warning_uses_if_node_location() {
        let program = HirProgram {
            modules: vec![HirModule {
                name: "demo.main".to_string(),
                defs: vec![HirDef {
                    name: "main".to_string(),
                    location: None,
                    expr: HirExpr::If {
                        id: 1,
                        cond: Box::new(HirExpr::Tuple {
                            id: 2,
                            items: vec![lit_int(3, "1"), lit_int(4, "2")],
                        }),
                        then_branch: Box::new(lit_int(5, "1")),
                        else_branch: Box::new(lit_int(6, "0")),
                        location: Some(origin("src/demo/main.aivi", 3, 5)),
                    },
                }],
            }],
        };

        let mut runtime = build_runtime_from_program(&program).expect("build runtime");
        let module = jit_compile_into_runtime(
            program,
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            &mut runtime,
            &HashSet::new(),
        )
        .expect("compile runtime");

        runtime.ctx.begin_console_capture();
        let value = runtime
            .ctx
            .globals
            .get("demo.main.main")
            .expect("main binding should exist");
        let _ = runtime.force_value(value).unwrap_or_else(|err| {
            panic!(
                "force main binding: {}",
                crate::runtime::format_runtime_error(err)
            )
        });
        let capture = runtime.ctx.take_console_capture();

        assert!(
            capture.stderr.contains("warning[RT]"),
            "expected runtime warning, got stderr:\n{}",
            capture.stderr
        );
        assert!(
            capture.stderr.contains("src/demo/main.aivi:3:5"),
            "expected if-node source location in warning, got stderr:\n{}",
            capture.stderr
        );

        drop(module);
    }

    #[test]
    fn field_access_errors_use_field_access_location() {
        let program = HirProgram {
            modules: vec![HirModule {
                name: "demo.main".to_string(),
                defs: vec![HirDef {
                    name: "helper".to_string(),
                    location: None,
                    expr: HirExpr::Lambda {
                        id: 10,
                        param: "_".to_string(),
                        body: Box::new(HirExpr::FieldAccess {
                            id: 11,
                            base: Box::new(HirExpr::Tuple {
                                id: 12,
                                items: vec![lit_int(13, "1"), lit_int(14, "2")],
                            }),
                            field: "provider".to_string(),
                            location: Some(origin("src/demo/main.aivi", 4, 13)),
                        }),
                        location: None,
                    },
                }],
            }],
        };

        let mut runtime = build_runtime_from_program(&program).expect("build runtime");
        let module = jit_compile_into_runtime(
            program,
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            &mut runtime,
            &HashSet::new(),
        )
        .expect("compile runtime");

        let helper = runtime
            .ctx
            .globals
            .get("demo.main.helper")
            .expect("helper binding should exist")
            .clone();
        let err = runtime
            .apply(helper, Value::Unit)
            .expect_err("helper should fail");
        let rendered = crate::runtime::format_runtime_error(err);

        assert!(
            rendered.contains("field `provider`"),
            "expected field-access error, got:\n{rendered}"
        );
        assert!(
            rendered.contains("src/demo/main.aivi:4:13"),
            "expected field-access location, got:\n{rendered}"
        );

        drop(module);
    }

    #[test]
    fn inject_in_expr_wraps_load_argument_with_source_schema() {
        let mut expr = RustIrExpr::Call {
            id: 1,
            func: Box::new(RustIrExpr::Global {
                id: 2,
                name: "load".to_string(),
                location: None,
            }),
            args: vec![RustIrExpr::Global {
                id: 3,
                name: "demo.source".to_string(),
                location: None,
            }],
            location: None,
        };
        let schemas = vec![CgType::Adt {
            name: "Option".to_string(),
            constructors: vec![
                ("None".to_string(), Vec::new()),
                ("Some".to_string(), vec![CgType::Float]),
            ],
        }];
        let mut schema_idx = 0;
        let mut source_site_idx = 0;

        inject_in_expr(
            &mut expr,
            &schemas,
            &mut schema_idx,
            &[],
            &mut source_site_idx,
        );

        assert_eq!(schema_idx, 1);
        assert_eq!(source_site_idx, 0);
        let RustIrExpr::Call { func, args, .. } = expr else {
            panic!("expected load call");
        };
        assert!(matches!(
            func.as_ref(),
            RustIrExpr::Global { name, .. } if name == "load"
        ));
        let wrapped_source = args.into_iter().next().expect("load arg");
        let RustIrExpr::Call {
            func: wrapped_func,
            args: wrapped_args,
            ..
        } = wrapped_source
        else {
            panic!("expected wrapped source argument");
        };
        assert!(matches!(
            wrapped_func.as_ref(),
            RustIrExpr::Global { name, .. } if name == "__set_source_schema"
        ));
        assert_eq!(wrapped_args.len(), 2);
        let RustIrExpr::LitString {
            text: schema_json, ..
        } = &wrapped_args[0]
        else {
            panic!("expected schema JSON string");
        };
        let parsed_schema: crate::runtime::json_schema::JsonSchema =
            serde_json::from_str(schema_json).expect("valid schema json");
        assert_eq!(
            parsed_schema,
            crate::runtime::json_schema::JsonSchema::Option(Box::new(
                crate::runtime::json_schema::JsonSchema::Float,
            ))
        );
        assert!(matches!(
            &wrapped_args[1],
            RustIrExpr::Global { name, .. } if name == "demo.source"
        ));
    }

    #[test]
    fn inject_in_expr_wraps_file_source_constructor_with_source_schema() {
        let mut expr = RustIrExpr::Call {
            id: 1,
            func: Box::new(RustIrExpr::Builtin {
                id: 2,
                builtin: "file.csv".to_string(),
                location: None,
            }),
            args: vec![RustIrExpr::LitString {
                id: 3,
                text: "./users.csv".to_string(),
            }],
            location: None,
        };
        let schemas = vec![CgType::ListOf(Box::new(CgType::Record(
            std::collections::BTreeMap::from([
                ("id".to_string(), CgType::Int),
                ("name".to_string(), CgType::Text),
            ]),
        )))];
        let mut load_idx = 0;
        let mut source_site_idx = 0;

        inject_in_expr(
            &mut expr,
            &[],
            &mut load_idx,
            &schemas,
            &mut source_site_idx,
        );

        assert_eq!(load_idx, 0);
        assert_eq!(source_site_idx, 1);
        let RustIrExpr::Call { func, args, .. } = expr else {
            panic!("expected wrapped source call");
        };
        assert!(matches!(
            func.as_ref(),
            RustIrExpr::Global { name, .. } if name == "__set_source_schema"
        ));
        assert_eq!(args.len(), 2);
        let RustIrExpr::Call { func, .. } = &args[1] else {
            panic!("expected original source call as second arg");
        };
        assert!(matches!(
            func.as_ref(),
            RustIrExpr::Builtin { builtin, .. } if builtin == "file.csv"
        ));
    }

    #[test]
    fn inject_source_schemas_uses_fully_qualified_def_name_fallback() {
        let mut modules = vec![rust_ir::RustIrModule {
            name: "demo.main".to_string(),
            defs: vec![rust_ir::RustIrDef {
                name: "demo.main.parseValue".to_string(),
                location: None,
                expr: RustIrExpr::Call {
                    id: 1,
                    func: Box::new(RustIrExpr::Builtin {
                        id: 2,
                        builtin: "load".to_string(),
                        location: None,
                    }),
                    args: vec![RustIrExpr::Global {
                        id: 3,
                        name: "demo.source".to_string(),
                        location: None,
                    }],
                    location: None,
                },
                cg_type: None,
            }],
        }];
        let source_schemas = HashMap::from([(
            "demo.main.parseValue".to_string(),
            vec![CgType::Adt {
                name: "Option".to_string(),
                constructors: vec![
                    ("None".to_string(), Vec::new()),
                    ("Some".to_string(), vec![CgType::Float]),
                ],
            }],
        )]);

        inject_source_schemas(&mut modules, &source_schemas);

        let expr = &modules[0].defs[0].expr;
        let RustIrExpr::Call { args, .. } = expr else {
            panic!("expected load call");
        };
        let RustIrExpr::Call {
            func: wrapped_func, ..
        } = &args[0]
        else {
            panic!("expected wrapped load argument");
        };
        assert!(matches!(
            wrapped_func.as_ref(),
            RustIrExpr::Global { name, .. } if name == "__set_source_schema"
        ));
    }

    #[test]
    fn jit_load_wraps_nested_optional_float_from_file_json() {
        let temp_path = std::env::temp_dir().join(format!(
            "aivi-source-schema-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        std::fs::write(
            &temp_path,
            r#"{"entities":{"orders":[{"totalAmount":42.5}]}}"#,
        )
        .expect("write fixture");

        let source = format!(
            r#"
module Test

Order = {{ totalAmount: Option Float }}
Entities = {{ orders: List Order }}
Parsed = {{ entities: Entities }}

parse : Effect Text Parsed
parse =
   |> attempt (load (file.json "{}"))#decoded
  ||> Ok value => pure value
  ||> Err _ => fail "decode failed"
"#,
            temp_path.to_string_lossy()
        );

        let (modules, parse_diags) = parse_modules(Path::new("test.aivi"), &source);
        assert!(
            !crate::diagnostics::file_diagnostics_have_errors(&parse_diags),
            "unexpected parse errors: {parse_diags:?}"
        );

        let infer_result = aivi_core::infer_value_types_full(&modules);
        let non_embedded: Vec<_> = infer_result
            .diagnostics
            .iter()
            .filter(|d| !d.path.starts_with("<embedded:"))
            .cloned()
            .collect();
        assert!(
            !crate::diagnostics::file_diagnostics_have_errors(&non_embedded),
            "unexpected infer errors: {non_embedded:?}"
        );
        assert!(
            matches!(
                infer_result.source_schemas.get("Test.parse"),
                Some(schemas)
                    if matches!(
                        schemas.as_slice(),
                        [CgType::Record(fields)]
                            if matches!(
                                fields.get("entities"),
                                Some(CgType::Record(entity_fields))
                                    if matches!(
                                        entity_fields.get("orders"),
                                        Some(CgType::ListOf(item))
                                            if matches!(
                                                item.as_ref(),
                                                CgType::Record(order_fields)
                                                    if matches!(
                                                        order_fields.get("totalAmount"),
                                                        Some(CgType::Adt { name, constructors })
                                                            if name == "Option"
                                                                && matches!(
                                                                    constructors.as_slice(),
                                                                    [(none_name, none_args), (some_name, some_args)]
                                                                        if none_name == "None"
                                                                            && none_args.is_empty()
                                                                            && some_name == "Some"
                                                                            && matches!(some_args.as_slice(), [CgType::Float])
                                                                )
                                                    )
                                            )
                                    )
                            )
                    )
            ),
            "unexpected source schemas map: {:?}",
            infer_result.source_schemas
        );

        let program = crate::desugar_modules(&modules);
        let mut runtime = build_runtime_from_program(&program).expect("build runtime");
        let module = jit_compile_into_runtime(
            program,
            infer_result.cg_types,
            infer_result.monomorph_plan,
            infer_result.source_schemas,
            &mut runtime,
            &HashSet::new(),
        )
        .expect("compile runtime");

        let value = runtime
            .ctx
            .globals
            .get("Test.parse")
            .expect("parse binding should exist");
        let effect = runtime.force_value(value).unwrap_or_else(|err| {
            panic!(
                "force parse binding: {}",
                crate::runtime::format_runtime_error(err)
            )
        });
        let result = runtime.run_effect_value(effect).unwrap_or_else(|err| {
            panic!(
                "run parse effect: {}",
                crate::runtime::format_runtime_error(err)
            )
        });

        std::fs::remove_file(&temp_path).expect("remove fixture");

        assert!(matches!(
            result,
            Value::Record(fields)
                if matches!(
                    fields.get("entities"),
                    Some(Value::Record(entity_fields))
                        if matches!(
                            entity_fields.get("orders"),
                            Some(Value::List(items))
                                if matches!(
                                    &items.as_ref()[..],
                                    [Value::Record(order_fields)]
                                        if matches!(
                                            order_fields.get("totalAmount"),
                                            Some(Value::Constructor { name, args })
                                                if name == "Some"
                                                    && matches!(args.as_slice(), [Value::Float(amount)] if (*amount - 42.5).abs() < 0.0001)
                                        )
                                )
                        )
                )
        ));

        drop(module);
    }
}
