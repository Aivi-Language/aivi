//! Cranelift compilation pipeline (shared between JIT and AOT).
//!
//! `run_cranelift_jit` is the JIT entrypoint that compiles and executes in-memory.
//! `compile_to_object` is the AOT entrypoint that emits a native object file.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use cranelift_codegen::ir::{types, AbiParam, Function, InstBuilder};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{Linkage, Module};

use crate::cg_type::CgType;
use crate::hir::HirProgram;
use crate::runtime::values::Value;
use crate::runtime::{
    build_runtime_from_program, build_runtime_from_program_with_cancel, run_main_effect,
    CancelToken, Runtime, RuntimeError,
};
use crate::rust_ir::{
    RustIrBlockItem, RustIrBlockKind, RustIrDef, RustIrExpr, RustIrListItem, RustIrPathSegment,
    RustIrPattern, RustIrRecordField, RustIrTextPart,
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
    runtime: &mut Runtime,
) -> Result<cranelift_jit::JITModule, AiviError> {
    // Lower HIR → Kernel → RustIR
    let kernel_program = kernel::lower_hir(program);
    let mut rust_program = rust_ir::lower_kernel(kernel_program)?;

    // Annotate each def with its CgType
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

    // Monomorphize
    let spec_map = monomorphize_program(&mut rust_program.modules, &monomorph_plan);

    // Create JIT module with runtime helpers registered
    let mut module =
        create_jit_module().map_err(|e| AiviError::Runtime(format!("cranelift jit init: {e}")))?;

    // Declare runtime helper imports in the module
    let helpers = declare_helpers(&mut module)
        .map_err(|e| AiviError::Runtime(format!("cranelift declare helpers: {e}")))?;

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
    for ir_module in &rust_program.modules {
        for def in &ir_module.defs {
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

            let is_effect_block = params.is_empty()
                && matches!(
                    body,
                    RustIrExpr::Block {
                        block_kind: RustIrBlockKind::Do { .. },
                        ..
                    }
                );

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

    for dd in &declared_defs {
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
                // Register under qualified name only
                compiled_decls.insert(
                    dd.qualified.clone(),
                    JitFuncDecl {
                        func_id: dd.func_id,
                        arity: dd.arity,
                        param_types: dd.param_types.clone(),
                        return_type: dd.return_type.clone(),
                    },
                );
            }
            Err(e) => {
                return Err(AiviError::Runtime(format!(
                    "cranelift compile {}: {e}",
                    dd.qualified
                )))
            }
        }
    }

    // Finalize all definitions at once, then extract pointers
    module
        .finalize_definitions()
        .map_err(|e| AiviError::Runtime(format!("cranelift finalize: {e}")))?;

    let mut compiled_globals: HashMap<String, Value> = HashMap::new();

    // Insert-or-merge: when the same name appears multiple times (multi-clause
    // domain operators), wrap all clauses in Value::MultiClause so the runtime
    // can try each clause in order via apply_multi_clause.
    fn insert_or_merge(map: &mut HashMap<String, Value>, key: String, value: Value) {
        use std::collections::hash_map::Entry;
        match map.entry(key) {
            Entry::Vacant(e) => {
                e.insert(value);
            }
            Entry::Occupied(mut e) => {
                let existing = e.get_mut();
                match existing {
                    Value::MultiClause(clauses) => clauses.push(value),
                    _ => {
                        let prev = std::mem::replace(existing, Value::Unit);
                        *existing = Value::MultiClause(vec![prev, value]);
                    }
                }
            }
        }
    }

    for pd in &pending {
        let ptr = module.get_finalized_function(pd.func_id);
        if pd.is_effect_block {
            // Zero-arity effect blocks: wrap in EffectValue::Thunk to defer execution.
            // The thunk calls the JIT function only when the effect is run.
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
                            // The JIT's lower_do_block wraps results via rt_wrap_effect.
                            // Unwrap the extra Effect layer to avoid double-wrapping.
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
            insert_or_merge(&mut compiled_globals, pd.name.clone(), effect.clone());
            insert_or_merge(&mut compiled_globals, pd.qualified.clone(), effect);
        } else {
            let jit_value = make_jit_builtin(&pd.qualified, pd.arity, ptr as usize);
            insert_or_merge(&mut compiled_globals, pd.name.clone(), jit_value.clone());
            insert_or_merge(&mut compiled_globals, pd.qualified.clone(), jit_value);
        }
    }

    // Install compiled globals into the runtime.
    for (name, value) in compiled_globals {
        // Source defs cannot shadow builtins.
        if let Some(Value::Builtin(_)) = runtime.ctx.globals.get(&name) {
            continue;
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
) -> Result<(), AiviError> {
    let mut runtime = build_runtime_from_program(&program)?;
    let _module = jit_compile_into_runtime(program, cg_types, monomorph_plan, &mut runtime)?;
    run_main_effect(&mut runtime)
}

/// Like [`run_cranelift_jit`] but accepts an external cancel token so the
/// caller can cancel execution from another thread (used by `--watch`).
pub fn run_cranelift_jit_cancellable(
    program: HirProgram,
    cg_types: HashMap<String, HashMap<String, CgType>>,
    monomorph_plan: HashMap<String, Vec<CgType>>,
    cancel: Arc<CancelToken>,
) -> Result<(), AiviError> {
    let mut runtime = build_runtime_from_program_with_cancel(&program, cancel)?;
    let _module = jit_compile_into_runtime(program, cg_types, monomorph_plan, &mut runtime)?;
    run_main_effect(&mut runtime)
}

/// JIT-compile an AIVI program and run its test suite.
///
/// Like `run_cranelift_jit` but executes the named test entries instead of `main`.
pub fn run_test_suite_jit(
    program: HirProgram,
    test_entries: &[(String, String)],
    surface_modules: &[crate::surface::Module],
) -> Result<crate::runtime::TestReport, AiviError> {
    use crate::runtime::{TestFailure, TestReport, TestSuccess, format_runtime_error, format_value};

    let infer_result = aivi_core::infer_value_types_full(surface_modules);
    let mut runtime = build_runtime_from_program(&program)?;
    let _module = jit_compile_into_runtime(
        program,
        infer_result.cg_types,
        infer_result.monomorph_plan,
        &mut runtime,
    )?;

    const TEST_FUEL_BUDGET: u64 = 500_000;
    let mut report = TestReport {
        passed: 0,
        failed: 0,
        failures: Vec::new(),
        successes: Vec::new(),
    };

    for (name, description) in test_entries {
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

/// Compile an AIVI program to a native object file via Cranelift AOT.
///
/// Returns the raw object file bytes. The caller is responsible for writing
/// them to disk and linking with the AIVI runtime library.
pub fn compile_to_object(
    program: HirProgram,
    cg_types: HashMap<String, HashMap<String, CgType>>,
    monomorph_plan: HashMap<String, Vec<CgType>>,
) -> Result<Vec<u8>, AiviError> {
    use super::object_module::create_object_module;

    // 1. Lower HIR → Kernel → RustIR
    let kernel_program = kernel::lower_hir(program);
    let mut rust_program = rust_ir::lower_kernel(kernel_program)?;

    // 2. Annotate each def with its CgType
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

    // 3. Monomorphize
    let spec_map = monomorphize_program(&mut rust_program.modules, &monomorph_plan);

    // 4. Create ObjectModule targeting the host platform
    let mut module = create_object_module("aivi_program")
        .map_err(|e| AiviError::Runtime(format!("cranelift object init: {e}")))?;

    // 5. Declare runtime helper imports
    let helpers = declare_helpers(&mut module)
        .map_err(|e| AiviError::Runtime(format!("cranelift declare helpers: {e}")))?;

    // 6. Two-pass compilation (same as JIT path)
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

    // Pass 1: Declare all eligible functions
    for ir_module in &rust_program.modules {
        for def in &ir_module.defs {
            let (params, body) = peel_params(&def.expr);
            let qualified = format!("{}.{}", ir_module.name, def.name);
            let is_stdlib_module = ir_module.name.starts_with("aivi.");
            if params.len() > 15 {
                if is_stdlib_module {
                    continue;
                }
                return Err(AiviError::Runtime(format!(
                    "cranelift aot compile {}: unsupported arity {} (max 15)",
                    qualified,
                    params.len()
                )));
            }
            if !expr_supported(body) {
                if is_stdlib_module {
                    continue;
                }
                return Err(AiviError::Runtime(format!(
                    "cranelift aot compile {}: unsupported expression shape",
                    qualified
                )));
            }
            let func_name = format!("__aivi_jit_{}", sanitize_name(&qualified));
            if declared_names.contains(&func_name) {
                continue;
            }
            declared_names.insert(func_name.clone());

            let arity = params.len();
            let mut sig = module.make_signature();
            sig.params.push(AbiParam::new(PTR)); // ctx
            for _ in 0..arity {
                sig.params.push(AbiParam::new(PTR));
            }
            sig.returns.push(AbiParam::new(PTR));

            // AOT: export all functions so the runtime can find them
            let func_id = module
                .declare_function(&func_name, Linkage::Export, &sig)
                .map_err(|e| AiviError::Runtime(format!("declare {}: {e}", func_name)))?;

            let (param_types, return_type) = if let Some(cg_ty) = &def.cg_type {
                decompose_func_type(cg_ty, arity)
            } else {
                (vec![None; arity], None)
            };

            let is_effect_block = params.is_empty()
                && matches!(
                    body,
                    RustIrExpr::Block {
                        block_kind: RustIrBlockKind::Do { .. },
                        ..
                    }
                );

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
        }
    }

    // Build JitFuncDecl registry for direct calls
    let mut compiled_decls: HashMap<String, JitFuncDecl> = HashMap::new();
    // Pre-populate with all declared functions (AOT can forward-reference)
    for dd in &declared_defs {
        compiled_decls.insert(
            dd.qualified.clone(),
            JitFuncDecl {
                func_id: dd.func_id,
                arity: dd.arity,
                param_types: dd.param_types.clone(),
                return_type: dd.return_type.clone(),
            },
        );
    }

    // Pass 2: Compile function bodies
    let mut lambda_counter: usize = 0;
    let mut compiled_func_entries: Vec<AotFuncEntry> = Vec::new();
    let mut str_counter: usize = 0;

    let mut all_lambdas: Vec<CompiledLambdaInfo> = Vec::new();

    for dd in &declared_defs {
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
                all_lambdas.extend(lambdas);
                compiled_func_entries.push(AotFuncEntry {
                    short_name: dd.def.name.clone(),
                    qualified_name: dd.qualified.clone(),
                    func_id: dd.func_id,
                    arity: dd.arity,
                    is_effect_block: dd.is_effect_block,
                });
            }
            Err(e) => {
                return Err(AiviError::Runtime(format!(
                    "cranelift aot compile {}: {e}",
                    dd.qualified
                )))
            }
        }
    }

    // 7. Generate the entry point wrapper: __aivi_main()
    generate_aot_entry(&mut module, &helpers, &compiled_func_entries, &all_lambdas)
        .map_err(|e| AiviError::Runtime(format!("aot entry point: {e}")))?;

    // 8. Emit the object file
    let product = module.finish();
    let bytes = product
        .emit()
        .map_err(|e| AiviError::Runtime(format!("emit object: {e}")))?;

    Ok(bytes)
}

/// Information about a compiled function for AOT entry-point registration.
pub(crate) struct AotFuncEntry {
    pub(crate) short_name: String,
    pub(crate) qualified_name: String,
    pub(crate) func_id: cranelift_module::FuncId,
    pub(crate) arity: usize,
    pub(crate) is_effect_block: bool,
}

/// Generate the AOT entry point `__aivi_main` that:
/// 1. Registers all compiled functions as globals via `rt_register_jit_fn`
/// 2. Looks up and runs the `main` function as an effect
/// 3. Returns the result
fn generate_aot_entry<M: Module>(
    module: &mut M,
    helpers: &DeclaredHelpers,
    compiled_funcs: &[AotFuncEntry],
    compiled_lambdas: &[CompiledLambdaInfo],
) -> Result<(), String> {
    use cranelift_module::DataDescription;

    // Declare the entry function: (ctx) -> ptr
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(PTR)); // ctx
    sig.returns.push(AbiParam::new(PTR)); // result

    let func_id = module
        .declare_function("__aivi_main", Linkage::Export, &sig)
        .map_err(|e| format!("declare __aivi_main: {e}"))?;

    let mut function = Function::with_name_signature(
        cranelift_codegen::ir::UserFuncName::user(0, func_id.as_u32()),
        sig,
    );

    // Import helpers
    let helper_refs = helpers.import_into(module, &mut function);

    // Embed function name strings as data sections and declare func refs
    struct FuncReg {
        func_ref: cranelift_codegen::ir::FuncRef,
        short_name_gv: cranelift_codegen::ir::GlobalValue,
        short_name_len: usize,
        qual_name_gv: cranelift_codegen::ir::GlobalValue,
        qual_name_len: usize,
        arity: usize,
        is_effect_block: bool,
    }
    let mut regs = Vec::new();

    for (i, entry) in compiled_funcs.iter().enumerate() {
        let func_ref = module.declare_func_in_func(entry.func_id, &mut function);

        // Embed short name
        let short_data_id = module
            .declare_data(&format!("__nm_s_{i}"), Linkage::Local, false, false)
            .map_err(|e| format!("declare name data: {e}"))?;
        let mut dd = DataDescription::new();
        dd.define(entry.short_name.as_bytes().to_vec().into_boxed_slice());
        module
            .define_data(short_data_id, &dd)
            .map_err(|e| format!("define name data: {e}"))?;
        let short_gv = module.declare_data_in_func(short_data_id, &mut function);

        // Embed qualified name
        let qual_data_id = module
            .declare_data(&format!("__nm_q_{i}"), Linkage::Local, false, false)
            .map_err(|e| format!("declare qual name data: {e}"))?;
        let mut dd = DataDescription::new();
        dd.define(entry.qualified_name.as_bytes().to_vec().into_boxed_slice());
        module
            .define_data(qual_data_id, &dd)
            .map_err(|e| format!("define qual name data: {e}"))?;
        let qual_gv = module.declare_data_in_func(qual_data_id, &mut function);

        regs.push(FuncReg {
            func_ref,
            short_name_gv: short_gv,
            short_name_len: entry.short_name.len(),
            qual_name_gv: qual_gv,
            qual_name_len: entry.qualified_name.len(),
            arity: entry.arity,
            is_effect_block: entry.is_effect_block,
        });
    }

    // Prepare lambda registrations
    struct LambdaReg {
        func_ref: cranelift_codegen::ir::FuncRef,
        name_gv: cranelift_codegen::ir::GlobalValue,
        name_len: usize,
        arity: usize,
    }
    let mut lambda_regs = Vec::new();

    for (i, lam) in compiled_lambdas.iter().enumerate() {
        let func_ref = module.declare_func_in_func(lam.func_id, &mut function);

        let data_id = module
            .declare_data(&format!("__nm_lam_{i}"), Linkage::Local, false, false)
            .map_err(|e| format!("declare lambda name data: {e}"))?;
        let mut dd = DataDescription::new();
        dd.define(lam.global_name.as_bytes().to_vec().into_boxed_slice());
        module
            .define_data(data_id, &dd)
            .map_err(|e| format!("define lambda name data: {e}"))?;
        let name_gv = module.declare_data_in_func(data_id, &mut function);

        lambda_regs.push(LambdaReg {
            func_ref,
            name_gv,
            name_len: lam.global_name.len(),
            arity: lam.total_arity,
        });
    }

    // Embed "main" string for the final lookup
    let main_data_id = module
        .declare_data("__nm_main", Linkage::Local, false, false)
        .map_err(|e| format!("declare main name: {e}"))?;
    let mut dd = DataDescription::new();
    dd.define(b"main".to_vec().into_boxed_slice());
    module
        .define_data(main_data_id, &dd)
        .map_err(|e| format!("define main name: {e}"))?;
    let main_name_gv = module.declare_data_in_func(main_data_id, &mut function);

    // Build the function body
    let mut fb_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut function, &mut fb_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let ctx_param = builder.block_params(entry)[0];

        // Register each compiled function (short + qualified name)
        for reg in &regs {
            let func_ptr = builder.ins().func_addr(PTR, reg.func_ref);
            let arity_val = builder.ins().iconst(PTR, reg.arity as i64);
            let is_effect_val = builder
                .ins()
                .iconst(PTR, if reg.is_effect_block { 1i64 } else { 0i64 });

            let short_ptr = builder.ins().global_value(PTR, reg.short_name_gv);
            let short_len = builder.ins().iconst(PTR, reg.short_name_len as i64);
            builder.ins().call(
                helper_refs.rt_register_jit_fn,
                &[
                    ctx_param,
                    short_ptr,
                    short_len,
                    func_ptr,
                    arity_val,
                    is_effect_val,
                ],
            );

            let qual_ptr = builder.ins().global_value(PTR, reg.qual_name_gv);
            let qual_len = builder.ins().iconst(PTR, reg.qual_name_len as i64);
            builder.ins().call(
                helper_refs.rt_register_jit_fn,
                &[
                    ctx_param,
                    qual_ptr,
                    qual_len,
                    func_ptr,
                    arity_val,
                    is_effect_val,
                ],
            );
        }

        // Register each compiled lambda
        for lreg in &lambda_regs {
            let func_ptr = builder.ins().func_addr(PTR, lreg.func_ref);
            let arity_val = builder.ins().iconst(PTR, lreg.arity as i64);
            let is_effect_val = builder.ins().iconst(PTR, 0i64);

            let name_ptr = builder.ins().global_value(PTR, lreg.name_gv);
            let name_len = builder.ins().iconst(PTR, lreg.name_len as i64);
            builder.ins().call(
                helper_refs.rt_register_jit_fn,
                &[ctx_param, name_ptr, name_len, func_ptr, arity_val, is_effect_val],
            );
        }

        // Look up "main" and run as effect
        let main_ptr = builder.ins().global_value(PTR, main_name_gv);
        let main_len = builder.ins().iconst(PTR, 4i64);
        let main_val = builder
            .ins()
            .call(helper_refs.rt_get_global, &[ctx_param, main_ptr, main_len]);
        let main_val = builder.inst_results(main_val)[0];

        let result = builder
            .ins()
            .call(helper_refs.rt_run_effect, &[ctx_param, main_val]);
        let result = builder.inst_results(result)[0];

        builder.ins().return_(&[result]);
        builder.finalize();
    }

    let mut ctx = module.make_context();
    ctx.func = function;
    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| format!("define __aivi_main: {e}"))?;
    module.clear_context(&mut ctx);

    Ok(())
}

