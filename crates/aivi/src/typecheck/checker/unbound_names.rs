fn collect_unbound_names(expr: &Expr, env: &TypeEnv) -> HashSet<String> {
    fn collect_pattern_binders(pattern: &Pattern, out: &mut Vec<String>) {
        match pattern {
            Pattern::Wildcard(_) => {}
            Pattern::Ident(name) => out.push(name.name.clone()),
            Pattern::SubjectIdent(name) => out.push(name.name.clone()),
            Pattern::Literal(_) => {}
            Pattern::At { name, pattern, .. } => {
                out.push(name.name.clone());
                collect_pattern_binders(pattern, out);
            }
            Pattern::Constructor { args, .. } => {
                for arg in args {
                    collect_pattern_binders(arg, out);
                }
            }
            Pattern::Tuple { items, .. } => {
                for item in items {
                    collect_pattern_binders(item, out);
                }
            }
            Pattern::List { items, rest, .. } => {
                for item in items {
                    collect_pattern_binders(item, out);
                }
                if let Some(rest) = rest.as_deref() {
                    collect_pattern_binders(rest, out);
                }
            }
            Pattern::Record { fields, .. } => {
                for field in fields {
                    collect_pattern_binders(&field.pattern, out);
                }
            }
        }
    }

    fn collect_expr(
        expr: &Expr,
        env: &TypeEnv,
        bound: &mut Vec<String>,
        out: &mut HashSet<String>,
    ) {
        match expr {
            Expr::Ident(name) => {
                if name.name == "_" {
                    return;
                }
                let reserved = matches!(name.name.as_str(), "key" | "value");
                let is_bound = bound.iter().rev().any(|b| b == &name.name)
                    || (!reserved && env.get(&name.name).is_some());
                if !is_bound {
                    out.insert(name.name.clone());
                }
            }
            Expr::Suffixed { base, .. } => {
                collect_expr(base, env, bound, out);
            }
            Expr::Literal(_) | Expr::Raw { .. } | Expr::FieldSection { .. } => {}
            Expr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let TextPart::Expr { expr, .. } = part {
                        collect_expr(expr, env, bound, out);
                    }
                }
            }
            Expr::List { items, .. } => {
                for item in items {
                    collect_expr(&item.expr, env, bound, out);
                }
            }
            Expr::Tuple { items, .. } => {
                for item in items {
                    collect_expr(item, env, bound, out);
                }
            }
            Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => {
                for field in fields {
                    for seg in &field.path {
                        if let PathSegment::Index(expr, _) = seg {
                            collect_expr(expr, env, bound, out);
                        }
                    }
                    collect_expr(&field.value, env, bound, out);
                }
            }
            Expr::FieldAccess { base, .. } => collect_expr(base, env, bound, out),
            Expr::Index { base, index, .. } => {
                collect_expr(base, env, bound, out);
                collect_expr(index, env, bound, out);
            }
            Expr::Call { func, args, .. } => {
                collect_expr(func, env, bound, out);
                for arg in args {
                    collect_expr(arg, env, bound, out);
                }
            }
            Expr::Lambda { params, body, .. } => {
                let before = bound.len();
                for param in params {
                    collect_pattern_binders(param, bound);
                }
                collect_expr(body, env, bound, out);
                bound.truncate(before);
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                if let Some(scrutinee) = scrutinee.as_deref() {
                    collect_expr(scrutinee, env, bound, out);
                }
                for arm in arms {
                    let before = bound.len();
                    collect_pattern_binders(&arm.pattern, bound);
                    if let Some(guard) = arm.guard.as_ref() {
                        collect_expr(guard, env, bound, out);
                    }
                    collect_expr(&arm.body, env, bound, out);
                    bound.truncate(before);
                }
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                collect_expr(cond, env, bound, out);
                collect_expr(then_branch, env, bound, out);
                collect_expr(else_branch, env, bound, out);
            }
            Expr::Binary { left, right, .. } => {
                collect_expr(left, env, bound, out);
                collect_expr(right, env, bound, out);
            }
            Expr::UnaryNeg { expr, .. } => {
                collect_expr(expr, env, bound, out);
            }
            Expr::Block { items, .. } => {
                let before = bound.len();
                for item in items {
                    match item {
                        BlockItem::Bind { pattern, expr, .. }
                        | BlockItem::Let { pattern, expr, .. } => {
                            collect_expr(expr, env, bound, out);
                            collect_pattern_binders(pattern, bound);
                        }
                        BlockItem::Filter { expr, .. }
                        | BlockItem::Yield { expr, .. }
                        | BlockItem::Recurse { expr, .. }
                        | BlockItem::Expr { expr, .. } => collect_expr(expr, env, bound, out),
                    }
                }
                bound.truncate(before);
            }
        }
    }

    let mut bound = Vec::new();
    let mut out = HashSet::new();
    collect_expr(expr, env, &mut bound, &mut out);
    out
}

