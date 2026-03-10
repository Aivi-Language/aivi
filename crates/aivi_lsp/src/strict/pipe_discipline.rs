use aivi::Module;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, TextEdit};

use super::{diag_with_fix, expr_span, push_simple, StrictCategory, StrictFix};
use crate::backend::Backend;

pub(super) fn strict_pipe_discipline(file_modules: &[Module], out: &mut Vec<Diagnostic>) {
    fn walk_expr(expr: &aivi::Expr, out: &mut Vec<Diagnostic>) {
        match expr {
            aivi::Expr::Binary {
                op,
                left,
                right,
                span,
            } if op == "|>" => {
                // Rule: RHS should be "callable-ish" to avoid `x |> 1`-style mistakes.
                let rhs_callable = matches!(
                    &**right,
                    aivi::Expr::Ident(_)
                        | aivi::Expr::FieldAccess { .. }
                        | aivi::Expr::Lambda { .. }
                        | aivi::Expr::Call { .. }
                        | aivi::Expr::Match { .. }
                        | aivi::Expr::Block { .. }
                );
                if !rhs_callable {
                    push_simple(
                        out,
                        "AIVI-S100",
                        StrictCategory::Pipe,
                        DiagnosticSeverity::ERROR,
                        format!(
                            "AIVI-S100 [{}]\nPipe step is not callable.\nFix: Replace the right-hand side with a function (e.g. `x => ...`) or a function name.",
                            StrictCategory::Pipe.as_str()
                        ),
                        span.clone(),
                    );
                }
                // Rule: `x |> f a b` should usually be `x |> f _ a b` (explicit placeholder).
                if let aivi::Expr::Call { func, args, .. } = &**right {
                    if args.len() >= 2 && matches!(&**func, aivi::Expr::Ident(_)) {
                        let func_span = expr_span(func);
                        let insert_at = aivi::Span {
                            start: func_span.end.clone(),
                            end: func_span.end.clone(),
                        };
                        let edit = TextEdit {
                            range: Backend::span_to_range(insert_at.clone()),
                            new_text: " _".to_string(),
                        };
                        out.push(diag_with_fix(
                            "AIVI-S101",
                            StrictCategory::Pipe,
                            DiagnosticSeverity::WARNING,
                            format!(
                                "AIVI-S101 [{}]\nAmbiguous pipe step with multi-argument call.\nFound: a pipe step `f a b`.\nHint: Pipelines apply the left value as the final argument.\nFix: Insert `_` to make the intended argument position explicit.",
                                StrictCategory::Pipe.as_str()
                            ),
                            Backend::span_to_range(span.clone()),
                            Some(StrictFix {
                                title: "Insert `_` placeholder".to_string(),
                                edits: vec![edit],
                                is_preferred: false,
                            }),
                        ));
                    }
                }
                walk_expr(left, out);
                walk_expr(right, out);
            }
            _ => walk_expr_children(expr, out),
        }
    }

    fn walk_expr_children(expr: &aivi::Expr, out: &mut Vec<Diagnostic>) {
        match expr {
            aivi::Expr::Tuple { items, .. } => items.iter().for_each(|e| walk_expr(e, out)),
            aivi::Expr::List { items, .. } => items.iter().for_each(|i| walk_expr(&i.expr, out)),
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => {
                fields.iter().for_each(|f| walk_expr(&f.value, out))
            }
            aivi::Expr::Call { func, args, .. } => {
                walk_expr(func, out);
                args.iter().for_each(|a| walk_expr(a, out));
            }
            aivi::Expr::Lambda { body, .. } => walk_expr(body, out),
            aivi::Expr::Match {
                scrutinee, arms, ..
            } => {
                scrutinee.iter().for_each(|e| walk_expr(e, out));
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        walk_expr(guard, out);
                    }
                    walk_expr(&arm.body, out);
                }
            }
            aivi::Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                walk_expr(cond, out);
                walk_expr(then_branch, out);
                walk_expr(else_branch, out);
            }
            aivi::Expr::Binary { left, right, .. } => {
                walk_expr(left, out);
                walk_expr(right, out);
            }
            aivi::Expr::Block { items, .. } => {
                for item in items {
                    match item {
                        aivi::BlockItem::Bind { expr, .. }
                        | aivi::BlockItem::Let { expr, .. }
                        | aivi::BlockItem::Filter { expr, .. }
                        | aivi::BlockItem::Yield { expr, .. }
                        | aivi::BlockItem::Recurse { expr, .. }
                        | aivi::BlockItem::Expr { expr, .. } => walk_expr(expr, out),
                        aivi::BlockItem::When { cond, effect, .. }
                        | aivi::BlockItem::Unless { cond, effect, .. } => {
                            walk_expr(cond, out);
                            walk_expr(effect, out);
                        }
                        aivi::BlockItem::Given {
                            cond, fail_expr, ..
                        } => {
                            walk_expr(cond, out);
                            walk_expr(fail_expr, out);
                        }
                    }
                }
            }
            aivi::Expr::UnaryNeg { expr, .. } => walk_expr(expr, out),
            aivi::Expr::FieldAccess { base, .. }
            | aivi::Expr::Index { base, .. }
            | aivi::Expr::Suffixed { base, .. } => walk_expr(base, out),
            aivi::Expr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let aivi::TextPart::Expr { expr, .. } = part {
                        walk_expr(expr, out);
                    }
                }
            }
            aivi::Expr::Ident(_)
            | aivi::Expr::Literal(_)
            | aivi::Expr::FieldSection { .. }
            | aivi::Expr::Raw { .. } => {}
            aivi::Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                for sub in substitutions {
                    if let Some(v) = &sub.value {
                        walk_expr(v, out);
                    }
                }
                walk_expr(body, out);
            }
        }
    }

    for module in file_modules {
        for item in &module.items {
            if let aivi::ModuleItem::Def(def) = item {
                walk_expr(&def.expr, out);
            }
            if let aivi::ModuleItem::DomainDecl(domain) = item {
                for d_item in &domain.items {
                    if let aivi::DomainItem::Def(def) | aivi::DomainItem::LiteralDef(def) = d_item {
                        walk_expr(&def.expr, out);
                    }
                }
            }
        }
    }
}

