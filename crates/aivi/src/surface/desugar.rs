use super::{BlockItem, BlockKind, Expr, SpannedName, TextPart};

fn is_unit_ident(expr: &Expr) -> bool {
    matches!(expr, Expr::Ident(name) if name.name == "Unit")
}

fn wrap_in_pure(expr: Expr) -> Expr {
    let span = match &expr {
        Expr::Ident(name) => name.span.clone(),
        Expr::Literal(literal) => match literal {
            super::Literal::Number { span, .. }
            | super::Literal::String { span, .. }
            | super::Literal::Sigil { span, .. }
            | super::Literal::Bool { span, .. }
            | super::Literal::DateTime { span, .. } => span.clone(),
        },
        Expr::UnaryNeg { span, .. }
        | Expr::TextInterpolate { span, .. }
        | Expr::List { span, .. }
        | Expr::Tuple { span, .. }
        | Expr::Record { span, .. }
        | Expr::PatchLit { span, .. }
        | Expr::Suffixed { span, .. }
        | Expr::FieldAccess { span, .. }
        | Expr::FieldSection { span, .. }
        | Expr::Index { span, .. }
        | Expr::Call { span, .. }
        | Expr::Lambda { span, .. }
        | Expr::Match { span, .. }
        | Expr::If { span, .. }
        | Expr::Binary { span, .. }
        | Expr::Block { span, .. }
        | Expr::Raw { span, .. } => span.clone(),
    };

    Expr::Call {
        func: Box::new(Expr::Ident(SpannedName {
            name: "pure".to_string(),
            span: span.clone(),
        })),
        args: vec![expr],
        span,
    }
}

fn desugar_effect_stmt_expr(expr: Expr) -> Expr {
    match expr {
        Expr::If {
            cond,
            then_branch,
            else_branch,
            span,
        } => {
            let cond = Box::new(desugar_expr(*cond));
            let then_branch_expr = desugar_expr(*then_branch);
            let else_branch_expr = desugar_expr(*else_branch);

            // Common ergonomic sugar in `effect { ... }` statements:
            // allow `if ... then <Effect> else Unit` by lifting `Unit` to `pure Unit`.
            let then_branch = if is_unit_ident(&then_branch_expr) {
                Box::new(wrap_in_pure(then_branch_expr))
            } else {
                Box::new(then_branch_expr)
            };
            let else_branch = if is_unit_ident(&else_branch_expr) {
                Box::new(wrap_in_pure(else_branch_expr))
            } else {
                Box::new(else_branch_expr)
            };

            Expr::If {
                cond,
                then_branch,
                else_branch,
                span,
            }
        }
        other => desugar_expr(other),
    }
}

