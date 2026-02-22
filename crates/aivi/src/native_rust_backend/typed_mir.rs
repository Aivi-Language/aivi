use std::collections::BTreeMap;

use crate::rust_ir::cg_type::CgType;
use crate::rust_ir::RustIrExpr;

use super::typed_expr::TypedCtx;
use super::utils::{rust_global_fn_name, rust_local_name};

#[derive(Clone, Debug)]
pub(super) struct TypedMirFunction {
    entry: u32,
    blocks: BTreeMap<u32, TypedMirBlock>,
}

#[derive(Clone, Debug)]
struct TypedMirBlock {
    terminator: TypedMirTerminator,
}

#[derive(Clone, Debug)]
enum TypedMirTerminator {
    Return(TypedMirExpr),
    Branch {
        cond: TypedMirExpr,
        then_bb: u32,
        else_bb: u32,
    },
}

#[derive(Clone, Debug)]
enum TypedMirExpr {
    Int(i64),
    Float(f64),
    Bool(bool),
    Local(String),
    Global(String),
    Binary {
        op: String,
        left: Box<TypedMirExpr>,
        right: Box<TypedMirExpr>,
    },
}

pub(super) fn emit_typed_via_mir(
    expr: &RustIrExpr,
    ty: &CgType,
    ctx: &TypedCtx,
    indent: usize,
) -> Option<String> {
    if !matches!(ty, CgType::Int | CgType::Float | CgType::Bool) {
        return None;
    }
    if !within_mir_depth(expr, 0) {
        return None;
    }
    let mir = lower_typed_expr(expr, ty, ctx)?;
    emit_mir_function(&mir, ty, ctx, indent)
}

const MAX_MIR_DEPTH: usize = 64;

fn within_mir_depth(expr: &RustIrExpr, depth: usize) -> bool {
    if depth > MAX_MIR_DEPTH {
        return false;
    }
    match expr {
        RustIrExpr::Binary { left, right, .. } => {
            within_mir_depth(left, depth + 1) && within_mir_depth(right, depth + 1)
        }
        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            within_mir_depth(cond, depth + 1)
                && within_mir_depth(then_branch, depth + 1)
                && within_mir_depth(else_branch, depth + 1)
        }
        _ => true,
    }
}

fn lower_typed_expr(expr: &RustIrExpr, ty: &CgType, ctx: &TypedCtx) -> Option<TypedMirFunction> {
    let root = lower_scalar_expr(expr, ty, ctx)?;
    let mut blocks = BTreeMap::new();
    match root {
        LoweredRoot::Expr(expr) => {
            blocks.insert(
                0,
                TypedMirBlock {
                    terminator: TypedMirTerminator::Return(expr),
                },
            );
        }
        LoweredRoot::If {
            cond,
            then_expr,
            else_expr,
        } => {
            blocks.insert(
                0,
                TypedMirBlock {
                    terminator: TypedMirTerminator::Branch {
                        cond,
                        then_bb: 1,
                        else_bb: 2,
                    },
                },
            );
            blocks.insert(
                1,
                TypedMirBlock {
                    terminator: TypedMirTerminator::Return(then_expr),
                },
            );
            blocks.insert(
                2,
                TypedMirBlock {
                    terminator: TypedMirTerminator::Return(else_expr),
                },
            );
        }
    }
    Some(TypedMirFunction { entry: 0, blocks })
}

enum LoweredRoot {
    Expr(TypedMirExpr),
    If {
        cond: TypedMirExpr,
        then_expr: TypedMirExpr,
        else_expr: TypedMirExpr,
    },
}