pub(super) fn strict_record_field_access(file_modules: &[Module], out: &mut Vec<Diagnostic>) {
    fn walk_expr(expr: &aivi::Expr, out: &mut Vec<Diagnostic>) {
        match expr {
            aivi::Expr::FieldAccess { base, field, span } => {
                if let aivi::Expr::Record { fields, .. } = &**base {
                    let mut has = false;
                    for f in fields {
                        if let Some(aivi::PathSegment::Field(name)) = f.path.last() {
                            if name.name == field.name {
                                has = true;
                                break;
                            }
                        }
                    }
                    if !has {
                        push_simple(
                            out,
                            "AIVI-S140",
                            StrictCategory::Type,
                            DiagnosticSeverity::ERROR,
                            format!(
                                "AIVI-S140 [{}]\nUnknown field on record literal.\nFound: `.{}'\nFix: Use an existing field name or add the field to the record literal.",
                                StrictCategory::Type.as_str(),
                                field.name
                            ),
                            span.clone(),
                        );
                    }
                }
                walk_expr(base, out);
            }
            _ => walk_expr_children(expr, out),
        }
    }

    fn walk_expr_children(expr: &aivi::Expr, out: &mut Vec<Diagnostic>) {
        match expr {
            aivi::Expr::Tuple { items, .. } => items.iter().for_each(|e| walk_expr(e, out)),
            aivi::Expr::List { items, .. } => items.iter().for_each(|i| walk_expr(&i.expr, out)),
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => {
                if matches!(expr, aivi::Expr::PatchLit { .. }) && fields.is_empty() {
                    let span = match expr {
                        aivi::Expr::PatchLit { span, .. } => span.clone(),
                        _ => unreachable!(),
                    };
                    push_simple(
                        out,
                        "AIVI-S141",
                        StrictCategory::Style,
                        DiagnosticSeverity::WARNING,
                        format!(
                            "AIVI-S141 [{}]\nEmpty patch literal.\nFix: Remove it, or add at least one patch entry.",
                            StrictCategory::Style.as_str()
                        ),
                        span,
                    );
                }
                fields.iter().for_each(|f| walk_expr(&f.value, out))
            }
            aivi::Expr::Call { func, args, .. } => {
                walk_expr(func, out);
                args.iter().for_each(|a| walk_expr(a, out));
            }
            aivi::Expr::Lambda { body, .. } => walk_expr(body, out),
            aivi::Expr::Match {
                scrutinee, arms, ..
            } => {
                scrutinee.iter().for_each(|e| walk_expr(e, out));
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        walk_expr(guard, out);
                    }
                    walk_expr(&arm.body, out);
                }
            }
            aivi::Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                walk_expr(cond, out);
                walk_expr(then_branch, out);
                walk_expr(else_branch, out);
            }
            aivi::Expr::Binary { left, right, .. } => {
                walk_expr(left, out);
                walk_expr(right, out);
            }
            aivi::Expr::Block { items, .. } => {
                for item in items {
                    match item {
                        aivi::BlockItem::Bind { expr, .. }
                        | aivi::BlockItem::Let { expr, .. }
                        | aivi::BlockItem::Filter { expr, .. }
                        | aivi::BlockItem::Yield { expr, .. }
                        | aivi::BlockItem::Recurse { expr, .. }
                        | aivi::BlockItem::Expr { expr, .. } => walk_expr(expr, out),
                        aivi::BlockItem::When { cond, effect, .. }
                        | aivi::BlockItem::Unless { cond, effect, .. } => {
                            walk_expr(cond, out);
                            walk_expr(effect, out);
                        }
                        aivi::BlockItem::Given {
                            cond, fail_expr, ..
                        } => {
                            walk_expr(cond, out);
                            walk_expr(fail_expr, out);
                        }
                    }
                }
            }
            aivi::Expr::UnaryNeg { expr, .. } => walk_expr(expr, out),
            aivi::Expr::FieldAccess { base, .. }
            | aivi::Expr::Index { base, .. }
            | aivi::Expr::Suffixed { base, .. } => walk_expr(base, out),
            aivi::Expr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let aivi::TextPart::Expr { expr, .. } = part {
                        walk_expr(expr, out);
                    }
                }
            }
            aivi::Expr::Ident(_)
            | aivi::Expr::Literal(_)
            | aivi::Expr::FieldSection { .. }
            | aivi::Expr::Raw { .. } => {}
            aivi::Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                for sub in substitutions {
                    if let Some(v) = &sub.value {
                        walk_expr(v, out);
                    }
                }
                walk_expr(body, out);
            }
        }
    }

    for module in file_modules {
        for item in &module.items {
            if let aivi::ModuleItem::Def(def) = item {
                walk_expr(&def.expr, out);
            }
            if let aivi::ModuleItem::DomainDecl(domain) = item {
                for d_item in &domain.items {
                    if let aivi::DomainItem::Def(def) | aivi::DomainItem::LiteralDef(def) = d_item {
                        walk_expr(&def.expr, out);
                    }
                }
            }
        }
    }
}