/// Information about a compiled lambda that needs post-processing.
pub(crate) struct CompiledLambdaInfo {
    pub(crate) func_id: cranelift_module::FuncId,
    pub(crate) global_name: String,
    pub(crate) total_arity: usize,
}

/// Compile the body of a pre-declared function.
///
/// The function has already been declared via `module.declare_function`; this
/// fills in the body IR. Returns pending lambda info on success.
/// Generic over `M: Module` so it works with both JITModule and ObjectModule.
fn compile_definition_body<M: Module>(
    module: &mut M,
    helpers: &DeclaredHelpers,
    def: &RustIrDef,
    module_name: &str,
    qualified_name: &str,
    func_id: cranelift_module::FuncId,
    _arity: usize,
    param_types: &[Option<CgType>],
    _return_type: &Option<CgType>,
    compiled_decls: &HashMap<String, JitFuncDecl>,
    lambda_counter: &mut usize,
    spec_map: &HashMap<String, Vec<String>>,
    str_counter: &mut usize,
) -> Result<Vec<CompiledLambdaInfo>, String> {
    let (params, body) = peel_params(&def.expr);

    // --- Pre-compile inner lambdas ---
    let mut lambdas: Vec<(&RustIrExpr, Vec<String>)> = Vec::new();
    collect_inner_lambdas(body, &mut Vec::new(), &mut lambdas);

    let mut compiled_lambdas: HashMap<usize, CompiledLambda> = HashMap::new();

    // Compile each lambda as a function: (ctx, cap0, cap1, ..., param) -> result
    // Lambdas are collected bottom-up (innermost first), so nested lambdas
    // appear before their parents.  Global lookups at runtime resolve
    // forward references.
    let mut pending_lambdas: Vec<CompiledLambdaInfo> = Vec::new();

    for (lambda_expr, captured_vars) in &lambdas {
        let RustIrExpr::Lambda { param, body, .. } = lambda_expr else {
            continue;
        };

        let total_arity = captured_vars.len() + 1; // captures + the actual param
        if total_arity > 15 {
            eprintln!(
                "aivi: lambda skipped: too many captures ({} captures + 1 param = {} > 15)",
                captured_vars.len(),
                total_arity
            );
            continue;
        }

        let global_name = format!("__jit_lambda_{}", *lambda_counter);
        *lambda_counter += 1;

        // Leak the name so the raw pointer embedded in JIT code remains valid.
        let global_name_static: &'static str = Box::leak(global_name.clone().into_boxed_str());

        // Store in compiled_lambdas so nested lambdas can reference it
        let key = *lambda_expr as *const RustIrExpr as usize;
        compiled_lambdas.insert(
            key,
            CompiledLambda {
                global_name: global_name_static,
                captured_vars: captured_vars.clone(),
            },
        );

        // Build function signature: (ctx, cap0, cap1, ..., param) -> result
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(PTR)); // ctx
        for _ in 0..total_arity {
            sig.params.push(AbiParam::new(PTR)); // each cap + param
        }
        sig.returns.push(AbiParam::new(PTR));

        let func_name = format!("__aivi_lambda_{}", sanitize_name(&global_name));
        let func_id = module
            .declare_function(&func_name, Linkage::Local, &sig)
            .map_err(|e| format!("declare lambda {}: {e}", func_name))?;

        let mut function = Function::with_name_signature(
            cranelift_codegen::ir::UserFuncName::user(0, func_id.as_u32()),
            sig,
        );

        let helper_refs = helpers.import_into(module, &mut function);

        let mut fb_ctx = FunctionBuilderContext::new();
        {
            let mut builder = FunctionBuilder::new(&mut function, &mut fb_ctx);
            let entry = builder.create_block();
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let block_params = builder.block_params(entry).to_vec();
            let ctx_param = block_params[0];

            // --- Call-depth guard: bail with Unit if recursion too deep ---
            let depth_exceeded = builder.ins().call(helper_refs.rt_check_call_depth, &[ctx_param]);
            let depth_flag = builder.inst_results(depth_exceeded)[0];
            let zero = builder.ins().iconst(types::I64, 0);
            let is_exceeded = builder.ins().icmp(cranelift_codegen::ir::condcodes::IntCC::NotEqual, depth_flag, zero);
            let body_block = builder.create_block();
            let bail_block = builder.create_block();
            builder.ins().brif(is_exceeded, bail_block, &[], body_block, &[]);

            // Bail block: return Unit without lowering the body
            builder.switch_to_block(bail_block);
            builder.seal_block(bail_block);
            let unit_val = builder.ins().call(helper_refs.rt_alloc_unit, &[ctx_param]);
            let unit_ptr = builder.inst_results(unit_val)[0];
            builder.ins().return_(&[unit_ptr]);

            // Body block: normal execution
            builder.switch_to_block(body_block);
            builder.seal_block(body_block);

            let empty_jit_funcs: HashMap<String, JitFuncInfo> = HashMap::new();
            let empty_spec_map: HashMap<String, Vec<String>> = HashMap::new();
            let mut lower_ctx = LowerCtx::new(
                ctx_param,
                &helper_refs,
                &compiled_lambdas,
                &empty_jit_funcs,
                &empty_spec_map,
                module,
                str_counter,
            );

            // Bind captured vars as leading params (boxed — received as *mut Value)
            for (i, var_name) in captured_vars.iter().enumerate() {
                lower_ctx.locals.insert(
                    var_name.clone(),
                    super::lower::TypedValue::boxed(block_params[i + 1]),
                );
            }
            // Bind the actual lambda parameter (boxed)
            lower_ctx.locals.insert(
                param.clone(),
                super::lower::TypedValue::boxed(block_params[captured_vars.len() + 1]),
            );

            let result = lower_ctx.lower_expr(&mut builder, body);
            let result_boxed = lower_ctx.ensure_boxed(&mut builder, result);
            builder.ins().call(helper_refs.rt_dec_call_depth, &[ctx_param]);
            builder.ins().return_(&[result_boxed]);
            builder.finalize();
        }

        let mut ctx = module.make_context();
        ctx.func = function;
        module
            .define_function(func_id, &mut ctx)
            .map_err(|e| format!("define lambda {}: {e}", func_name))?;
        module.clear_context(&mut ctx);

        pending_lambdas.push(CompiledLambdaInfo {
            func_id,
            global_name: global_name.clone(),
            total_arity,
        });
    }

    // --- Compile the main body (function was pre-declared in Pass 1) ---
    let sig = module
        .declarations()
        .get_function_decl(func_id)
        .signature
        .clone();
    let mut function = Function::with_name_signature(
        cranelift_codegen::ir::UserFuncName::user(0, func_id.as_u32()),
        sig,
    );

    let helper_refs = helpers.import_into(module, &mut function);

    // Import only JIT functions that the body actually references AND that have
    // been successfully compiled. Resolve short names via the current module.
    // Also import specializations (from spec_map) when the original is referenced.
    let mut called_globals = HashSet::new();
    collect_called_globals(body, &mut called_globals);

    let mut local_jit_funcs: HashMap<String, JitFuncInfo> = HashMap::new();
    let mut local_spec_map: HashMap<String, Vec<String>> = HashMap::new();
    for name in &called_globals {
        // Try qualified name first, then resolve short name via current module
        let decl = compiled_decls.get(name).or_else(|| {
            let qualified = format!("{}.{}", module_name, name);
            compiled_decls.get(&qualified)
        });
        if let Some(decl) = decl {
            let func_ref = module.declare_func_in_func(decl.func_id, &mut function);
            local_jit_funcs.insert(
                name.clone(),
                JitFuncInfo {
                    func_ref,
                    arity: decl.arity,
                    param_types: decl.param_types.clone(),
                    return_type: decl.return_type.clone(),
                },
            );
        }

        // Also import any specializations of this function
        if let Some(spec_names) = spec_map.get(name.as_str()) {
            let mut imported_specs = Vec::new();
            for spec_short in spec_names {
                // Resolve the specialization's qualified name
                let spec_qualified = format!("{}.{}", module_name, spec_short);
                let spec_decl = compiled_decls
                    .get(spec_short)
                    .or_else(|| compiled_decls.get(&spec_qualified));
                if let Some(sd) = spec_decl {
                    let func_ref = module.declare_func_in_func(sd.func_id, &mut function);
                    local_jit_funcs.insert(
                        spec_short.clone(),
                        JitFuncInfo {
                            func_ref,
                            arity: sd.arity,
                            param_types: sd.param_types.clone(),
                            return_type: sd.return_type.clone(),
                        },
                    );
                    imported_specs.push(spec_short.clone());
                }
            }
            if !imported_specs.is_empty() {
                local_spec_map.insert(name.clone(), imported_specs);
            }
        }
    }

    let mut fb_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut function, &mut fb_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let block_params = builder.block_params(entry).to_vec();
        let ctx_param = block_params[0];

        // --- Call-depth guard: bail with Unit if recursion too deep ---
        let depth_exceeded = builder.ins().call(helper_refs.rt_check_call_depth, &[ctx_param]);
        let depth_flag = builder.inst_results(depth_exceeded)[0];
        let zero = builder.ins().iconst(types::I64, 0);
        let is_exceeded = builder.ins().icmp(cranelift_codegen::ir::condcodes::IntCC::NotEqual, depth_flag, zero);
        let body_block = builder.create_block();
        let bail_block = builder.create_block();
        builder.ins().brif(is_exceeded, bail_block, &[], body_block, &[]);

        // Bail block: return Unit without lowering the body
        builder.switch_to_block(bail_block);
        builder.seal_block(bail_block);
        let unit_val = builder.ins().call(helper_refs.rt_alloc_unit, &[ctx_param]);
        let unit_ptr = builder.inst_results(unit_val)[0];
        builder.ins().return_(&[unit_ptr]);

        // Body block: normal execution
        builder.switch_to_block(body_block);
        builder.seal_block(body_block);

        let mut lower_ctx = LowerCtx::new(
            ctx_param,
            &helper_refs,
            &compiled_lambdas,
            &local_jit_funcs,
            &local_spec_map,
            module,
            str_counter,
        );

        // Bind params with typed unboxing when types are known
        let param_names: Vec<String> = params.iter().map(|s| s.to_string()).collect();
        lower_ctx.bind_typed_params(&mut builder, &param_names, &block_params, param_types);

        let result = lower_ctx.lower_expr(&mut builder, body);
        let result_boxed = lower_ctx.ensure_boxed(&mut builder, result);
        builder.ins().call(helper_refs.rt_dec_call_depth, &[ctx_param]);
        builder.ins().return_(&[result_boxed]);
        builder.finalize();
    }

    let mut ctx = module.make_context();
    ctx.func = function;
    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| format!("define {}: {e}", qualified_name))?;
    module.clear_context(&mut ctx);

    Ok(pending_lambdas)
}

