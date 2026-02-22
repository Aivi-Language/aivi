use crate::rust_ir::cg_type::CgType;
use crate::rust_ir::RustIrExpr;

use super::typed_expr::TypedCtx;
use super::typed_mir::{lower_typed_mir, TypedMirExpr, TypedMirTerminator};

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
    }
}
