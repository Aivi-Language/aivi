use crate::rust_ir::{RustIrDef, RustIrExpr, RustIrListItem, RustIrRecordField};

/// Reuse opportunities discovered by the Perceus-style ownership analysis.
///
/// v0 keeps this conservative: it only tracks definitions that are monomorphic
/// and closed, where backend lowering can later safely add in-place update paths.
#[derive(Debug, Default, Clone)]
pub(super) struct ReusePlan {
    pub(super) reusable_defs: Vec<String>,
    pub(super) patching_defs: Vec<String>,
}

pub(super) fn analyze_reuse(defs: &[RustIrDef]) -> ReusePlan {
    let mut reusable_defs = Vec::new();
    let mut patching_defs = Vec::new();
    for def in defs {
        let is_closed = def.cg_type.as_ref().is_some_and(|ty| ty.is_closed());
        if is_closed {
            reusable_defs.push(def.name.clone());
            if contains_patch(&def.expr) {
                patching_defs.push(def.name.clone());
            }
        }
    }
    ReusePlan {
        reusable_defs,
        patching_defs,
    }
}

fn contains_patch(expr: &RustIrExpr) -> bool {
    match expr {
        RustIrExpr::Patch { target, fields, .. } => {
            contains_patch(target) || fields.iter().any(contains_patch_field)
        }
        RustIrExpr::Lambda { body, .. } => contains_patch(body),
        RustIrExpr::App { func, arg, .. } => contains_patch(func) || contains_patch(arg),
        RustIrExpr::Call { func, args, .. } => {
            contains_patch(func) || args.iter().any(contains_patch)
        }
        RustIrExpr::List { items, .. } => items.iter().any(contains_patch_item),
        RustIrExpr::Tuple { items, .. } => items.iter().any(contains_patch),
        RustIrExpr::Record { fields, .. } => fields.iter().any(contains_patch_field),
        RustIrExpr::FieldAccess { base, .. } => contains_patch(base),
        RustIrExpr::Index { base, index, .. } => contains_patch(base) || contains_patch(index),
        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            contains_patch(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(contains_patch) || contains_patch(&arm.body)
                })
        }
        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => contains_patch(cond) || contains_patch(then_branch) || contains_patch(else_branch),
        RustIrExpr::Binary { left, right, .. } => contains_patch(left) || contains_patch(right),
        RustIrExpr::Block { items, .. } => items.iter().any(|item| match item {
            crate::rust_ir::RustIrBlockItem::Bind { expr, .. }
            | crate::rust_ir::RustIrBlockItem::Filter { expr }
            | crate::rust_ir::RustIrBlockItem::Yield { expr }
            | crate::rust_ir::RustIrBlockItem::Recurse { expr }
            | crate::rust_ir::RustIrBlockItem::Expr { expr, .. } => contains_patch(expr),
        }),
        RustIrExpr::TextInterpolate { parts, .. } => parts.iter().any(|part| match part {
            crate::rust_ir::RustIrTextPart::Expr { expr } => contains_patch(expr),
            crate::rust_ir::RustIrTextPart::Text { .. } => false,
        }),
        RustIrExpr::DebugFn { body, .. } => contains_patch(body),
        RustIrExpr::Pipe { func, arg, .. } => contains_patch(func) || contains_patch(arg),
        RustIrExpr::Raw { .. }
        | RustIrExpr::Local { .. }
        | RustIrExpr::Global { .. }
        | RustIrExpr::Builtin { .. }
        | RustIrExpr::ConstructorValue { .. }
        | RustIrExpr::LitNumber { .. }
        | RustIrExpr::LitString { .. }
        | RustIrExpr::LitSigil { .. }
        | RustIrExpr::LitBool { .. }
        | RustIrExpr::LitDateTime { .. } => false,
    }
}

fn contains_patch_field(field: &RustIrRecordField) -> bool {
    field.path.iter().any(|seg| match seg {
        crate::rust_ir::RustIrPathSegment::IndexValue(expr)
        | crate::rust_ir::RustIrPathSegment::IndexPredicate(expr) => contains_patch(expr),
        crate::rust_ir::RustIrPathSegment::Field(_)
        | crate::rust_ir::RustIrPathSegment::IndexFieldBool(_)
        | crate::rust_ir::RustIrPathSegment::IndexAll => false,
    }) || contains_patch(&field.value)
}

fn contains_patch_item(item: &RustIrListItem) -> bool {
    contains_patch(&item.expr)
}