fn record_field_supported(field: &RustIrRecordField) -> bool {
    if field.spread {
        return false;
    }
    if field.path.len() != 1 {
        return false;
    }
    matches!(field.path[0], RustIrPathSegment::Field(_)) && expr_supported(&field.value)
}

fn list_item_supported(item: &RustIrListItem) -> bool {
    expr_supported(&item.expr)
}

fn pattern_supported(pattern: &RustIrPattern) -> bool {
    match pattern {
        RustIrPattern::Wildcard { .. }
        | RustIrPattern::Var { .. }
        | RustIrPattern::Literal { .. } => true,
        RustIrPattern::At { pattern, .. } => pattern_supported(pattern),
        RustIrPattern::Constructor { args, .. } => args.iter().all(pattern_supported),
        RustIrPattern::Tuple { items, .. } => items.iter().all(pattern_supported),
        RustIrPattern::List { items, rest, .. } => {
            items.iter().all(pattern_supported) && rest.as_deref().map_or(true, pattern_supported)
        }
        RustIrPattern::Record { fields, .. } => {
            fields.iter().all(|f| pattern_supported(&f.pattern))
        }
    }
}

fn block_item_supported(item: &RustIrBlockItem) -> bool {
    match item {
        RustIrBlockItem::Bind { pattern, expr } => {
            pattern_supported(pattern) && expr_supported(expr)
        }
        RustIrBlockItem::Expr { expr } => expr_supported(expr),
        RustIrBlockItem::Filter { .. }
        | RustIrBlockItem::Yield { .. }
        | RustIrBlockItem::Recurse { .. } => false,
    }
}