fn lower_scalar_expr(expr: &RustIrExpr, ty: &CgType, ctx: &TypedCtx) -> Option<LoweredRoot> {
    let lowered = match expr {
        RustIrExpr::LitNumber { text, .. } => match ty {
            CgType::Int => text.parse::<i64>().ok().map(TypedMirExpr::Int),
            CgType::Float => text.parse::<f64>().ok().map(TypedMirExpr::Float),
            _ => None,
        },
        RustIrExpr::LitBool { value, .. } if matches!(ty, CgType::Bool) => {
            Some(TypedMirExpr::Bool(*value))
        }
        RustIrExpr::Local { name, .. }
            if ctx.lookup(name).is_some_and(|local_ty| local_ty == ty) =>
        {
            Some(TypedMirExpr::Local(name.clone()))
        }
        RustIrExpr::Global { name, .. }
            if ctx.lookup(name).is_some_and(|global_ty| global_ty == ty) =>
        {
            Some(TypedMirExpr::Global(name.clone()))
        }
        RustIrExpr::Binary {
            op, left, right, ..
        } if matches!(ty, CgType::Int | CgType::Float) => {
            let left = lower_leaf_expr(left, ty, ctx)?;
            let right = lower_leaf_expr(right, ty, ctx)?;
            Some(TypedMirExpr::Binary {
                op: op.clone(),
                left: Box::new(left),
                right: Box::new(right),
            })
        }
        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            let cond_expr = lower_leaf_expr(cond, &CgType::Bool, ctx)?;
            let then_expr = lower_leaf_expr(then_branch, ty, ctx)?;
            let else_expr = lower_leaf_expr(else_branch, ty, ctx)?;
            return Some(LoweredRoot::If {
                cond: cond_expr,
                then_expr,
                else_expr,
            });
        }
        _ => None,
    }?;
    Some(LoweredRoot::Expr(lowered))
}

fn lower_leaf_expr(expr: &RustIrExpr, ty: &CgType, ctx: &TypedCtx) -> Option<TypedMirExpr> {
    match lower_scalar_expr(expr, ty, ctx)? {
        LoweredRoot::Expr(expr) => Some(expr),
        LoweredRoot::If { .. } => None,
    }
}

fn emit_mir_function(
    mir: &TypedMirFunction,
    ty: &CgType,
    ctx: &TypedCtx,
    indent: usize,
) -> Option<String> {
    let entry = mir.blocks.get(&mir.entry)?;
    match &entry.terminator {
        TypedMirTerminator::Return(expr) => emit_mir_expr(expr, ty, ctx),
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
            let cond_code = emit_mir_expr(cond, &CgType::Bool, ctx)?;
            let then_code = emit_mir_expr(&then_expr, ty, ctx)?;
            let else_code = emit_mir_expr(&else_expr, ty, ctx)?;
            let ind = "    ".repeat(indent);
            Some(format!(
                "/* typed-mir */ if {cond_code} {{ {then_code} }} else {{ {else_code} }}{ind}"
            ))
        }
    }
}

fn emit_mir_expr(expr: &TypedMirExpr, ty: &CgType, ctx: &TypedCtx) -> Option<String> {
    match expr {
        TypedMirExpr::Int(value) if matches!(ty, CgType::Int) => Some(format!("{value}_i64")),
        TypedMirExpr::Float(value) if matches!(ty, CgType::Float) => Some(format!("{value:?}_f64")),
        TypedMirExpr::Bool(value) if matches!(ty, CgType::Bool) => Some(format!("{value}")),
        TypedMirExpr::Local(name) if ctx.lookup(name).is_some_and(|local_ty| local_ty == ty) => {
            Some(rust_local_name(name))
        }
        TypedMirExpr::Global(name) if ctx.lookup(name).is_some_and(|global_ty| global_ty == ty) => {
            Some(format!("{}_typed(rt)?", rust_global_fn_name(name)))
        }
        TypedMirExpr::Binary { op, left, right } if matches!(ty, CgType::Int | CgType::Float) => {
            let left_code = emit_mir_expr(left, ty, ctx)?;
            let right_code = emit_mir_expr(right, ty, ctx)?;
            Some(format!("/* typed-mir */ ({left_code} {op} {right_code})"))
        }
        _ => None,
    }
}
