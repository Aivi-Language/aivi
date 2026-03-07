use crate::surface::{Expr, Literal, PathSegment};

mod coverage;
mod parsing;
mod sigils;

fn diag_codes(diags: &[crate::FileDiagnostic]) -> Vec<String> {
    let mut codes: Vec<String> = diags.iter().map(|d| d.diagnostic.code.clone()).collect();
    codes.sort();
    codes
}

fn expr_contains_ident(expr: &Expr, target: &str) -> bool {
    match expr {
        Expr::Ident(name) => name.name == target,
        Expr::Literal(_) => false,
        Expr::UnaryNeg { expr, .. } => expr_contains_ident(expr, target),
        Expr::Suffixed { base, .. } => expr_contains_ident(base, target),
        Expr::TextInterpolate { parts, .. } => parts.iter().any(|part| match part {
            crate::surface::TextPart::Text { .. } => false,
            crate::surface::TextPart::Expr { expr, .. } => expr_contains_ident(expr, target),
        }),
        Expr::List { items, .. } => items
            .iter()
            .any(|item| expr_contains_ident(&item.expr, target)),
        Expr::Tuple { items, .. } => items.iter().any(|item| expr_contains_ident(item, target)),
        Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => fields
            .iter()
            .any(|field| expr_contains_ident(&field.value, target)),
        Expr::FieldAccess { base, field, .. } => {
            field.name == target || expr_contains_ident(base, target)
        }
        Expr::Index { base, index, .. } => {
            expr_contains_ident(base, target) || expr_contains_ident(index, target)
        }
        Expr::FieldSection { field, .. } => field.name == target,
        Expr::Call { func, args, .. } => {
            expr_contains_ident(func, target)
                || args.iter().any(|arg| expr_contains_ident(arg, target))
        }
        Expr::Lambda { body, .. } => expr_contains_ident(body, target),
        Expr::Match {
            scrutinee, arms, ..
        } => {
            scrutinee
                .as_ref()
                .is_some_and(|e| expr_contains_ident(e, target))
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(|e| expr_contains_ident(e, target))
                        || expr_contains_ident(&arm.body, target)
                })
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            expr_contains_ident(cond, target)
                || expr_contains_ident(then_branch, target)
                || expr_contains_ident(else_branch, target)
        }
        Expr::Binary { left, right, .. } => {
            expr_contains_ident(left, target) || expr_contains_ident(right, target)
        }
        Expr::CapabilityScope { handlers, body, .. } => {
            handlers
                .iter()
                .any(|handler| expr_contains_ident(&handler.handler, target))
                || expr_contains_ident(body, target)
        }
        Expr::Block { items, .. } => items.iter().any(|item| match item {
            crate::surface::BlockItem::Bind { expr, .. }
            | crate::surface::BlockItem::Let { expr, .. }
            | crate::surface::BlockItem::Filter { expr, .. }
            | crate::surface::BlockItem::Yield { expr, .. }
            | crate::surface::BlockItem::Recurse { expr, .. }
            | crate::surface::BlockItem::Expr { expr, .. } => expr_contains_ident(expr, target),
            crate::surface::BlockItem::When { cond, effect, .. }
            | crate::surface::BlockItem::Unless { cond, effect, .. } => {
                expr_contains_ident(cond, target) || expr_contains_ident(effect, target)
            }
            crate::surface::BlockItem::Given {
                cond, fail_expr, ..
            } => expr_contains_ident(cond, target) || expr_contains_ident(fail_expr, target),
            crate::surface::BlockItem::On {
                transition,
                handler,
                ..
            } => expr_contains_ident(transition, target) || expr_contains_ident(handler, target),
        }),
        Expr::Raw { .. } => false,
        Expr::Mock {
            substitutions,
            body,
            ..
        } => {
            substitutions.iter().any(|sub| {
                sub.value
                    .as_ref()
                    .is_some_and(|v| expr_contains_ident(v, target))
            }) || expr_contains_ident(body, target)
        }
    }
}