fn expr_supported(expr: &RustIrExpr) -> bool {
    match expr {
        RustIrExpr::Local { .. }
        | RustIrExpr::Global { .. }
        | RustIrExpr::Builtin { .. }
        | RustIrExpr::ConstructorValue { .. }
        | RustIrExpr::LitString { .. }
        | RustIrExpr::LitBool { .. }
        | RustIrExpr::Raw { .. } => true,

        RustIrExpr::LitNumber { text, .. } => {
            text.parse::<i64>().is_ok() || text.parse::<f64>().is_ok()
        }

        // These have non-trivial runtime semantics we haven't matched yet.
        RustIrExpr::LitSigil { .. }
        | RustIrExpr::LitDateTime { .. }
        | RustIrExpr::TextInterpolate { .. }
        | RustIrExpr::DebugFn { .. }
        | RustIrExpr::Pipe { .. } => true,

        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            expr_supported(scrutinee)
                && arms.iter().all(|arm| {
                    pattern_supported(&arm.pattern)
                        && arm.guard.as_ref().map_or(true, expr_supported)
                        && expr_supported(&arm.body)
                })
        }

        RustIrExpr::Patch { target, fields, .. } => {
            expr_supported(target) && fields.iter().all(record_field_supported)
        }

        RustIrExpr::Lambda { body, .. } => expr_supported(body),

        RustIrExpr::App { func, arg, .. } => expr_supported(func) && expr_supported(arg),
        RustIrExpr::Call { func, args, .. } => {
            expr_supported(func) && args.iter().all(expr_supported)
        }

        RustIrExpr::List { items, .. } => items.iter().all(list_item_supported),
        RustIrExpr::Tuple { items, .. } => items.iter().all(expr_supported),
        RustIrExpr::Record { fields, .. } => fields.iter().all(record_field_supported),

        RustIrExpr::FieldAccess { base, .. } => expr_supported(base),
        RustIrExpr::Index { base, index, .. } => expr_supported(base) && expr_supported(index),

        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => expr_supported(cond) && expr_supported(then_branch) && expr_supported(else_branch),

        RustIrExpr::Binary { left, right, .. } => expr_supported(left) && expr_supported(right),

        RustIrExpr::Block {
            block_kind, items, ..
        } => match block_kind {
            // Plain blocks: all items must be individually supported
            RustIrBlockKind::Plain => items.iter().all(block_item_supported),
            // Generate and Resource blocks are compiled natively in Cranelift.
            RustIrBlockKind::Generate | RustIrBlockKind::Resource => true,
            // Do blocks: items must be individually supported
            RustIrBlockKind::Do { .. } => items.iter().all(block_item_supported),
        },
    }
}

