use std::collections::{BTreeMap, HashMap};

use crate::rust_ir::cg_type::CgType;
use crate::rust_ir::RustIrExpr;

use super::typed_expr::{emit_typed_expr, TypedCtx};
use super::utils::{rust_global_fn_name, rust_local_name};

#[derive(Clone, Debug)]
pub(super) struct TypedMirFunction {
    pub(super) entry: u32,
    pub(super) blocks: BTreeMap<u32, TypedMirBlock>,
}

#[derive(Clone, Debug)]
pub(super) struct TypedMirBlock {
    pub(super) terminator: TypedMirTerminator,
}

#[derive(Clone, Debug)]
pub(super) enum TypedMirTerminator {
    Return(TypedMirExpr),
    Branch {
        cond: TypedMirExpr,
        then_bb: u32,
        else_bb: u32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum TypedMirExpr {
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
    Opaque(String),
}

pub(super) fn emit_typed_via_mir(
    expr: &RustIrExpr,
    ty: &CgType,
    ctx: &TypedCtx,
    indent: usize,
) -> Option<String> {
    if !within_mir_depth(expr, 0) {
        return None;
    }
    let mir = lower_typed_mir(expr, ty, ctx)?;
    emit_mir_function(&mir, ty, ctx, indent)
}

pub(super) fn lower_typed_mir(
    expr: &RustIrExpr,
    ty: &CgType,
    ctx: &TypedCtx,
) -> Option<TypedMirFunction> {
    if !within_mir_depth(expr, 0) {
        return None;
    }
    let mut ctx = ctx.clone();
    let mut mir = lower_typed_expr(expr, ty, &mut ctx)?;
    optimize_typed_mir(&mut mir, ty, ctx);
    Some(mir)
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

fn lower_typed_expr(
    expr: &RustIrExpr,
    ty: &CgType,
    ctx: &mut TypedCtx,
) -> Option<TypedMirFunction> {
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

fn lower_scalar_expr(expr: &RustIrExpr, ty: &CgType, ctx: &mut TypedCtx) -> Option<LoweredRoot> {
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
        _ => {
            let mut typed_ctx = ctx.clone();
            emit_typed_expr(expr, ty, &mut typed_ctx, 1)
                .ok()
                .flatten()
                .map(TypedMirExpr::Opaque)
        }
    }?;
    Some(LoweredRoot::Expr(lowered))
}

fn lower_leaf_expr(expr: &RustIrExpr, ty: &CgType, ctx: &mut TypedCtx) -> Option<TypedMirExpr> {
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
        TypedMirTerminator::Return(expr) => emit_mir_expr_with_cse(expr, ty, ctx),
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
            let cond_code = emit_mir_expr_with_cse(cond, &CgType::Bool, ctx)?;
            let then_code = emit_mir_expr_with_cse(&then_expr, ty, ctx)?;
            let else_code = emit_mir_expr_with_cse(&else_expr, ty, ctx)?;
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
        TypedMirExpr::Opaque(code) => Some(format!("/* typed-mir */ ({code})")),
        _ => None,
    }
}

fn optimize_typed_mir(mir: &mut TypedMirFunction, ty: &CgType, ctx: TypedCtx) {
    for block in mir.blocks.values_mut() {
        match &mut block.terminator {
            TypedMirTerminator::Return(expr) => {
                *expr = fold_constants(expr.clone(), ty, &ctx);
            }
            TypedMirTerminator::Branch {
                cond,
                then_bb: _,
                else_bb: _,
            } => {
                *cond = fold_constants(cond.clone(), &CgType::Bool, &ctx);
            }
        }
    }
    simplify_branches(mir);
}

fn simplify_branches(mir: &mut TypedMirFunction) {
    let block_ids: Vec<u32> = mir.blocks.keys().copied().collect();
    for block_id in block_ids {
        let Some(block) = mir.blocks.get(&block_id).cloned() else {
            continue;
        };
        let TypedMirTerminator::Branch {
            cond,
            then_bb,
            else_bb,
        } = block.terminator
        else {
            continue;
        };

        if let TypedMirExpr::Bool(value) = cond {
            let selected = if value { then_bb } else { else_bb };
            if let Some(TypedMirTerminator::Return(expr)) =
                mir.blocks.get(&selected).map(|b| b.terminator.clone())
            {
                if let Some(entry) = mir.blocks.get_mut(&block_id) {
                    entry.terminator = TypedMirTerminator::Return(expr);
                }
            }
            continue;
        }

        let then_expr = mir
            .blocks
            .get(&then_bb)
            .and_then(|b| match b.terminator.clone() {
                TypedMirTerminator::Return(expr) => Some(expr),
                TypedMirTerminator::Branch { .. } => None,
            });
        let else_expr = mir
            .blocks
            .get(&else_bb)
            .and_then(|b| match b.terminator.clone() {
                TypedMirTerminator::Return(expr) => Some(expr),
                TypedMirTerminator::Branch { .. } => None,
            });
        if let (Some(a), Some(b)) = (then_expr, else_expr) {
            if a == b {
                if let Some(entry) = mir.blocks.get_mut(&block_id) {
                    entry.terminator = TypedMirTerminator::Return(a);
                }
            }
        }
    }
}

fn fold_constants(expr: TypedMirExpr, ty: &CgType, ctx: &TypedCtx) -> TypedMirExpr {
    let _ = ctx;
    match expr {
        TypedMirExpr::Binary { op, left, right } => {
            let left = fold_constants(*left, ty, ctx);
            let right = fold_constants(*right, ty, ctx);
            match (&left, &right, op.as_str(), ty) {
                (TypedMirExpr::Int(a), TypedMirExpr::Int(b), "+", CgType::Int) => {
                    TypedMirExpr::Int(a + b)
                }
                (TypedMirExpr::Int(a), TypedMirExpr::Int(b), "-", CgType::Int) => {
                    TypedMirExpr::Int(a - b)
                }
                (TypedMirExpr::Int(a), TypedMirExpr::Int(b), "*", CgType::Int) => {
                    TypedMirExpr::Int(a * b)
                }
                (TypedMirExpr::Int(a), TypedMirExpr::Int(b), "/", CgType::Int) if *b != 0 => {
                    TypedMirExpr::Int(a / b)
                }
                (TypedMirExpr::Int(a), TypedMirExpr::Int(b), "%", CgType::Int) if *b != 0 => {
                    TypedMirExpr::Int(a % b)
                }
                (TypedMirExpr::Float(a), TypedMirExpr::Float(b), "+", CgType::Float) => {
                    TypedMirExpr::Float(a + b)
                }
                (TypedMirExpr::Float(a), TypedMirExpr::Float(b), "-", CgType::Float) => {
                    TypedMirExpr::Float(a - b)
                }
                (TypedMirExpr::Float(a), TypedMirExpr::Float(b), "*", CgType::Float) => {
                    TypedMirExpr::Float(a * b)
                }
                (TypedMirExpr::Float(a), TypedMirExpr::Float(b), "/", CgType::Float)
                    if *b != 0.0 =>
                {
                    TypedMirExpr::Float(a / b)
                }
                _ => TypedMirExpr::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
            }
        }
        other => other,
    }
}

fn emit_mir_expr_with_cse(expr: &TypedMirExpr, ty: &CgType, ctx: &TypedCtx) -> Option<String> {
    let mut counts = HashMap::new();
    collect_expr_counts(expr, &mut counts);
    let mut cached = HashMap::new();
    let mut temps = Vec::new();
    let mut next_temp = 0usize;
    let body = emit_mir_expr_cse(
        expr,
        ty,
        ctx,
        &counts,
        &mut cached,
        &mut temps,
        &mut next_temp,
    )?;
    if temps.is_empty() {
        Some(body)
    } else {
        Some(format!("{{ {} {} }}", temps.join(" "), body))
    }
}

fn collect_expr_counts(expr: &TypedMirExpr, counts: &mut HashMap<String, usize>) {
    if let Some(key) = pure_expr_key(expr) {
        *counts.entry(key).or_insert(0) += 1;
    }
    if let TypedMirExpr::Binary { left, right, .. } = expr {
        collect_expr_counts(left, counts);
        collect_expr_counts(right, counts);
    }
}

fn pure_expr_key(expr: &TypedMirExpr) -> Option<String> {
    match expr {
        TypedMirExpr::Int(v) => Some(format!("i:{v}")),
        TypedMirExpr::Float(v) => Some(format!("f:{v:?}")),
        TypedMirExpr::Bool(v) => Some(format!("b:{v}")),
        TypedMirExpr::Local(name) => Some(format!("l:{name}")),
        TypedMirExpr::Binary { op, left, right } => {
            let left = pure_expr_key(left)?;
            let right = pure_expr_key(right)?;
            Some(format!("({op} {left} {right})"))
        }
        TypedMirExpr::Global(_) | TypedMirExpr::Opaque(_) => None,
    }
}

fn emit_mir_expr_cse(
    expr: &TypedMirExpr,
    ty: &CgType,
    ctx: &TypedCtx,
    counts: &HashMap<String, usize>,
    cached: &mut HashMap<String, String>,
    temps: &mut Vec<String>,
    next_temp: &mut usize,
) -> Option<String> {
    match expr {
        TypedMirExpr::Binary { op, left, right } if matches!(ty, CgType::Int | CgType::Float) => {
            let key = pure_expr_key(expr);
            if let Some(key) = key {
                if let Some(existing) = cached.get(&key) {
                    return Some(existing.clone());
                }
                let left_code = emit_mir_expr_cse(left, ty, ctx, counts, cached, temps, next_temp)?;
                let right_code =
                    emit_mir_expr_cse(right, ty, ctx, counts, cached, temps, next_temp)?;
                let expr_code = format!("/* typed-mir */ ({left_code} {op} {right_code})");
                if counts.get(&key).copied().unwrap_or(0) > 1 {
                    let name = format!("__mir_cse_{next_temp}");
                    *next_temp += 1;
                    temps.push(format!("let {name} = {expr_code};"));
                    cached.insert(key, name.clone());
                    return Some(name);
                }
                return Some(expr_code);
            }
            emit_mir_expr(expr, ty, ctx)
        }
        _ => emit_mir_expr(expr, ty, ctx),
    }
}
