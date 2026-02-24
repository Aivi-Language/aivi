use std::collections::{BTreeSet, HashMap};

use cranelift_codegen::ir::{types, AbiParam, Function, InstBuilder, Value};
use cranelift_codegen::isa::CallConv;
use cranelift_codegen::settings::Configurable;
use cranelift_codegen::{settings, Context};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};

use crate::rust_ir::cg_type::CgType;
use crate::rust_ir::RustIrExpr;

use super::typed_expr::TypedCtx;
use super::typed_mir::{lower_typed_mir, TypedMirExpr, TypedMirFunction, TypedMirTerminator};

pub(crate) struct RuntimeLowering {
    pub(crate) function: Function,
    pub(crate) param_names: Vec<String>,
}

pub(super) fn emit_typed_via_cranelift(
    expr: &RustIrExpr,
    ty: &CgType,
    ctx: &TypedCtx,
    _indent: usize,
) -> Option<String> {
    let mir = lower_typed_mir(expr, ty, ctx)?;
    lower_with_cranelift(&mir, ty, ctx)?;
    let body = render_rust_body_from_mir(&mir, ty, ctx)?;
    Some(format!("/* typed-clif */ {body}"))
}

pub(super) fn cranelift_lowering_comment(
    expr: &RustIrExpr,
    ty: &CgType,
    ctx: &TypedCtx,
) -> Option<String> {
    let mir = lower_typed_mir(expr, ty, ctx)?;
    let (function, _) = lower_with_cranelift(&mir, ty, ctx)?;
    let mut text = String::new();
    text.push_str("clif.lowering.begin\n");
    text.push_str(&function.to_string());
    text.push_str("\nclif.lowering.end");
    Some(text)
}

pub(crate) fn lower_for_runtime(
    expr: &RustIrExpr,
    ty: &CgType,
    globals: &HashMap<String, CgType>,
    locals: &[(String, CgType)],
) -> Option<RuntimeLowering> {
    let mut ctx = TypedCtx::new(globals.clone());
    for (name, local_ty) in locals {
        ctx.with_runtime_local(name, local_ty.clone());
    }
    let mir = lower_typed_mir(expr, ty, &ctx)?;
    let (function, param_names) = lower_with_cranelift(&mir, ty, &ctx)?;
    Some(RuntimeLowering {
        function,
        param_names,
    })
}

fn lower_with_cranelift(
    mir: &TypedMirFunction,
    ret_ty: &CgType,
    ctx: &TypedCtx,
) -> Option<(Function, Vec<String>)> {
    let mut sig = cranelift_codegen::ir::Signature::new(CallConv::SystemV);
    let mut param_names = BTreeSet::new();
    collect_mir_names(mir, &mut param_names);
    let mut name_order: Vec<String> = param_names.into_iter().collect();
    name_order.sort();
    for name in &name_order {
        let clif_ty = clif_type(ctx.lookup(name)?)?;
        sig.params.push(AbiParam::new(clif_ty));
    }
    sig.returns.push(AbiParam::new(clif_type(ret_ty)?));

    let mut function =
        Function::with_name_signature(cranelift_codegen::ir::UserFuncName::user(0, 0), sig);
    let mut fb_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut function, &mut fb_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let mut params = HashMap::new();
        for (idx, name) in name_order.iter().enumerate() {
            params.insert(name.clone(), builder.block_params(entry)[idx]);
        }

        match &mir.blocks.get(&mir.entry)?.terminator {
            TypedMirTerminator::Return(expr) => {
                let value = emit_expr(&mut builder, &params, expr, ret_ty, ctx)?;
                builder.ins().return_(&[value]);
            }
            TypedMirTerminator::Branch {
                cond,
                then_bb,
                else_bb,
            } => {
                let cond_value = emit_expr(&mut builder, &params, cond, &CgType::Bool, ctx)?;
                let then_block = builder.create_block();
                let else_block = builder.create_block();
                builder
                    .ins()
                    .brif(cond_value, then_block, &[], else_block, &[]);
                builder.seal_block(then_block);
                builder.seal_block(else_block);

                builder.switch_to_block(then_block);
                let then_expr = match &mir.blocks.get(then_bb)?.terminator {
                    TypedMirTerminator::Return(expr) => expr,
                    TypedMirTerminator::Branch { .. } => return None,
                };
                let then_value = emit_expr(&mut builder, &params, then_expr, ret_ty, ctx)?;
                builder.ins().return_(&[then_value]);

                builder.switch_to_block(else_block);
                let else_expr = match &mir.blocks.get(else_bb)?.terminator {
                    TypedMirTerminator::Return(expr) => expr,
                    TypedMirTerminator::Branch { .. } => return None,
                };
                let else_value = emit_expr(&mut builder, &params, else_expr, ret_ty, ctx)?;
                builder.ins().return_(&[else_value]);
            }
        }

        builder.finalize();
    }

    let mut flags = settings::builder();
    flags.enable("is_pic").ok()?;
    let flag_values = settings::Flags::new(flags);
    let mut codegen_ctx = Context::for_function(function);
    codegen_ctx.compute_cfg();
    codegen_ctx.compute_domtree();
    codegen_ctx.verify(&flag_values).ok()?;
    Some((codegen_ctx.func, name_order))
}

