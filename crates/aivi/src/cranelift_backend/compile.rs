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
    collect_surface_constructor_ordinals, register_machines_for_jit, run_main_effect, CancelToken,
    Runtime, RuntimeError,
};
use crate::rust_ir::{
    RustIrDef, RustIrExpr, RustIrListItem, RustIrPathSegment, RustIrPattern, RustIrRecordField,
    RustIrTextPart,
};
use crate::AiviError;
use crate::{kernel, rust_ir};

use super::abi::JitRuntimeCtx;
use super::jit_module::create_jit_module;
use super::lower::{
    declare_helpers, decompose_func_type, CompiledLambda, DeclaredHelpers, JitFuncDecl,
    JitFuncInfo, LowerCtx,
};

/// Pointer type used throughout.
const PTR: cranelift_codegen::ir::Type = types::I64;

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

    let total_defs: usize = rust_program.modules.iter().map(|m| m.defs.len()).sum();
    if trace {
        eprintln!("[AIVI_TIMING] total defs to JIT-compile: {}", total_defs);
    }

    // Create JIT module with runtime helpers registered
    let mut module = timed!(
        "cranelift jit init",
        create_jit_module().map_err(|e| AiviError::Runtime(format!("cranelift jit init: {e}")))?
    );

    // Declare runtime helper imports in the module
    let helpers = timed!(
        "declare_helpers",
        declare_helpers(&mut module)
            .map_err(|e| AiviError::Runtime(format!("cranelift declare helpers: {e}")))?
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
            let (params, body) = peel_params(&def.expr);
            let qualified = format!("{}.{}", ir_module.name, def.name);
            let is_stdlib_module = ir_module.name.starts_with("aivi.");
            if params.len() > 15 {
                if is_stdlib_module {
                    continue;
                }
                return Err(AiviError::Runtime(format!(
                    "cranelift compile {}: unsupported arity {} (max 15)",
                    qualified,
                    params.len()
                )));
            }
            if !expr_supported(body) {
                if is_stdlib_module {
                    continue;
                }
                return Err(AiviError::Runtime(format!(
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
                .map_err(|e| AiviError::Runtime(format!("declare {}: {e}", func_name)))?;

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
        ) {
            Ok(lambdas) => {
                // JIT-specific: finalize and install lambdas immediately
                if !lambdas.is_empty() {
                    module
                        .finalize_definitions()
                        .map_err(|e| AiviError::Runtime(format!("finalize lambdas: {e}")))?;
                    for pl in &lambdas {
                        let ptr = module.get_finalized_function(pl.func_id);
                        let jit_value =
                            make_jit_builtin(&pl.global_name, pl.total_arity, ptr as usize);
                        runtime.ctx.globals.set(pl.global_name.clone(), jit_value);
                    }
                }
                pending.push(PendingDef {
                    name: dd.def.name.clone(),
                    qualified: dd.qualified.clone(),
                    func_id: dd.func_id,
                    arity: dd.arity,
                    is_effect_block: dd.is_effect_block,
                });
            }
            Err(e) => {
                return Err(AiviError::Runtime(format!(
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
            .map_err(|e| AiviError::Runtime(format!("cranelift finalize: {e}")))?
    );

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
    //    historical default.
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

    let trace = std::env::var("AIVI_TRACE_TIMING").is_ok_and(|v| v == "1");
    let t0 = if trace { Some(Instant::now()) } else { None };
    let mut runtime = build_runtime_from_program(&program)?;
    {
        let surface_ordinals = collect_surface_constructor_ordinals(surface_modules);
        if let Some(ctx) = Arc::get_mut(&mut runtime.ctx) {
            ctx.merge_constructor_ordinals(surface_ordinals);
        }
    }
    let _module = jit_compile_into_runtime(
        program,
        cg_types,
        monomorph_plan,
        source_schemas,
        &mut runtime,
    )?;
    register_machines_for_jit(&runtime, surface_modules);
    if let Some(t0) = t0 {
        eprintln!(
            "[AIVI_TIMING] {:40} {:>8.1}ms  ← TOTAL JIT",
            "JIT pipeline total",
            t0.elapsed().as_secs_f64() * 1000.0
        );
    }
    run_main_effect(&mut runtime)
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
    let _module = jit_compile_into_runtime(
        program,
        cg_types,
        monomorph_plan,
        source_schemas,
        &mut runtime,
    )?;
    register_machines_for_jit(&runtime, surface_modules);
    run_main_effect(&mut runtime)
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
    use crate::runtime::{
        format_runtime_error, format_value, TestFailure, TestReport, TestSuccess,
    };

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
    )?;
    register_machines_for_jit(&runtime, surface_modules);
    // Discard any pending errors from the compilation phase so they don't
    // contaminate the first test.
    runtime.jit_pending_error = None;

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

        // Clear any pending JIT error from a prior test so it doesn't contaminate this one.
        runtime.jit_pending_error = None;

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

        // Clear pending error right before executing the test effect, so only
        // errors from `run_effect_value` are captured.
        runtime.jit_pending_error = None;

        match runtime.run_effect_value(effect) {
            Ok(_) => {
                // Check for snapshot assertion failures that the JIT couldn't
                // propagate through the Effect chain.
                if let Some(msg) = runtime.snapshot_failure.take() {
                    runtime.jit_pending_error = None;
                    report.failed += 1;
                    report.failures.push(TestFailure {
                        name: name.clone(),
                        description: description.clone(),
                        message: msg,
                    });
                } else if let Some(err) = runtime.jit_pending_error.take() {
                    report.failed += 1;
                    report.failures.push(TestFailure {
                        name: name.clone(),
                        description: description.clone(),
                        message: format_runtime_error(err),
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
                // Discard any pending error — the propagated Err is the authoritative failure.
                runtime.jit_pending_error = None;
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

/// Walk all RustIR modules and wrap `load(source)` calls with
/// `__set_source_schema(schema_json, load(source))` when the typechecker
/// recorded a concrete inner type for that load site.
fn inject_source_schemas(
    modules: &mut [rust_ir::RustIrModule],
    source_schemas: &HashMap<String, Vec<CgType>>,
) {
    for module in modules.iter_mut() {
        for def in &mut module.defs {
            let key = format!("{}.{}", module.name, def.name);
            if let Some(schemas) = source_schemas.get(&key) {
                let mut schema_idx = 0;
                inject_in_expr(&mut def.expr, schemas, &mut schema_idx);
            }
        }
    }
}

/// Recursively walk a RustIR expression. When we find `App(Global("load"), arg)`
/// or `Call(Global("load"), [arg])`, wrap the whole thing:
///   `Call(Global("__set_source_schema"), [LitString(schema_json), <original>])`
fn inject_in_expr(expr: &mut RustIrExpr, schemas: &[CgType], idx: &mut usize) {
    // First recurse into children so inner load calls are found in order
    match expr {
        RustIrExpr::Lambda { body, .. } => inject_in_expr(body, schemas, idx),
        RustIrExpr::App { func, arg, .. } => {
            inject_in_expr(func, schemas, idx);
            inject_in_expr(arg, schemas, idx);
        }
        RustIrExpr::Call { func, args, .. } => {
            inject_in_expr(func, schemas, idx);
            for a in args.iter_mut() {
                inject_in_expr(a, schemas, idx);
            }
        }
        RustIrExpr::List { items, .. } => {
            for item in items {
                inject_in_expr(&mut item.expr, schemas, idx);
            }
        }
        RustIrExpr::Tuple { items, .. } => {
            for item in items {
                inject_in_expr(item, schemas, idx);
            }
        }
        RustIrExpr::Record { fields, .. } => {
            for field in fields {
                inject_in_expr(&mut field.value, schemas, idx);
            }
        }
        RustIrExpr::Patch { target, fields, .. } => {
            inject_in_expr(target, schemas, idx);
            for field in fields {
                inject_in_expr(&mut field.value, schemas, idx);
            }
        }
        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            inject_in_expr(scrutinee, schemas, idx);
            for arm in arms {
                inject_in_expr(&mut arm.body, schemas, idx);
            }
        }
        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            inject_in_expr(cond, schemas, idx);
            inject_in_expr(then_branch, schemas, idx);
            inject_in_expr(else_branch, schemas, idx);
        }
        RustIrExpr::Binary { left, right, .. } => {
            inject_in_expr(left, schemas, idx);
            inject_in_expr(right, schemas, idx);
        }
        RustIrExpr::TextInterpolate { parts, .. } => {
            for part in parts {
                if let RustIrTextPart::Expr { expr: e } = part {
                    inject_in_expr(e, schemas, idx);
                }
            }
        }
        RustIrExpr::DebugFn { body, .. } => inject_in_expr(body, schemas, idx),
        RustIrExpr::Pipe { func, arg, .. } => {
            inject_in_expr(func, schemas, idx);
            inject_in_expr(arg, schemas, idx);
        }
        RustIrExpr::FieldAccess { base, .. } | RustIrExpr::Index { base, .. } => {
            inject_in_expr(base, schemas, idx);
        }
        RustIrExpr::Mock { body, .. } => inject_in_expr(body, schemas, idx),
        // Leaves: Local, Global, Builtin, ConstructorValue, Lit*, Raw, etc.
        _ => {}
    }

    // After recursing, check if this node is a `load` application
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
        if let Some(cg_type) = schemas.get(*idx) {
            let schema = cg_type_to_json_schema(cg_type);
            if let Ok(schema_json) = serde_json::to_string(&schema) {
                // Replace: `load(source)` → `__set_source_schema(schema_json, load(source))`
                let original = std::mem::replace(
                    expr,
                    RustIrExpr::LitBool {
                        id: 0,
                        value: false,
                    },
                );
                *expr = RustIrExpr::Call {
                    id: 0,
                    func: Box::new(RustIrExpr::Global {
                        id: 0,
                        name: "__set_source_schema".to_string(),
                    }),
                    args: vec![
                        RustIrExpr::LitString {
                            id: 0,
                            text: schema_json,
                        },
                        original,
                    ],
                };
            }
        }
        *idx += 1;
    }
}

include!("compile/support.rs");
include!("compile/body.rs");
include!("compile/aot.rs");