fn desugar_expr(expr: Expr) -> Expr {
    match expr {
        Expr::Ident(_) | Expr::Literal(_) | Expr::Raw { .. } | Expr::FieldSection { .. } => expr,
        Expr::UnaryNeg { expr, span } => Expr::UnaryNeg {
            expr: Box::new(desugar_expr(*expr)),
            span,
        },
        Expr::Suffixed { base, suffix, span } => Expr::Suffixed {
            base: Box::new(desugar_expr(*base)),
            suffix,
            span,
        },
        Expr::TextInterpolate { parts, span } => Expr::TextInterpolate {
            parts: parts
                .into_iter()
                .map(|part| match part {
                    TextPart::Text { .. } => part,
                    TextPart::Expr { expr, span } => TextPart::Expr {
                        expr: Box::new(desugar_expr(*expr)),
                        span,
                    },
                })
                .collect(),
            span,
        },
        Expr::List { items, span } => Expr::List {
            items: items
                .into_iter()
                .map(|item| super::ListItem {
                    expr: desugar_expr(item.expr),
                    spread: item.spread,
                    span: item.span,
                })
                .collect(),
            span,
        },
        Expr::Tuple { items, span } => Expr::Tuple {
            items: items.into_iter().map(desugar_expr).collect(),
            span,
        },
        Expr::Record { fields, span } => Expr::Record {
            fields: fields
                .into_iter()
                .map(|field| super::RecordField {
                    spread: field.spread,
                    path: field
                        .path
                        .into_iter()
                        .map(|segment| match segment {
                            super::PathSegment::Field(name) => super::PathSegment::Field(name),
                            super::PathSegment::Index(expr, span) => {
                                super::PathSegment::Index(desugar_expr(expr), span)
                            }
                            super::PathSegment::All(span) => super::PathSegment::All(span),
                        })
                        .collect(),
                    value: desugar_expr(field.value),
                    span: field.span,
                })
                .collect(),
            span,
        },
        Expr::PatchLit { fields, span } => Expr::PatchLit {
            fields: fields
                .into_iter()
                .map(|field| super::RecordField {
                    spread: field.spread,
                    path: field
                        .path
                        .into_iter()
                        .map(|segment| match segment {
                            super::PathSegment::Field(name) => super::PathSegment::Field(name),
                            super::PathSegment::Index(expr, span) => {
                                super::PathSegment::Index(desugar_expr(expr), span)
                            }
                            super::PathSegment::All(span) => super::PathSegment::All(span),
                        })
                        .collect(),
                    value: desugar_expr(field.value),
                    span: field.span,
                })
                .collect(),
            span,
        },
        Expr::FieldAccess { base, field, span } => Expr::FieldAccess {
            base: Box::new(desugar_expr(*base)),
            field,
            span,
        },
        Expr::Index { base, index, span } => Expr::Index {
            base: Box::new(desugar_expr(*base)),
            index: Box::new(desugar_expr(*index)),
            span,
        },
        Expr::Call { func, args, span } => Expr::Call {
            func: Box::new(desugar_expr(*func)),
            args: args.into_iter().map(desugar_expr).collect(),
            span,
        },
        Expr::Lambda { params, body, span } => Expr::Lambda {
            params,
            body: Box::new(desugar_expr(*body)),
            span,
        },
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => Expr::Match {
            scrutinee: scrutinee.map(|expr| Box::new(desugar_expr(*expr))),
            arms: arms
                .into_iter()
                .map(|arm| super::MatchArm {
                    pattern: arm.pattern,
                    guard: arm.guard.map(desugar_expr),
                    body: desugar_expr(arm.body),
                    span: arm.span,
                })
                .collect(),
            span,
        },
        Expr::If {
            cond,
            then_branch,
            else_branch,
            span,
        } => Expr::If {
            cond: Box::new(desugar_expr(*cond)),
            then_branch: Box::new(desugar_expr(*then_branch)),
            else_branch: Box::new(desugar_expr(*else_branch)),
            span,
        },
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => Expr::Binary {
            op,
            left: Box::new(desugar_expr(*left)),
            right: Box::new(desugar_expr(*right)),
            span,
        },
        Expr::Block { kind, items, span } => {
            let items = items
                .into_iter()
                .map(|item| match item {
                    BlockItem::Bind {
                        pattern,
                        expr,
                        span,
                    } => BlockItem::Bind {
                        pattern,
                        expr: desugar_expr(expr),
                        span,
                    },
                    BlockItem::Let {
                        pattern,
                        expr,
                        span,
                    } => BlockItem::Let {
                        pattern,
                        expr: desugar_expr(expr),
                        span,
                    },
                    BlockItem::Filter { expr, span } => BlockItem::Filter {
                        expr: desugar_expr(expr),
                        span,
                    },
                    BlockItem::Yield { expr, span } => BlockItem::Yield {
                        expr: desugar_expr(expr),
                        span,
                    },
                    BlockItem::Recurse { expr, span } => BlockItem::Recurse {
                        expr: desugar_expr(expr),
                        span,
                    },
                    BlockItem::Expr { expr, span } => {
                        let expr = if matches!(kind, BlockKind::Effect) {
                            desugar_effect_stmt_expr(expr)
                        } else {
                            desugar_expr(expr)
                        };
                        BlockItem::Expr { expr, span }
                    }
                })
                .collect();
            Expr::Block { kind, items, span }
        }
    }
}

pub fn desugar_effect_sugars(expr: Expr) -> Expr {
    desugar_expr(expr)
}