fn rewrite_implicit_field_vars(
    expr: Expr,
    implicit_param: &str,
    unbound: &HashSet<String>,
) -> Expr {
    match expr {
        Expr::UnaryNeg { expr, span } => Expr::UnaryNeg {
            expr: Box::new(rewrite_implicit_field_vars(*expr, implicit_param, unbound)),
            span,
        },
        Expr::Ident(name) if unbound.contains(&name.name) => {
            let param = SpannedName {
                name: implicit_param.to_string(),
                span: name.span.clone(),
            };
            let field = SpannedName {
                name: name.name,
                span: name.span.clone(),
            };
            Expr::FieldAccess {
                base: Box::new(Expr::Ident(param)),
                field,
                span: name.span,
            }
        }
        Expr::Ident(_) | Expr::Literal(_) | Expr::Raw { .. } | Expr::FieldSection { .. } => expr,
        Expr::Suffixed { base, suffix, span } => Expr::Suffixed {
            base: Box::new(rewrite_implicit_field_vars(*base, implicit_param, unbound)),
            suffix,
            span,
        },
        Expr::TextInterpolate { parts, span } => Expr::TextInterpolate {
            parts: parts
                .into_iter()
                .map(|part| match part {
                    TextPart::Text { .. } => part,
                    TextPart::Expr { expr, span } => TextPart::Expr {
                        expr: Box::new(rewrite_implicit_field_vars(*expr, implicit_param, unbound)),
                        span,
                    },
                })
                .collect(),
            span,
        },
        Expr::List { items, span } => Expr::List {
            items: items
                .into_iter()
                .map(|item| ListItem {
                    expr: rewrite_implicit_field_vars(item.expr, implicit_param, unbound),
                    spread: item.spread,
                    span: item.span,
                })
                .collect(),
            span,
        },
        Expr::Tuple { items, span } => Expr::Tuple {
            items: items
                .into_iter()
                .map(|item| rewrite_implicit_field_vars(item, implicit_param, unbound))
                .collect(),
            span,
        },
        Expr::Record { fields, span } => Expr::Record {
            fields: fields
                .into_iter()
                .map(|field| RecordField {
                    spread: field.spread,
                    path: field
                        .path
                        .into_iter()
                        .map(|seg| match seg {
                            PathSegment::Field(name) => PathSegment::Field(name),
                            PathSegment::Index(expr, seg_span) => PathSegment::Index(
                                rewrite_implicit_field_vars(expr, implicit_param, unbound),
                                seg_span,
                            ),
                            PathSegment::All(seg_span) => PathSegment::All(seg_span),
                        })
                        .collect(),
                    value: rewrite_implicit_field_vars(field.value, implicit_param, unbound),
                    span: field.span,
                })
                .collect(),
            span,
        },
        Expr::PatchLit { fields, span } => Expr::PatchLit {
            fields: fields
                .into_iter()
                .map(|field| RecordField {
                    spread: field.spread,
                    path: field
                        .path
                        .into_iter()
                        .map(|seg| match seg {
                            PathSegment::Field(name) => PathSegment::Field(name),
                            PathSegment::Index(expr, seg_span) => PathSegment::Index(
                                rewrite_implicit_field_vars(expr, implicit_param, unbound),
                                seg_span,
                            ),
                            PathSegment::All(seg_span) => PathSegment::All(seg_span),
                        })
                        .collect(),
                    value: rewrite_implicit_field_vars(field.value, implicit_param, unbound),
                    span: field.span,
                })
                .collect(),
            span,
        },
        Expr::FieldAccess { base, field, span } => Expr::FieldAccess {
            base: Box::new(rewrite_implicit_field_vars(*base, implicit_param, unbound)),
            field,
            span,
        },
        Expr::Index { base, index, span } => Expr::Index {
            base: Box::new(rewrite_implicit_field_vars(*base, implicit_param, unbound)),
            index: Box::new(rewrite_implicit_field_vars(*index, implicit_param, unbound)),
            span,
        },
        Expr::Call { func, args, span } => Expr::Call {
            func: Box::new(rewrite_implicit_field_vars(*func, implicit_param, unbound)),
            args: args
                .into_iter()
                .map(|arg| rewrite_implicit_field_vars(arg, implicit_param, unbound))
                .collect(),
            span,
        },
        Expr::Lambda { params, body, span } => Expr::Lambda {
            params,
            body: Box::new(rewrite_implicit_field_vars(*body, implicit_param, unbound)),
            span,
        },
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => Expr::Match {
            scrutinee: scrutinee
                .map(|e| Box::new(rewrite_implicit_field_vars(*e, implicit_param, unbound))),
            arms: arms
                .into_iter()
                .map(|mut arm| {
                    arm.guard = arm
                        .guard
                        .map(|g| rewrite_implicit_field_vars(g, implicit_param, unbound));
                    arm.body = rewrite_implicit_field_vars(arm.body, implicit_param, unbound);
                    arm
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
            cond: Box::new(rewrite_implicit_field_vars(*cond, implicit_param, unbound)),
            then_branch: Box::new(rewrite_implicit_field_vars(
                *then_branch,
                implicit_param,
                unbound,
            )),
            else_branch: Box::new(rewrite_implicit_field_vars(
                *else_branch,
                implicit_param,
                unbound,
            )),
            span,
        },
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => Expr::Binary {
            op,
            left: Box::new(rewrite_implicit_field_vars(*left, implicit_param, unbound)),
            right: Box::new(rewrite_implicit_field_vars(*right, implicit_param, unbound)),
            span,
        },
        Expr::Block { kind, items, span } => Expr::Block {
            kind,
            items: items
                .into_iter()
                .map(|mut item| {
                    match &mut item {
                        BlockItem::Bind { expr, .. }
                        | BlockItem::Let { expr, .. }
                        | BlockItem::Filter { expr, .. }
                        | BlockItem::Yield { expr, .. }
                        | BlockItem::Recurse { expr, .. }
                        | BlockItem::Expr { expr, .. } => {
                            *expr =
                                rewrite_implicit_field_vars(expr.clone(), implicit_param, unbound);
                        }
                    }
                    item
                })
                .collect(),
            span,
        },
    }
}