/// Peel Lambda wrappers to extract parameter names and the innermost body.
fn peel_params(expr: &RustIrExpr) -> (Vec<String>, &RustIrExpr) {
    let mut params = Vec::new();
    let mut cursor = expr;
    loop {
        match cursor {
            RustIrExpr::Lambda { param, body, .. } => {
                params.push(param.clone());
                cursor = body.as_ref();
            }
            _ => return (params, cursor),
        }
    }
}

/// Collect all global names referenced in an expression (shallow, no dedup).
fn collect_called_globals(expr: &RustIrExpr, out: &mut HashSet<String>) {
    match expr {
        RustIrExpr::Global { name, .. } => {
            out.insert(name.clone());
        }
        RustIrExpr::App { func, arg, .. } => {
            collect_called_globals(func, out);
            collect_called_globals(arg, out);
        }
        RustIrExpr::Call { func, args, .. } => {
            collect_called_globals(func, out);
            for a in args {
                collect_called_globals(a, out);
            }
        }
        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_called_globals(cond, out);
            collect_called_globals(then_branch, out);
            collect_called_globals(else_branch, out);
        }
        RustIrExpr::Binary { left, right, .. } => {
            collect_called_globals(left, out);
            collect_called_globals(right, out);
        }
        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            collect_called_globals(scrutinee, out);
            for arm in arms {
                collect_called_globals(&arm.body, out);
                if let Some(g) = &arm.guard {
                    collect_called_globals(g, out);
                }
            }
        }
        RustIrExpr::Lambda { body, .. } => collect_called_globals(body, out),
        RustIrExpr::Block { items, .. } => {
            for item in items {
                match item {
                    RustIrBlockItem::Bind { expr, .. } | RustIrBlockItem::Expr { expr } => {
                        collect_called_globals(expr, out);
                    }
                    _ => {}
                }
            }
        }
        RustIrExpr::List { items, .. } => {
            for item in items {
                collect_called_globals(&item.expr, out);
            }
        }
        RustIrExpr::Record { fields, .. } => {
            for f in fields {
                collect_called_globals(&f.value, out);
            }
        }
        RustIrExpr::Tuple { items, .. } => {
            for item in items {
                collect_called_globals(item, out);
            }
        }
        RustIrExpr::Patch { target, fields, .. } => {
            collect_called_globals(target, out);
            for f in fields {
                collect_called_globals(&f.value, out);
            }
        }
        RustIrExpr::FieldAccess { base, .. } => collect_called_globals(base, out),
        RustIrExpr::Pipe { func, arg, .. } => {
            collect_called_globals(func, out);
            collect_called_globals(arg, out);
        }
        RustIrExpr::TextInterpolate { parts, .. } => {
            for part in parts {
                if let RustIrTextPart::Expr { expr } = part {
                    collect_called_globals(expr, out);
                }
            }
        }
        RustIrExpr::DebugFn { body, .. } => collect_called_globals(body, out),
        _ => {}
    }
}

