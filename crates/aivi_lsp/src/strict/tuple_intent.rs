use aivi::Module;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, TextEdit};

use super::{diag_with_fix, expr_span, StrictCategory, StrictFix};
use crate::backend::Backend;

pub(super) fn strict_tuple_intent(file_modules: &[Module], out: &mut Vec<Diagnostic>) {
    fn walk_expr(expr: &aivi::Expr, out: &mut Vec<Diagnostic>) {
        match expr {
            aivi::Expr::Tuple { items, .. } => {
                for item in items {
                    // Strict: `(a b, c)` often means `(a, b, c)`; surface parse sees `a b` as a call.
                    if let aivi::Expr::Call { func, args, span } = item {
                        if matches!(&**func, aivi::Expr::Ident(_))
                            && args.len() == 1
                            && matches!(&args[0], aivi::Expr::Ident(_))
                        {
                            let func_span = expr_span(func);
                            let insert_at = aivi::Span {
                                start: func_span.end.clone(),
                                end: func_span.end.clone(),
                            };
                            let edit = TextEdit {
                                range: Backend::span_to_range(insert_at.clone()),
                                new_text: ",".to_string(),
                            };
                            let message = format!(
                                    "AIVI-S020 [{}]\nSuspicious tuple element.\nFound: function application inside a tuple element.\nHint: If you meant a 3-tuple, use commas.\nFix: Insert ',' after the first name.",
                                    StrictCategory::Syntax.as_str(),
                                );
                            out.push(diag_with_fix(
                                "AIVI-S020",
                                StrictCategory::Syntax,
                                DiagnosticSeverity::WARNING,
                                message,
                                Backend::span_to_range(span.clone()),
                                Some(StrictFix {
                                    title: "Insert missing comma".to_string(),
                                    edits: vec![edit],
                                    is_preferred: false,
                                }),
                            ));
                        }
                    }
                    walk_expr(item, out);
                }
            }
            aivi::Expr::List { items, .. } => {
                for item in items {
                    walk_expr(&item.expr, out);
                }
            }
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => {
                for field in fields {
                    walk_expr(&field.value, out);
                }
            }
            aivi::Expr::Call { func, args, .. } => {
                walk_expr(func, out);
                for arg in args {
                    walk_expr(arg, out);
                }
            }
            aivi::Expr::Lambda { body, .. } => walk_expr(body, out),
            aivi::Expr::Match {
                scrutinee, arms, ..
            } => {
                if let Some(scrutinee) = scrutinee {
                    walk_expr(scrutinee, out);
                }
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
                        aivi::BlockItem::On {
                            transition,
                            handler,
                            ..
                        } => {
                            walk_expr(transition, out);
                            walk_expr(handler, out);
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