fn emit_expr(
    builder: &mut FunctionBuilder<'_>,
    params: &HashMap<String, Value>,
    expr: &TypedMirExpr,
    ty: &CgType,
    ctx: &TypedCtx,
) -> Option<Value> {
    match expr {
        TypedMirExpr::Int(value) if matches!(ty, CgType::Int) => {
            Some(builder.ins().iconst(types::I64, *value))
        }
        TypedMirExpr::Float(value) if matches!(ty, CgType::Float) => {
            Some(builder.ins().f64const(*value))
        }
        TypedMirExpr::Bool(value) if matches!(ty, CgType::Bool) => {
            Some(builder.ins().iconst(types::I8, i64::from(*value)))
        }
        TypedMirExpr::Local(name) | TypedMirExpr::Global(name) => {
            let expected = ctx.lookup(name)?;
            if expected != ty {
                return None;
            }
            params.get(name).copied()
        }
        TypedMirExpr::Binary { op, left, right } => match ty {
            CgType::Int => {
                let l = emit_expr(builder, params, left, ty, ctx)?;
                let r = emit_expr(builder, params, right, ty, ctx)?;
                match op.as_str() {
                    "+" => Some(builder.ins().iadd(l, r)),
                    "-" => Some(builder.ins().isub(l, r)),
                    "*" => Some(builder.ins().imul(l, r)),
                    "/" => Some(builder.ins().sdiv(l, r)),
                    "%" => Some(builder.ins().srem(l, r)),
                    _ => None,
                }
            }
            CgType::Float => {
                let l = emit_expr(builder, params, left, ty, ctx)?;
                let r = emit_expr(builder, params, right, ty, ctx)?;
                match op.as_str() {
                    "+" => Some(builder.ins().fadd(l, r)),
                    "-" => Some(builder.ins().fsub(l, r)),
                    "*" => Some(builder.ins().fmul(l, r)),
                    "/" => Some(builder.ins().fdiv(l, r)),
                    _ => None,
                }
            }
            CgType::Bool => {
                let left_bool = emit_expr(builder, params, left, &CgType::Bool, ctx)?;
                let right_bool = emit_expr(builder, params, right, &CgType::Bool, ctx)?;
                match op.as_str() {
                    "&&" => Some(builder.ins().band(left_bool, right_bool)),
                    "||" => Some(builder.ins().bor(left_bool, right_bool)),
                    _ => None,
                }
            }
            _ => None,
        },
        TypedMirExpr::Opaque(_) => None,
        _ => None,
    }
}