/// Build a human-readable suffix for a CgType, used for specialization naming.
fn cg_type_suffix(ty: &CgType) -> String {
    match ty {
        CgType::Int => "Int".into(),
        CgType::Float => "Float".into(),
        CgType::Bool => "Bool".into(),
        CgType::Text => "Text".into(),
        CgType::Unit => "Unit".into(),
        CgType::DateTime => "DateTime".into(),
        CgType::Func(a, b) => format!("{}_to_{}", cg_type_suffix(a), cg_type_suffix(b)),
        CgType::ListOf(elem) => format!("List_{}", cg_type_suffix(elem)),
        CgType::Tuple(items) => {
            let parts: Vec<_> = items.iter().map(|t| cg_type_suffix(t)).collect();
            format!("Tup_{}", parts.join("_"))
        }
        _ => {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            format!("{:?}", ty).hash(&mut hasher);
            format!("h{:x}", hasher.finish())
        }
    }
}

/// Monomorphize polymorphic definitions based on the monomorph plan.
///
/// Returns a `spec_map` mapping original short names to their specialization
/// short names (for call-site routing in the lowering phase).
fn monomorphize_program(
    modules: &mut [rust_ir::RustIrModule],
    monomorph_plan: &HashMap<String, Vec<CgType>>,
) -> HashMap<String, Vec<String>> {
    let mut spec_map: HashMap<String, Vec<String>> = HashMap::new();

    for module in modules.iter_mut() {
        let mut new_defs = Vec::new();
        let mut single_type_updates: Vec<(String, CgType)> = Vec::new();

        for def in module.defs.iter() {
            // Skip defs that already have a concrete type
            if def.cg_type.as_ref().is_some_and(|t| t.is_closed()) {
                continue;
            }
            let qualified = format!("{}.{}", module.name, def.name);
            let Some(instantiations) = monomorph_plan.get(&qualified) else {
                continue;
            };
            if instantiations.is_empty() {
                continue;
            }

            if instantiations.len() == 1 {
                // Single instantiation: set cg_type on the original def directly.
                single_type_updates.push((def.name.clone(), instantiations[0].clone()));
            }

            // Create specialized clones for each concrete type.
            for concrete_type in instantiations {
                let suffix = cg_type_suffix(concrete_type);
                let spec_name = format!("{}$mono_{}", def.name, suffix);
                new_defs.push(RustIrDef {
                    name: spec_name.clone(),
                    inline: def.inline,
                    expr: def.expr.clone(),
                    cg_type: Some(concrete_type.clone()),
                });
                spec_map
                    .entry(def.name.clone())
                    .or_default()
                    .push(spec_name);
            }
        }

        // Apply single-instantiation type updates
        for (name, cg_type) in single_type_updates {
            if let Some(def) = module.defs.iter_mut().find(|d| d.name == name) {
                def.cg_type = Some(cg_type);
            }
        }

        module.defs.extend(new_defs);
    }

    spec_map
}

fn sanitize_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        match ch {
            _ if ch.is_ascii_alphanumeric() => out.push(ch),
            '.' => out.push('_'),
            '+' => out.push_str("_plus_"),
            '-' => out.push_str("_minus_"),
            '*' => out.push_str("_star_"),
            '/' => out.push_str("_slash_"),
            '<' => out.push_str("_lt_"),
            '>' => out.push_str("_gt_"),
            '=' => out.push_str("_eq_"),
            '!' => out.push_str("_bang_"),
            '&' => out.push_str("_amp_"),
            '|' => out.push_str("_pipe_"),
            '^' => out.push_str("_caret_"),
            '%' => out.push_str("_pct_"),
            '~' => out.push_str("_tilde_"),
            _ => out.push_str(&format!("_x{:02x}_", ch as u32)),
        }
    }
    out
}

