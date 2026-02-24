//! End-to-end Cranelift JIT compilation and execution pipeline.
//!
//! `run_cranelift_jit` is the new entrypoint that replaces `run_native_jit`.
//! It compiles ALL definitions to Cranelift IR, builds a JIT module, then
//! executes `main`.

use std::collections::HashMap;

use cranelift_codegen::ir::{types, AbiParam, Function, InstBuilder};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{Linkage, Module};

use crate::cg_type::CgType;
use crate::hir::HirProgram;
use crate::runtime::{
    build_runtime_from_program, run_main_effect, Runtime,
};
use crate::runtime::values::Value;
use crate::{kernel, rust_ir};
use crate::rust_ir::{RustIrDef, RustIrExpr, RustIrBlockKind};
use crate::AiviError;

use super::abi::JitRuntimeCtx;
use super::jit_module::create_jit_module;
use super::lower::{DeclaredHelpers, LowerCtx, declare_helpers};

/// Pointer type used throughout.
const PTR: cranelift_codegen::ir::Type = types::I64;

/// Compile and execute an AIVI program entirely via Cranelift JIT.
///
/// This replaces `run_native_jit`: every definition is compiled to native
/// machine code, then `main` is executed.
pub fn run_cranelift_jit(
    program: HirProgram,
    cg_types: HashMap<String, HashMap<String, CgType>>,
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

    // 4. Create JIT module with runtime helpers registered
    let mut module = create_jit_module()
        .map_err(|e| AiviError::Runtime(format!("cranelift jit init: {e}")))?;

    // 5. Declare runtime helper imports in the module
    let helpers = declare_helpers(&mut module)
        .map_err(|e| AiviError::Runtime(format!("cranelift declare helpers: {e}")))?;

    // 6. Compile each definition to a Cranelift function.
    //    Skip arity-0 do-block definitions — the interpreter handles those lazily.
    struct PendingDef {
        name: String,
        func_name: String,
        func_id: cranelift_module::FuncId,
        arity: usize,
    }
    let mut pending: Vec<PendingDef> = Vec::new();
    let mut declared_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for ir_module in &rust_program.modules {
        for def in &ir_module.defs {
            let (params, body) = peel_params(&def.expr);
            // Skip arity-0 effect/block defs — these produce lazy Effect values
            // that the interpreter handles correctly.
            if params.is_empty() {
                if matches!(body, RustIrExpr::Block { block_kind: RustIrBlockKind::Do { .. }, .. }) {
                    continue;
                }
            }

            let qualified = format!("{}.{}", ir_module.name, def.name);
            let func_name = format!("__aivi_jit_{}", sanitize_name(&qualified));
            // Skip duplicates (e.g., re-exported defs across modules)
            if declared_names.contains(&func_name) {
                continue;
            }
            declared_names.insert(func_name);

            match compile_definition(&mut module, &helpers, def, &qualified) {
                Ok(Some((func_id, func_name, arity))) => {
                    pending.push(PendingDef { name: def.name.clone(), func_name, func_id, arity });
                }
                Ok(None) => {}
                Err(e) => {
                    eprintln!("warning: cranelift compile {}: {e}", qualified);
                }
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
        let jit_value = make_jit_builtin(&pd.name, pd.arity, ptr as usize);
        compiled_globals.insert(pd.name.clone(), jit_value);
    }

    // 8. Install compiled globals into the runtime (overriding interpreter thunks)
    for (name, value) in compiled_globals {
        runtime.ctx.globals.set(name, value);
    }

    // 9. Run main
    run_main_effect(&mut runtime)
}

/// Compile a single `RustIrDef` to native code via Cranelift.
///
/// Returns `Ok(Some((func_id, func_name, arity)))` if compilation succeeded,
/// `Ok(None)` if the definition can't be compiled yet,
/// or `Err(msg)` on failure.
fn compile_definition(
    module: &mut cranelift_jit::JITModule,
    helpers: &DeclaredHelpers,
    def: &RustIrDef,
    qualified_name: &str,
) -> Result<Option<(cranelift_module::FuncId, String, usize)>, String> {
    let (params, body) = peel_params(&def.expr);
    let arity = params.len();

    // Build function signature: (ctx, ...args) -> result
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(PTR)); // ctx
    for _ in 0..arity {
        sig.params.push(AbiParam::new(PTR)); // each arg
    }
    sig.returns.push(AbiParam::new(PTR)); // return value

    let func_name = format!("__aivi_jit_{}", sanitize_name(qualified_name));
    let func_id = module
        .declare_function(&func_name, Linkage::Local, &sig)
        .map_err(|e| format!("declare {}: {e}", func_name))?;

    // Build the function body
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

        let mut lower_ctx = LowerCtx::new(ctx_param, &helper_refs);

        // Bind parameters
        for (i, param_name) in params.iter().enumerate() {
            lower_ctx.locals.insert(param_name.clone(), block_params[i + 1]);
        }

        // Lower the body expression
        let result = lower_ctx.lower_expr(&mut builder, body);
        builder.ins().return_(&[result]);

        builder.finalize();
    }

    // Define (but don't finalize yet — caller finalizes all at once)
    let mut ctx = module.make_context();
    ctx.func = function;
    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| format!("define {}: {e}", func_name))?;
    module.clear_context(&mut ctx);

    Ok(Some((func_id, func_name, arity)))
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
