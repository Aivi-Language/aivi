use crate::rust_ir::cg_type::CgType;
use crate::rust_ir::RustIrExpr;

use super::typed_expr::TypedCtx;
use super::typed_mir::{lower_typed_mir, TypedMirExpr, TypedMirTerminator};

pub(super) fn emit_typed_via_cranelift(
    expr: &RustIrExpr,
    ty: &CgType,
    ctx: &TypedCtx,
    indent: usize,
) -> Option<String> {
    let mir = lower_typed_mir(expr, ty, ctx)?;
    let entry = mir.blocks.get(&mir.entry)?;
    match &entry.terminator {
        TypedMirTerminator::Return(expr) => {
            let code = render_rust_expr(expr, ty, ctx)?;
            Some(format!("/* typed-clif */ {code}"))
        }
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
            let ind = "    ".repeat(indent);
            Some(format!(
                "/* typed-clif */ if {cond_code} {{ {then_code} }} else {{ {else_code} }}{ind}"
            ))
        }
    }
}

pub(super) fn cranelift_lowering_comment(
    expr: &RustIrExpr,
    ty: &CgType,
    ctx: &TypedCtx,
) -> Option<String> {
    let mir = lower_typed_mir(expr, ty, ctx)?;
    let mut lines = Vec::new();
    lines.push("clif.lowering.begin".to_string());
    for (bb, block) in &mir.blocks {
        lines.push(format!("  block{bb}:"));
        match &block.terminator {
            TypedMirTerminator::Return(expr) => {
                lines.push(format!("    return {}", render_expr(expr)));
            }
            TypedMirTerminator::Branch {
                cond,
                then_bb,
                else_bb,
            } => {
                lines.push(format!(
                    "    brif {} -> block{}, block{}",
                    render_expr(cond),
                    then_bb,
                    else_bb
                ));
            }
        }
    }
    lines.push("clif.lowering.end".to_string());
    Some(lines.join("\n"))
}

fn render_expr(expr: &TypedMirExpr) -> String {
    match expr {
        TypedMirExpr::Int(v) => format!("iconst.i64 {v}"),
        TypedMirExpr::Float(v) => format!("fconst.f64 {v:?}"),
        TypedMirExpr::Bool(v) => format!("bconst {v}"),
        TypedMirExpr::Local(name) => format!("v.local.{name}"),
        TypedMirExpr::Global(name) => format!("v.global.{name}"),
        TypedMirExpr::Binary { op, left, right } => {
            format!("{op}({}, {})", render_expr(left), render_expr(right))
        }
        TypedMirExpr::Opaque(code) => format!("opaque {{{code}}}"),
    }
}

fn render_rust_expr(expr: &TypedMirExpr, ty: &CgType, ctx: &TypedCtx) -> Option<String> {
    match expr {
        TypedMirExpr::Int(value) if matches!(ty, CgType::Int) => Some(format!("{value}_i64")),
        TypedMirExpr::Float(value) if matches!(ty, CgType::Float) => Some(format!("{value:?}_f64")),
        TypedMirExpr::Bool(value) if matches!(ty, CgType::Bool) => Some(format!("{value}")),
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
        TypedMirExpr::Opaque(code) => Some(code.clone()),
        _ => None,
    }
}