/// Create a runtime `Value::Builtin` that calls a JIT-compiled function.
pub(crate) fn make_jit_builtin(def_name: &str, arity: usize, func_ptr: usize) -> Value {
    use crate::runtime::values::{BuiltinImpl, BuiltinValue};
    use std::sync::Arc;

    let def_name = def_name.to_string();

    // For arity-0 defs, we call the JIT function immediately and cache the result
    if arity == 0 {
        // Arity-0 non-effect definitions can be eagerly evaluated
        let builtin = Value::Builtin(BuiltinValue {
            imp: Arc::new(BuiltinImpl {
                name: format!("__jit|cranelift|{}", def_name),
                arity: 0,
                func: Arc::new(move |_args: Vec<Value>, runtime: &mut Runtime| {
                    let ctx = unsafe { JitRuntimeCtx::from_runtime(runtime) };
                    let ctx_ptr = &ctx as *const JitRuntimeCtx as usize;
                    let call_args = [ctx_ptr as i64];
                    let result_ptr = unsafe { call_jit_function(func_ptr, &call_args) };
                    if result_ptr == 0 {
                        eprintln!("aivi: JIT function '{}' returned null pointer", def_name);
                        Ok(Value::Unit)
                    } else {
                        Ok(unsafe { super::abi::unbox_value(result_ptr as *mut Value) })
                    }
                }),
            }),
            args: Vec::new(),
            tagged_args: None,
        });
        return builtin;
    }

    // The builtin accumulates args until arity is reached, then calls the JIT code
    Value::Builtin(BuiltinValue {
        imp: Arc::new(BuiltinImpl {
            name: format!("__jit|cranelift|{}", def_name),
            arity,
            func: Arc::new(move |args: Vec<Value>, runtime: &mut Runtime| {
                // Construct JitRuntimeCtx and call the compiled function
                let ctx = unsafe { JitRuntimeCtx::from_runtime(runtime) };
                let ctx_ptr = &ctx as *const JitRuntimeCtx as usize;

                // Box all arguments
                let boxed_args: Vec<*mut Value> =
                    args.into_iter().map(|v| super::abi::box_value(v)).collect();

                // Build call arguments: [ctx_ptr, arg0, arg1, ...]
                let mut call_args: Vec<i64> = Vec::with_capacity(1 + arity);
                call_args.push(ctx_ptr as i64);
                for arg in &boxed_args {
                    call_args.push(*arg as i64);
                }

                // Call the JIT function
                let result_ptr = unsafe { call_jit_function(func_ptr, &call_args) };

                // Check if the JIT function signalled a non-exhaustive match.
                // This lets apply_multi_clause try the next clause.
                if runtime.jit_match_failed {
                    runtime.jit_match_failed = false;
                    // Clean up boxed arguments
                    for arg_ptr in boxed_args {
                        unsafe {
                            drop(Box::from_raw(arg_ptr));
                        }
                    }
                    if result_ptr != 0
                        && !call_args[1..].iter().any(|a| *a == result_ptr)
                    {
                        unsafe {
                            drop(Box::from_raw(result_ptr as *mut Value));
                        }
                    }
                    return Err(RuntimeError::Message(
                        "non-exhaustive match".to_string(),
                    ));
                }

                // Clone the result from the pointer (don't take ownership — the
                // pointer might alias one of the input args).
                let result = if result_ptr == 0 {
                    eprintln!("aivi: JIT function '{}' returned null pointer", def_name);
                    Value::Unit
                } else {
                    let rp = result_ptr as *const Value;
                    unsafe { (*rp).clone() }
                };

                // Drop all boxed arguments. Since we cloned the result above,
                // we won't double-free even if result_ptr == one of the arg ptrs.
                for arg_ptr in boxed_args {
                    unsafe {
                        drop(Box::from_raw(arg_ptr));
                    }
                }

                // If the result_ptr is distinct from all arg_ptrs, drop it too.
                if result_ptr != 0 && !call_args[1..].iter().any(|a| *a == result_ptr) {
                    unsafe {
                        drop(Box::from_raw(result_ptr as *mut Value));
                    }
                }

                Ok(result)
            }),
        }),
        args: Vec::new(),
        tagged_args: None,
    })
}

/// Call a JIT-compiled function at the given address with the given arguments.
///
/// # Safety
/// `func_ptr` must point to valid JIT-compiled code with the matching signature.
pub(crate) unsafe fn call_jit_function(func_ptr: usize, args: &[i64]) -> i64 {
    let code = func_ptr as *const u8;
    match args.len() {
        1 => {
            let f: extern "C" fn(i64) -> i64 = std::mem::transmute(code);
            f(args[0])
        }
        2 => {
            let f: extern "C" fn(i64, i64) -> i64 = std::mem::transmute(code);
            f(args[0], args[1])
        }
        3 => {
            let f: extern "C" fn(i64, i64, i64) -> i64 = std::mem::transmute(code);
            f(args[0], args[1], args[2])
        }
        4 => {
            let f: extern "C" fn(i64, i64, i64, i64) -> i64 = std::mem::transmute(code);
            f(args[0], args[1], args[2], args[3])
        }
        5 => {
            let f: extern "C" fn(i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(code);
            f(args[0], args[1], args[2], args[3], args[4])
        }
        6 => {
            let f: extern "C" fn(i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(code);
            f(args[0], args[1], args[2], args[3], args[4], args[5])
        }
        7 => {
            let f: extern "C" fn(i64, i64, i64, i64, i64, i64, i64) -> i64 =
                std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6],
            )
        }
        8 => {
            let f: extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64) -> i64 =
                std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7],
            )
        }
        9 => {
            let f: extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 =
                std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
            )
        }
        10 => {
            let f: extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 =
                std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9],
            )
        }
        11 => {
            let f: extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 =
                std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10],
            )
        }
        12 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11],
            )
        }
        13 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12],
            )
        }
        14 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13],
            )
        }
        15 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14],
            )
        }
        16 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15],
            )
        }
        n => {
            eprintln!("aivi: call_jit_function: unsupported arity {n} (max 15 params + ctx)");
            0
        }
    }
}

/// Collect all inner Lambda nodes in post-order (innermost first).
/// `bound` tracks variables that are in scope (parameters, let-bindings).
/// Each lambda is returned with its list of captured (free) variables.
fn collect_inner_lambdas<'a>(
    expr: &'a RustIrExpr,
    bound: &mut Vec<String>,
    out: &mut Vec<(&'a RustIrExpr, Vec<String>)>,
) {
    match expr {
        RustIrExpr::Lambda { param, body, .. } => {
            bound.push(param.clone());
            collect_inner_lambdas(body, bound, out);
            bound.pop();

            let mut free = HashSet::new();
            let mut inner_bound = vec![param.clone()];
            collect_free_locals(body, &mut inner_bound, &mut free);
            let mut captured: Vec<String> = free.into_iter().collect();
            captured.sort();

            out.push((expr, captured));
        }
        RustIrExpr::App { func, arg, .. } => {
            collect_inner_lambdas(func, bound, out);
            collect_inner_lambdas(arg, bound, out);
        }
        RustIrExpr::Call { func, args, .. } => {
            collect_inner_lambdas(func, bound, out);
            for a in args {
                collect_inner_lambdas(a, bound, out);
            }
        }
        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_inner_lambdas(cond, bound, out);
            collect_inner_lambdas(then_branch, bound, out);
            collect_inner_lambdas(else_branch, bound, out);
        }
        RustIrExpr::Binary { left, right, .. } => {
            collect_inner_lambdas(left, bound, out);
            collect_inner_lambdas(right, bound, out);
        }
        RustIrExpr::Block { items, .. } => {
            let mark = bound.len();
            for item in items {
                match item {
                    RustIrBlockItem::Bind { pattern, expr } => {
                        collect_inner_lambdas(expr, bound, out);
                        collect_pattern_vars(pattern, bound);
                    }
                    RustIrBlockItem::Expr { expr }
                    | RustIrBlockItem::Yield { expr }
                    | RustIrBlockItem::Recurse { expr }
                    | RustIrBlockItem::Filter { expr } => {
                        collect_inner_lambdas(expr, bound, out);
                    }
                }
            }
            bound.truncate(mark);
        }
        RustIrExpr::FieldAccess { base, .. } => {
            collect_inner_lambdas(base, bound, out);
        }
        RustIrExpr::Index { base, index, .. } => {
            collect_inner_lambdas(base, bound, out);
            collect_inner_lambdas(index, bound, out);
        }
        RustIrExpr::List { items, .. } => {
            for item in items {
                collect_inner_lambdas(&item.expr, bound, out);
            }
        }
        RustIrExpr::Tuple { items, .. } => {
            for item in items {
                collect_inner_lambdas(item, bound, out);
            }
        }
        RustIrExpr::Record { fields, .. } => {
            for f in fields {
                collect_inner_lambdas(&f.value, bound, out);
            }
        }
        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            collect_inner_lambdas(scrutinee, bound, out);
            for arm in arms {
                let mark = bound.len();
                collect_pattern_vars(&arm.pattern, bound);
                if let Some(g) = &arm.guard {
                    collect_inner_lambdas(g, bound, out);
                }
                collect_inner_lambdas(&arm.body, bound, out);
                bound.truncate(mark);
            }
        }
        RustIrExpr::Patch { target, fields, .. } => {
            collect_inner_lambdas(target, bound, out);
            for f in fields {
                collect_inner_lambdas(&f.value, bound, out);
            }
        }
        RustIrExpr::TextInterpolate { parts, .. } => {
            for p in parts {
                if let RustIrTextPart::Expr { expr } = p {
                    collect_inner_lambdas(expr, bound, out);
                }
            }
        }
        RustIrExpr::Pipe { func, arg, .. } => {
            collect_inner_lambdas(func, bound, out);
            collect_inner_lambdas(arg, bound, out);
        }
        RustIrExpr::DebugFn { body, .. } => {
            collect_inner_lambdas(body, bound, out);
        }
        // Leaf expressions don't contain lambdas
        RustIrExpr::Local { .. }
        | RustIrExpr::Global { .. }
        | RustIrExpr::Builtin { .. }
        | RustIrExpr::ConstructorValue { .. }
        | RustIrExpr::LitBool { .. }
        | RustIrExpr::LitNumber { .. }
        | RustIrExpr::LitString { .. }
        | RustIrExpr::LitSigil { .. }
        | RustIrExpr::LitDateTime { .. }
        | RustIrExpr::Raw { .. } => {}
    }
}