fn clif_type(ty: &CgType) -> Option<cranelift_codegen::ir::Type> {
    match ty {
        CgType::Int => Some(types::I64),
        CgType::Float => Some(types::F64),
        CgType::Bool => Some(types::I8),
        _ => None,
    }
}

fn collect_mir_names(mir: &TypedMirFunction, out: &mut BTreeSet<String>) {
    for block in mir.blocks.values() {
        match &block.terminator {
            TypedMirTerminator::Return(expr) => collect_expr_names(expr, out),
            TypedMirTerminator::Branch { cond, .. } => collect_expr_names(cond, out),
        }
    }
}

fn collect_expr_names(expr: &TypedMirExpr, out: &mut BTreeSet<String>) {
    match expr {
        TypedMirExpr::Local(name) | TypedMirExpr::Global(name) => {
            out.insert(name.clone());
        }
        TypedMirExpr::Binary { left, right, .. } => {
            collect_expr_names(left, out);
            collect_expr_names(right, out);
        }
        TypedMirExpr::Int(_)
        | TypedMirExpr::Float(_)
        | TypedMirExpr::Bool(_)
        | TypedMirExpr::Opaque(_) => {}
    }
}

fn render_rust_body_from_mir(
    mir: &TypedMirFunction,
    ty: &CgType,
    ctx: &TypedCtx,
) -> Option<String> {
    let entry = mir.blocks.get(&mir.entry)?;
    match &entry.terminator {
        TypedMirTerminator::Return(expr) => render_rust_expr(expr, ty, ctx),
        TypedMirTerminator::Branch {
            cond,
            then_bb,
            else_bb,
        } => {
            let then_expr = match mir.blocks.get(then_bb)?.terminator.clone() {
                TypedMirTerminator::Return(expr) => expr,
                TypedMirTerminator::Branch { .. } => return None,
            };
            let else_expr = match mir.blocks.get(else_bb)?.terminator.clone() {
                TypedMirTerminator::Return(expr) => expr,
                TypedMirTerminator::Branch { .. } => return None,
            };
            let cond_code = render_rust_expr(cond, &CgType::Bool, ctx)?;
            let then_code = render_rust_expr(&then_expr, ty, ctx)?;
            let else_code = render_rust_expr(&else_expr, ty, ctx)?;
            Some(format!(
                "if {cond_code} {{ {then_code} }} else {{ {else_code} }}"
            ))
        }
    }
}

fn render_rust_expr(expr: &TypedMirExpr, ty: &CgType, ctx: &TypedCtx) -> Option<String> {
    match expr {
        TypedMirExpr::Int(value) if matches!(ty, CgType::Int) => Some(format!("{value}_i64")),
        TypedMirExpr::Float(value) if matches!(ty, CgType::Float) => Some(format!("{value:?}_f64")),
        TypedMirExpr::Bool(value) if matches!(ty, CgType::Bool) => Some(value.to_string()),
        TypedMirExpr::Local(name) if ctx.lookup(name).is_some_and(|local_ty| local_ty == ty) => {
            Some(super::utils::rust_local_name(name))
        }
        TypedMirExpr::Global(name) if ctx.lookup(name).is_some_and(|global_ty| global_ty == ty) => {
            Some(format!(
                "{}_typed(rt)?",
                super::utils::rust_global_fn_name(name)
            ))
        }
        TypedMirExpr::Binary { op, left, right } if matches!(ty, CgType::Int | CgType::Float) => {
            let left_code = render_rust_expr(left, ty, ctx)?;
            let right_code = render_rust_expr(right, ty, ctx)?;
            Some(format!("({left_code} {op} {right_code})"))
        }
        TypedMirExpr::Binary { op, left, right } if matches!(ty, CgType::Bool) => {
            let left_code = render_rust_expr(left, &CgType::Bool, ctx)?;
            let right_code = render_rust_expr(right, &CgType::Bool, ctx)?;
            Some(format!("({left_code} {op} {right_code})"))
        }
        TypedMirExpr::Opaque(code) => Some(code.clone()),
        _ => None,
    }
}