fn expr_contains_record_field(expr: &Expr, field_name: &str) -> bool {
    match expr {
        Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => fields.iter().any(|f| {
            f.path.first().is_some_and(|seg| match seg {
                PathSegment::Field(name) => name.name == field_name,
                _ => false,
            }) || expr_contains_record_field(&f.value, field_name)
        }),
        Expr::Call { func, args, .. } => {
            expr_contains_record_field(func, field_name)
                || args
                    .iter()
                    .any(|a| expr_contains_record_field(a, field_name))
        }
        Expr::List { items, .. } => items
            .iter()
            .any(|item| expr_contains_record_field(&item.expr, field_name)),
        Expr::FieldAccess { base, .. } => expr_contains_record_field(base, field_name),
        Expr::Lambda { body, .. } => expr_contains_record_field(body, field_name),
        _ => false,
    }
}

fn expr_contains_string(expr: &Expr, target: &str) -> bool {
    match expr {
        Expr::Literal(Literal::String { text, .. }) => text == target,
        Expr::Ident(_) | Expr::Literal(_) | Expr::Raw { .. } => false,
        Expr::UnaryNeg { expr, .. } => expr_contains_string(expr, target),
        Expr::Suffixed { base, .. } => expr_contains_string(base, target),
        Expr::TextInterpolate { parts, .. } => parts.iter().any(|part| match part {
            crate::surface::TextPart::Text { text, .. } => text == target,
            crate::surface::TextPart::Expr { expr, .. } => expr_contains_string(expr, target),
        }),
        Expr::List { items, .. } => items
            .iter()
            .any(|item| expr_contains_string(&item.expr, target)),
        Expr::Tuple { items, .. } => items.iter().any(|item| expr_contains_string(item, target)),
        Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => fields
            .iter()
            .any(|field| expr_contains_string(&field.value, target)),
        Expr::FieldAccess { base, .. } => expr_contains_string(base, target),
        Expr::Index { base, index, .. } => {
            expr_contains_string(base, target) || expr_contains_string(index, target)
        }
        Expr::FieldSection { .. } => false,
        Expr::Call { func, args, .. } => {
            expr_contains_string(func, target)
                || args.iter().any(|arg| expr_contains_string(arg, target))
        }
        Expr::Lambda { body, .. } => expr_contains_string(body, target),
        Expr::Match {
            scrutinee, arms, ..
        } => {
            scrutinee
                .as_ref()
                .is_some_and(|e| expr_contains_string(e, target))
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(|e| expr_contains_string(e, target))
                        || expr_contains_string(&arm.body, target)
                })
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            expr_contains_string(cond, target)
                || expr_contains_string(then_branch, target)
                || expr_contains_string(else_branch, target)
        }
        Expr::Binary { left, right, .. } => {
            expr_contains_string(left, target) || expr_contains_string(right, target)
        }
        Expr::CapabilityScope { handlers, body, .. } => {
            handlers
                .iter()
                .any(|handler| expr_contains_string(&handler.handler, target))
                || expr_contains_string(body, target)
        }
        Expr::Block { items, .. } => items.iter().any(|item| match item {
            crate::surface::BlockItem::Bind { expr, .. }
            | crate::surface::BlockItem::Let { expr, .. }
            | crate::surface::BlockItem::Filter { expr, .. }
            | crate::surface::BlockItem::Yield { expr, .. }
            | crate::surface::BlockItem::Recurse { expr, .. }
            | crate::surface::BlockItem::Expr { expr, .. } => expr_contains_string(expr, target),
            crate::surface::BlockItem::When { cond, effect, .. }
            | crate::surface::BlockItem::Unless { cond, effect, .. } => {
                expr_contains_string(cond, target) || expr_contains_string(effect, target)
            }
            crate::surface::BlockItem::Given {
                cond, fail_expr, ..
            } => expr_contains_string(cond, target) || expr_contains_string(fail_expr, target),
            crate::surface::BlockItem::On {
                transition,
                handler,
                ..
            } => expr_contains_string(transition, target) || expr_contains_string(handler, target),
        }),
        Expr::Mock {
            substitutions,
            body,
            ..
        } => {
            substitutions.iter().any(|sub| {
                sub.value
                    .as_ref()
                    .is_some_and(|v| expr_contains_string(v, target))
            }) || expr_contains_string(body, target)
        }
    }
}
