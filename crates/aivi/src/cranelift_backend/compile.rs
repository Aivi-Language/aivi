//! End-to-end Cranelift JIT compilation and execution pipeline.
//!
//! `run_cranelift_jit` is the new entrypoint that replaces `run_native_jit`.
//! It compiles ALL definitions to Cranelift IR, builds a JIT module, then
//! executes `main`.

use std::collections::{HashMap, HashSet};

use cranelift_codegen::ir::{types, AbiParam, Function, InstBuilder};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::JITModule;
use cranelift_module::{Linkage, Module};

use crate::cg_type::CgType;
use crate::hir::HirProgram;
use crate::runtime::{
    build_runtime_from_program, run_main_effect, Runtime,
};
use crate::runtime::values::Value;
use crate::{kernel, rust_ir};
use crate::rust_ir::{
    RustIrBlockItem, RustIrBlockKind, RustIrDef, RustIrExpr, RustIrListItem, RustIrPattern,
    RustIrPathSegment, RustIrRecordField, RustIrTextPart,
};
use crate::AiviError;

use super::abi::JitRuntimeCtx;
use super::jit_module::create_jit_module;
use super::lower::{CompiledLambda, DeclaredHelpers, JitFuncDecl, JitFuncInfo, LowerCtx, declare_helpers, decompose_func_type};

/// Pointer type used throughout.
const PTR: cranelift_codegen::ir::Type = types::I64;

/// Compile and execute an AIVI program entirely via Cranelift JIT.
///
/// This replaces `run_native_jit`: every definition is compiled to native
/// machine code, then `main` is executed.
pub fn run_cranelift_jit(
    program: HirProgram,
    cg_types: HashMap<String, HashMap<String, CgType>>,
    _monomorph_plan: HashMap<String, Vec<CgType>>,
) -> Result<(), AiviError> {
    // 1. Build the interpreter runtime (for builtins, thunks, globals).
    //    During the migration this is still needed because lambdas/closures
    //    and complex builtins fall back to the interpreter.
    let mut runtime = build_runtime_from_program(program.clone())?;

    // 2. Lower HIR → Kernel → RustIR
    let kernel_program = kernel::lower_hir(program);
    let mut rust_program = rust_ir::lower_kernel(kernel_program)?;

    // 3. Annotate each def with its CgType
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

    // 3b. Monomorphize: create specialized copies of polymorphic defs
    //     based on the call-site type recordings from Phase 6.
    //     Single-instantiation defs get their cg_type set directly.
    //     Multi-instantiation defs get cloned with specialized names.
    let spec_map = monomorphize_program(&mut rust_program.modules, &_monomorph_plan);

    // 4. Create JIT module with runtime helpers registered
    let mut module = create_jit_module()
        .map_err(|e| AiviError::Runtime(format!("cranelift jit init: {e}")))?;

    // 5. Declare runtime helper imports in the module
    let helpers = declare_helpers(&mut module)
        .map_err(|e| AiviError::Runtime(format!("cranelift declare helpers: {e}")))?;

    // 6. Two-pass compilation for direct calls between JIT functions.
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
    }
    let mut declared_defs: Vec<DeclaredDef> = Vec::new();
    let mut declared_names: HashSet<String> = HashSet::new();
    let mut jit_func_ids: HashMap<String, JitFuncDecl> = HashMap::new();

    // Pass 1: Declare all eligible functions
    for ir_module in &rust_program.modules {
        for def in &ir_module.defs {
            let (params, body) = peel_params(&def.expr);
            if params.is_empty() {
                if matches!(body, RustIrExpr::Block { block_kind: RustIrBlockKind::Do { .. }, .. }) {
                    continue;
                }
            }
            if params.len() > 7 || !expr_supported(body) {
                continue;
            }
            let qualified = format!("{}.{}", ir_module.name, def.name);
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

            let func_id = module
                .declare_function(&func_name, Linkage::Local, &sig)
                .map_err(|e| AiviError::Runtime(format!("declare {}: {e}", func_name)))?;

            // Extract typed param/return info from CgType
            let (param_types, return_type) = if let Some(cg_ty) = &def.cg_type {
                decompose_func_type(cg_ty, arity)
            } else {
                (vec![None; arity], None)
            };

            declared_defs.push(DeclaredDef {
                def,
                module_name: ir_module.name.clone(),
                qualified: qualified.clone(),
                func_name,
                func_id,
                arity,
                param_types: param_types.clone(),
                return_type: return_type.clone(),
            });

            // Register only under the qualified name — short names collide across modules
            jit_func_ids.insert(qualified, JitFuncDecl {
                func_id,
                arity,
                param_types,
                return_type,
            });
        }
    }

    // Pass 2: Compile function bodies.
    // Track successfully-compiled functions so later functions can direct-call them.
    struct PendingDef {
        name: String,
        qualified: String,
        func_id: cranelift_module::FuncId,
        arity: usize,
    }
    let mut pending: Vec<PendingDef> = Vec::new();
    let mut lambda_counter: usize = 0;
    let mut compiled_decls: HashMap<String, JitFuncDecl> = HashMap::new();

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
            &mut runtime,
            &spec_map,
        ) {
            Ok(()) => {
                pending.push(PendingDef {
                    name: dd.def.name.clone(),
                    qualified: dd.qualified.clone(),
                    func_id: dd.func_id,
                    arity: dd.arity,
                });
                // Register under qualified name only
                compiled_decls.insert(dd.qualified.clone(), JitFuncDecl {
                    func_id: dd.func_id,
                    arity: dd.arity,
                    param_types: dd.param_types.clone(),
                    return_type: dd.return_type.clone(),
                });
            }
            Err(e) => {
                eprintln!("warning: cranelift compile {}: {e}", dd.qualified);
            }
        }
    }

    // 7. Finalize all definitions at once, then extract pointers
    module
        .finalize_definitions()
        .map_err(|e| AiviError::Runtime(format!("cranelift finalize: {e}")))?;

    let mut compiled_globals: HashMap<String, Value> = HashMap::new();
    for pd in &pending {
        let ptr = module.get_finalized_function(pd.func_id);
        let jit_value = make_jit_builtin(&pd.qualified, pd.arity, ptr as usize);
        compiled_globals.insert(pd.name.clone(), jit_value.clone());
        compiled_globals.insert(pd.qualified.clone(), jit_value);
    }

    // 8. Install compiled globals into the runtime (overriding interpreter thunks).
    //    For unqualified (short) names, only set if not yet present — this mirrors
    //    `build_runtime_from_program` which keeps the first definition and lets
    //    module order determine priority.
    for (name, value) in compiled_globals {
        // Source defs cannot shadow builtins.
        if let Some(Value::Builtin(_)) = runtime.ctx.globals.get(&name) {
            continue;
        }
        runtime.ctx.globals.set(name, value);
    }

    // 9. Run main
    run_main_effect(&mut runtime)
}

