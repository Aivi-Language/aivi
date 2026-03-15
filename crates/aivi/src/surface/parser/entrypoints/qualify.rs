fn qualify_expr(
    expr: Expr,
    import_map: &std::collections::HashMap<String, String>,
    scope: &std::collections::HashSet<String>,
) -> Expr {
    match expr {
        Expr::Ident(ref name) => {
            if !scope.contains(&name.name) {
                if let Some(qualified) = import_map.get(&name.name) {
                    return Expr::Ident(SpannedName {
                        name: qualified.clone(),
                        span: name.span.clone(),
                    });
                }
            }
            expr
        }
        Expr::Call { func, args, span } => Expr::Call {
            func: Box::new(qualify_expr(*func, import_map, scope)),
            args: args
                .into_iter()
                .map(|a| qualify_expr(a, import_map, scope))
                .collect(),
            span,
        },
        Expr::Lambda { params, body, span } => {
            let mut inner = scope.clone();
            for param in &params {
                collect_pattern_names(param, &mut inner);
            }
            Expr::Lambda {
                params,
                body: Box::new(qualify_expr(*body, import_map, &inner)),
                span,
            }
        }
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => Expr::Match {
            scrutinee: scrutinee.map(|e| Box::new(qualify_expr(*e, import_map, scope))),
            arms: arms
                .into_iter()
                .map(|arm| {
                    let mut inner = scope.clone();
                    collect_pattern_names(&arm.pattern, &mut inner);
                    MatchArm {
                        pattern: arm.pattern,
                        guard: arm.guard.map(|g| qualify_expr(g, import_map, &inner)),
                        guard_negated: arm.guard_negated,
                        body: qualify_expr(arm.body, import_map, &inner),
                        span: arm.span,
                    }
                })
                .collect(),
            span,
        },
        Expr::Block { kind, items, span } => Expr::Block {
            kind,
            items: qualify_block_items(items, import_map, scope),
            span,
        },
        Expr::If {
            cond,
            then_branch,
            else_branch,
            span,
        } => Expr::If {
            cond: Box::new(qualify_expr(*cond, import_map, scope)),
            then_branch: Box::new(qualify_expr(*then_branch, import_map, scope)),
            else_branch: Box::new(qualify_expr(*else_branch, import_map, scope)),
            span,
        },
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => Expr::Binary {
            op,
            left: Box::new(qualify_expr(*left, import_map, scope)),
            right: Box::new(qualify_expr(*right, import_map, scope)),
            span,
        },
        Expr::FieldAccess { base, field, span } => Expr::FieldAccess {
            base: Box::new(qualify_expr(*base, import_map, scope)),
            field,
            span,
        },
        Expr::Index { base, index, span } => Expr::Index {
            base: Box::new(qualify_expr(*base, import_map, scope)),
            index: Box::new(qualify_expr(*index, import_map, scope)),
            span,
        },
        Expr::List { items, span } => Expr::List {
            items: items
                .into_iter()
                .map(|item| ListItem {
                    expr: qualify_expr(item.expr, import_map, scope),
                    spread: item.spread,
                    span: item.span,
                })
                .collect(),
            span,
        },
        Expr::Tuple { items, span } => Expr::Tuple {
            items: items
                .into_iter()
                .map(|i| qualify_expr(i, import_map, scope))
                .collect(),
            span,
        },
        Expr::Record { fields, span } => Expr::Record {
            fields: fields
                .into_iter()
                .map(|f| RecordField {
                    path: f.path,
                    value: qualify_expr(f.value, import_map, scope),
                    spread: f.spread,
                    span: f.span,
                })
                .collect(),
            span,
        },
        Expr::PatchLit { fields, span } => Expr::PatchLit {
            fields: fields
                .into_iter()
                .map(|f| RecordField {
                    path: f.path,
                    value: qualify_expr(f.value, import_map, scope),
                    spread: f.spread,
                    span: f.span,
                })
                .collect(),
            span,
        },
        Expr::Suffixed { base, suffix, span } => {
            let template_name = format!("1{}", suffix.name);
            let suffix = if let Some(qualified) = import_map.get(&template_name) {
                // Strip the leading "1" from the qualified name's final segment to recover
                // the qualified suffix. E.g. "aivi.duration.1s" → suffix "aivi.duration.s"
                // is wrong; we need the full template name as-is. Instead, store the qualified
                // template name and decompose in HIR lowering.
                //
                // Simpler: just rename the suffix so HIR lowering produces the qualified Var.
                let suffix_portion = qualified.strip_prefix(&template_name[..1]).unwrap_or(qualified);
                // Actually: the template name format is "1{suffix}" where suffix is e.g. "s".
                // The qualified form is e.g. "aivi.duration.1s". HIR lowering produces
                // Var("1{suffix}") from Suffixed. So we want the suffix such that
                // format!("1{}", new_suffix) == qualified. That means new_suffix =
                // qualified[1..] if qualified starts with '1'. But qualified is
                // "aivi.duration.1s" which doesn't start with '1'.
                // The simplest approach: just keep the full qualified name in the suffix and
                // patch HIR lowering to check for dots. Actually, easier: rewrite Suffixed to
                // Call(Ident(qualified), base).
                let _ = suffix_portion;
                return Expr::Call {
                    func: Box::new(Expr::Ident(SpannedName {
                        name: qualified.clone(),
                        span: suffix.span.clone(),
                    })),
                    args: vec![qualify_expr(*base, import_map, scope)],
                    span,
                };
            } else {
                suffix
            };
            Expr::Suffixed {
                base: Box::new(qualify_expr(*base, import_map, scope)),
                suffix,
                span,
            }
        },
        Expr::UnaryNeg { expr, span } => Expr::UnaryNeg {
            expr: Box::new(qualify_expr(*expr, import_map, scope)),
            span,
        },
        Expr::TextInterpolate { parts, span } => Expr::TextInterpolate {
            parts: parts
                .into_iter()
                .map(|p| match p {
                    TextPart::Text { .. } => p,
                    TextPart::Expr { expr, span } => TextPart::Expr {
                        expr: Box::new(qualify_expr(*expr, import_map, scope)),
                        span,
                    },
                })
                .collect(),
            span,
        },
        Expr::Literal(Literal::Number { ref text, ref span }) => {
            // Handle suffixed number literals like "30s" — the suffix "s" maps to
            // template "1s". If an import qualifies "1s", rewrite to a Call.
            if let Some((number, suffix)) = split_number_suffix(text) {
                let template_name = format!("1{suffix}");
                if let Some(qualified) = import_map.get(&template_name) {
                    return Expr::Call {
                        func: Box::new(Expr::Ident(SpannedName {
                            name: qualified.clone(),
                            span: span.clone(),
                        })),
                        args: vec![Expr::Literal(Literal::Number {
                            text: number,
                            span: span.clone(),
                        })],
                        span: span.clone(),
                    };
                }
            }
            expr
        }
        Expr::Literal(_) | Expr::Raw { .. } | Expr::FieldSection { .. } => expr,
        Expr::Mock {
            substitutions,
            body,
            span,
        } => Expr::Mock {
            substitutions: substitutions
                .into_iter()
                .map(|sub| {
                    // Qualify the mock path segments against imports.
                    let first_name = sub.path.first().map(|s| s.name.as_str()).unwrap_or("");
                    let qualified_first =
                        import_map.get(first_name).cloned().unwrap_or_default();
                    let path = if !qualified_first.is_empty() {
                        // Replace the first segment with its qualified form split by '.'.
                        // `qualified_first` is only non-empty when path is non-empty.
                        let first_span = sub.path.first().map(|s| s.span.clone()).expect("non-empty path");
                        let mut parts: Vec<SpannedName> = qualified_first
                            .split('.')
                            .map(|seg| SpannedName {
                                name: seg.into(),
                                span: first_span.clone(),
                            })
                            .collect();
                        parts.extend(sub.path.into_iter().skip(1));
                        parts
                    } else {
                        sub.path
                    };
                    MockSubstitution {
                        path,
                        snapshot: sub.snapshot,
                        value: sub.value.map(|v| qualify_expr(v, import_map, scope)),
                        span: sub.span,
                    }
                })
                .collect(),
            body: Box::new(qualify_expr(*body, import_map, scope)),
            span,
        },
    }
}