/// Collect variable names bound by a pattern.
fn collect_pattern_vars(pat: &RustIrPattern, bound: &mut Vec<String>) {
    match pat {
        RustIrPattern::Var { name, .. } => bound.push(name.clone()),
        RustIrPattern::At { name, pattern, .. } => {
            bound.push(name.clone());
            collect_pattern_vars(pattern, bound);
        }
        RustIrPattern::Constructor { args, .. } => {
            for a in args {
                collect_pattern_vars(a, bound);
            }
        }
        RustIrPattern::Tuple { items, .. } => {
            for i in items {
                collect_pattern_vars(i, bound);
            }
        }
        RustIrPattern::List { items, rest, .. } => {
            for i in items {
                collect_pattern_vars(i, bound);
            }
            if let Some(r) = rest {
                collect_pattern_vars(r, bound);
            }
        }
        RustIrPattern::Record { fields, .. } => {
            for f in fields {
                collect_pattern_vars(&f.pattern, bound);
            }
        }
        RustIrPattern::Literal { .. } | RustIrPattern::Wildcard { .. } => {}
    }
}

/// Collect free local variable references in an expression.
/// `bound` tracks variables currently in scope; `free` accumulates unbound locals.
fn collect_free_locals(expr: &RustIrExpr, bound: &mut Vec<String>, free: &mut HashSet<String>) {
    match expr {
        RustIrExpr::Local { name, .. } => {
            if !bound.contains(name) {
                free.insert(name.clone());
            }
        }
        RustIrExpr::Lambda { param, body, .. } => {
            bound.push(param.clone());
            collect_free_locals(body, bound, free);
            bound.pop();
        }
        RustIrExpr::App { func, arg, .. } => {
            collect_free_locals(func, bound, free);
            collect_free_locals(arg, bound, free);
        }
        RustIrExpr::Call { func, args, .. } => {
            collect_free_locals(func, bound, free);
            for a in args {
                collect_free_locals(a, bound, free);
            }
        }
        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_free_locals(cond, bound, free);
            collect_free_locals(then_branch, bound, free);
            collect_free_locals(else_branch, bound, free);
        }
        RustIrExpr::Binary { left, right, .. } => {
            collect_free_locals(left, bound, free);
            collect_free_locals(right, bound, free);
        }
        RustIrExpr::Block { items, .. } => {
            let mark = bound.len();
            for item in items {
                match item {
                    RustIrBlockItem::Bind { pattern, expr } => {
                        collect_free_locals(expr, bound, free);
                        collect_pattern_vars(pattern, bound);
                    }
                    RustIrBlockItem::Expr { expr }
                    | RustIrBlockItem::Yield { expr }
                    | RustIrBlockItem::Recurse { expr }
                    | RustIrBlockItem::Filter { expr } => {
                        collect_free_locals(expr, bound, free);
                    }
                }
            }
            bound.truncate(mark);
        }
        RustIrExpr::FieldAccess { base, .. } => {
            collect_free_locals(base, bound, free);
        }
        RustIrExpr::Index { base, index, .. } => {
            collect_free_locals(base, bound, free);
            collect_free_locals(index, bound, free);
        }
        RustIrExpr::List { items, .. } => {
            for item in items {
                collect_free_locals(&item.expr, bound, free);
            }
        }
        RustIrExpr::Tuple { items, .. } => {
            for item in items {
                collect_free_locals(item, bound, free);
            }
        }
        RustIrExpr::Record { fields, .. } => {
            for f in fields {
                collect_free_locals(&f.value, bound, free);
            }
        }
        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            collect_free_locals(scrutinee, bound, free);
            for arm in arms {
                let mark = bound.len();
                collect_pattern_vars(&arm.pattern, bound);
                if let Some(g) = &arm.guard {
                    collect_free_locals(g, bound, free);
                }
                collect_free_locals(&arm.body, bound, free);
                bound.truncate(mark);
            }
        }
        RustIrExpr::Patch { target, fields, .. } => {
            collect_free_locals(target, bound, free);
            for f in fields {
                collect_free_locals(&f.value, bound, free);
            }
        }
        RustIrExpr::TextInterpolate { parts, .. } => {
            for p in parts {
                if let RustIrTextPart::Expr { expr } = p {
                    collect_free_locals(expr, bound, free);
                }
            }
        }
        RustIrExpr::Pipe { func, arg, .. } => {
            collect_free_locals(func, bound, free);
            collect_free_locals(arg, bound, free);
        }
        RustIrExpr::DebugFn { body, .. } => {
            collect_free_locals(body, bound, free);
        }
        // Leaves with no free locals
        RustIrExpr::Global { .. }
        | RustIrExpr::Builtin { .. }
        | RustIrExpr::ConstructorValue { .. }
        | RustIrExpr::LitBool { .. }
        | RustIrExpr::LitNumber { .. }
        | RustIrExpr::LitString { .. }
        | RustIrExpr::LitSigil { .. }
        | RustIrExpr::LitDateTime { .. }
        | RustIrExpr::Raw { .. } => {}
    }
}