/// Compile the body of a pre-declared JIT function.
///
/// The function has already been declared via `module.declare_function`; this
/// fills in the body IR. Returns `Ok(())` on success or `Err(msg)` on failure.
fn compile_definition_body(
    module: &mut JITModule,
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
    runtime: &mut Runtime,
    spec_map: &HashMap<String, Vec<String>>,
) -> Result<(), String> {
    let (params, body) = peel_params(&def.expr);

    // --- Pre-compile inner lambdas ---
    let mut lambdas: Vec<(&RustIrExpr, Vec<String>)> = Vec::new();
    collect_inner_lambdas(body, &mut Vec::new(), &mut lambdas);

    let mut compiled_lambdas: HashMap<usize, CompiledLambda> = HashMap::new();

    // Compile each lambda as a function: (ctx, cap0, cap1, ..., param) -> result
    // Lambdas are collected bottom-up (innermost first), so nested lambdas
    // appear before their parents.  Global lookups at runtime resolve
    // forward references.
    struct PendingLambda {
        func_id: cranelift_module::FuncId,
        global_name: String,
        total_arity: usize,
    }
    let mut pending_lambdas: Vec<PendingLambda> = Vec::new();

    for (lambda_expr, captured_vars) in &lambdas {
        let RustIrExpr::Lambda { param, body, .. } = lambda_expr else { continue };

        let total_arity = captured_vars.len() + 1; // captures + the actual param
        if total_arity > 7 {
            // Too many captures + param for call_jit_function
            continue;
        }

        let global_name = format!("__jit_lambda_{}", *lambda_counter);
        *lambda_counter += 1;

        // Leak the name so the raw pointer embedded in JIT code remains valid.
        let global_name_static: &'static str = Box::leak(global_name.clone().into_boxed_str());

        // Store in compiled_lambdas so nested lambdas can reference it
        let key = *lambda_expr as *const RustIrExpr as usize;
        compiled_lambdas.insert(key, CompiledLambda {
            global_name: global_name_static,
            captured_vars: captured_vars.clone(),
        });

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

            let empty_jit_funcs: HashMap<String, JitFuncInfo> = HashMap::new();
            let empty_spec_map: HashMap<String, Vec<String>> = HashMap::new();
            let mut lower_ctx = LowerCtx::new(ctx_param, &helper_refs, &compiled_lambdas, &empty_jit_funcs, &empty_spec_map);

            // Bind captured vars as leading params (boxed — received as *mut Value)
            for (i, var_name) in captured_vars.iter().enumerate() {
                lower_ctx.locals.insert(var_name.clone(), super::lower::TypedValue::boxed(block_params[i + 1]));
            }
            // Bind the actual lambda parameter (boxed)
            lower_ctx.locals.insert(param.clone(), super::lower::TypedValue::boxed(block_params[captured_vars.len() + 1]));

            let result = lower_ctx.lower_expr(&mut builder, body);
            let result_boxed = lower_ctx.ensure_boxed(&mut builder, result);
            builder.ins().return_(&[result_boxed]);
            builder.finalize();
        }

        let mut ctx = module.make_context();
        ctx.func = function;
        module
            .define_function(func_id, &mut ctx)
            .map_err(|e| format!("define lambda {}: {e}", func_name))?;
        module.clear_context(&mut ctx);

        pending_lambdas.push(PendingLambda {
            func_id,
            global_name: global_name.clone(),
            total_arity,
        });
    }

    // Finalize lambda functions and install them as globals
    if !pending_lambdas.is_empty() {
        module
            .finalize_definitions()
            .map_err(|e| format!("finalize lambdas: {e}"))?;

        for pl in &pending_lambdas {
            let ptr = module.get_finalized_function(pl.func_id);
            let jit_value = make_jit_builtin(&pl.global_name, pl.total_arity, ptr as usize);
            runtime.ctx.globals.set(pl.global_name.clone(), jit_value);
        }
    }

    // --- Compile the main body (function was pre-declared in Pass 1) ---
    let sig = module.declarations().get_function_decl(func_id).signature.clone();
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
            local_jit_funcs.insert(name.clone(), JitFuncInfo {
                func_ref,
                arity: decl.arity,
                param_types: decl.param_types.clone(),
                return_type: decl.return_type.clone(),
            });
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
                    local_jit_funcs.insert(spec_short.clone(), JitFuncInfo {
                        func_ref,
                        arity: sd.arity,
                        param_types: sd.param_types.clone(),
                        return_type: sd.return_type.clone(),
                    });
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

        let mut lower_ctx = LowerCtx::new(ctx_param, &helper_refs, &compiled_lambdas, &local_jit_funcs, &local_spec_map);

        // Bind params with typed unboxing when types are known
        let param_names: Vec<String> = params.iter().map(|s| s.to_string()).collect();
        lower_ctx.bind_typed_params(&mut builder, &param_names, &block_params, param_types);

        let result = lower_ctx.lower_expr(&mut builder, body);
        let result_boxed = lower_ctx.ensure_boxed(&mut builder, result);
        builder.ins().return_(&[result_boxed]);
        builder.finalize();
    }

    let mut ctx = module.make_context();
    ctx.func = function;
    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| format!("define {}: {e}", qualified_name))?;
    module.clear_context(&mut ctx);

    Ok(())
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
    !item.spread && expr_supported(&item.expr)
}