/// Qualify block items, threading scope through let/bind patterns.
fn qualify_block_items(
    items: Vec<BlockItem>,
    import_map: &std::collections::HashMap<String, String>,
    scope: &std::collections::HashSet<String>,
) -> Vec<BlockItem> {
    let mut current_scope = scope.clone();
    items
        .into_iter()
        .map(|item| match item {
            BlockItem::Bind {
                pattern,
                expr,
                span,
            } => {
                let rewritten = qualify_expr(expr, import_map, &current_scope);
                collect_pattern_names(&pattern, &mut current_scope);
                BlockItem::Bind {
                    pattern,
                    expr: rewritten,
                    span,
                }
            }
            BlockItem::Let {
                pattern,
                expr,
                span,
            } => {
                let rewritten = qualify_expr(expr, import_map, &current_scope);
                collect_pattern_names(&pattern, &mut current_scope);
                BlockItem::Let {
                    pattern,
                    expr: rewritten,
                    span,
                }
            }
            BlockItem::Filter { expr, span } => BlockItem::Filter {
                expr: qualify_expr(expr, import_map, &current_scope),
                span,
            },
            BlockItem::Yield { expr, span } => BlockItem::Yield {
                expr: qualify_expr(expr, import_map, &current_scope),
                span,
            },
            BlockItem::Recurse { expr, span } => BlockItem::Recurse {
                expr: qualify_expr(expr, import_map, &current_scope),
                span,
            },
            BlockItem::Expr { expr, span } => BlockItem::Expr {
                expr: qualify_expr(expr, import_map, &current_scope),
                span,
            },
            BlockItem::When { cond, effect, span } => BlockItem::When {
                cond: qualify_expr(cond, import_map, &current_scope),
                effect: qualify_expr(effect, import_map, &current_scope),
                span,
            },
            BlockItem::Unless { cond, effect, span } => BlockItem::Unless {
                cond: qualify_expr(cond, import_map, &current_scope),
                effect: qualify_expr(effect, import_map, &current_scope),
                span,
            },
            BlockItem::Given {
                cond,
                fail_expr,
                span,
            } => BlockItem::Given {
                cond: qualify_expr(cond, import_map, &current_scope),
                fail_expr: qualify_expr(fail_expr, import_map, &current_scope),
                span,
            },
        })
        .collect()
}