fn pattern_supported(pattern: &RustIrPattern) -> bool {
    match pattern {
        RustIrPattern::Wildcard { .. } | RustIrPattern::Var { .. } | RustIrPattern::Literal { .. } => true,
        RustIrPattern::At { pattern, .. } => pattern_supported(pattern),
        RustIrPattern::Constructor { args, .. } => args.iter().all(pattern_supported),
        RustIrPattern::Tuple { items, .. } => items.iter().all(pattern_supported),
        RustIrPattern::List { items, rest, .. } => {
            items.iter().all(pattern_supported)
                && rest.as_deref().map_or(true, pattern_supported)
        }
        RustIrPattern::Record { fields, .. } => {
            fields.iter().all(|f| pattern_supported(&f.pattern))
        }
    }
}

fn block_item_supported(item: &RustIrBlockItem) -> bool {
    match item {
        RustIrBlockItem::Bind { pattern, expr } => pattern_supported(pattern) && expr_supported(expr),
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

        RustIrExpr::LitNumber { text, .. } => text.parse::<i64>().is_ok() || text.parse::<f64>().is_ok(),

        // These have non-trivial runtime semantics we haven't matched yet.
        RustIrExpr::LitSigil { .. }
        | RustIrExpr::LitDateTime { .. }
        | RustIrExpr::TextInterpolate { .. }
        | RustIrExpr::DebugFn { .. }
        | RustIrExpr::Pipe { .. } => true,

        RustIrExpr::Match { scrutinee, arms, .. } => {
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
            block_kind,
            items,
            ..
        } => match block_kind {
            // Plain blocks: all items must be individually supported
            RustIrBlockKind::Plain => items.iter().all(block_item_supported),
            // Generate and Resource blocks are delegated to the interpreter;
            // the items don't need to be individually supported since they'll
            // be evaluated by the interpreter at runtime.
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
        RustIrExpr::Global { name, .. } => { out.insert(name.clone()); }
        RustIrExpr::App { func, arg, .. } => {
            collect_called_globals(func, out);
            collect_called_globals(arg, out);
        }
        RustIrExpr::Call { func, args, .. } => {
            collect_called_globals(func, out);
            for a in args { collect_called_globals(a, out); }
        }
        RustIrExpr::If { cond, then_branch, else_branch, .. } => {
            collect_called_globals(cond, out);
            collect_called_globals(then_branch, out);
            collect_called_globals(else_branch, out);
        }
        RustIrExpr::Binary { left, right, .. } => {
            collect_called_globals(left, out);
            collect_called_globals(right, out);
        }
        RustIrExpr::Match { scrutinee, arms, .. } => {
            collect_called_globals(scrutinee, out);
            for arm in arms {
                collect_called_globals(&arm.body, out);
                if let Some(g) = &arm.guard { collect_called_globals(g, out); }
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
            for item in items { collect_called_globals(&item.expr, out); }
        }
        RustIrExpr::Record { fields, .. } => {
            for f in fields { collect_called_globals(&f.value, out); }
        }
        RustIrExpr::Tuple { items, .. } => {
            for item in items { collect_called_globals(item, out); }
        }
        RustIrExpr::Patch { target, fields, .. } => {
            collect_called_globals(target, out);
            for f in fields { collect_called_globals(&f.value, out); }
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
fn make_jit_builtin(def_name: &str, arity: usize, func_ptr: usize) -> Value {
    use std::sync::Arc;
    use crate::runtime::values::{BuiltinImpl, BuiltinValue};

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
                let boxed_args: Vec<*mut Value> = args
                    .into_iter()
                    .map(|v| super::abi::box_value(v))
                    .collect();

                // Build call arguments: [ctx_ptr, arg0, arg1, ...]
                let mut call_args: Vec<i64> = Vec::with_capacity(1 + arity);
                call_args.push(ctx_ptr as i64);
                for arg in &boxed_args {
                    call_args.push(*arg as i64);
                }

                // Call the JIT function
                let result_ptr = unsafe {
                    call_jit_function(func_ptr, &call_args)
                };

                // Clone the result from the pointer (don't take ownership — the
                // pointer might alias one of the input args).
                let result = if result_ptr == 0 {
                    Value::Unit
                } else {
                    let rp = result_ptr as *const Value;
                    unsafe { (*rp).clone() }
                };

                // Drop all boxed arguments. Since we cloned the result above,
                // we won't double-free even if result_ptr == one of the arg ptrs.
                for arg_ptr in boxed_args {
                    unsafe { drop(Box::from_raw(arg_ptr)); }
                }

                // If the result_ptr is distinct from all arg_ptrs, drop it too.
                if result_ptr != 0
                    && !call_args[1..].iter().any(|a| *a == result_ptr)
                {
                    unsafe { drop(Box::from_raw(result_ptr as *mut Value)); }
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
unsafe fn call_jit_function(func_ptr: usize, args: &[i64]) -> i64 {
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
            let f: extern "C" fn(i64, i64, i64, i64, i64, i64) -> i64 =
                std::mem::transmute(code);
            f(args[0], args[1], args[2], args[3], args[4], args[5])
        }
        7 => {
            let f: extern "C" fn(i64, i64, i64, i64, i64, i64, i64) -> i64 =
                std::mem::transmute(code);
            f(args[0], args[1], args[2], args[3], args[4], args[5], args[6])
        }
        8 => {
            let f: extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64) -> i64 =
                std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7],
            )
        }
        n => panic!("call_jit_function: unsupported arity {n} (max 7 params + ctx)"),
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
        RustIrExpr::If { cond, then_branch, else_branch, .. } => {
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
        RustIrExpr::Match { scrutinee, arms, .. } => {
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
fn collect_free_locals(
    expr: &RustIrExpr,
    bound: &mut Vec<String>,
    free: &mut HashSet<String>,
) {
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
        RustIrExpr::If { cond, then_branch, else_branch, .. } => {
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
        RustIrExpr::Match { scrutinee, arms, .. } => {
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
