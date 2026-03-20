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
                let is_bound = bound.iter().rev().any(|b| b == &name.name)
                    || env.get_all(&name.name).is_some();
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
            Expr::Flow { root, .. } => {
                collect_expr(root, env, bound, out);
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
                            // Pre-add compiler-generated bindings (e.g. __loop)
                            // to allow self-reference inside their definition.
                            if matches!(pattern, Pattern::Ident(n) if n.name.starts_with("__")) {
                                collect_pattern_binders(pattern, bound);
                            }
                            collect_expr(expr, env, bound, out);
                            collect_pattern_binders(pattern, bound);
                        }
                        BlockItem::Filter { expr, .. }
                        | BlockItem::Yield { expr, .. }
                        | BlockItem::Expr { expr, .. } => collect_expr(expr, env, bound, out),
                        BlockItem::Recurse { expr, .. } => match expr {
                            Expr::Ident(_) => {}
                            Expr::Call { func, args, .. }
                                if matches!(func.as_ref(), Expr::Ident(_)) && args.len() == 1 =>
                            {
                                collect_expr(&args[0], env, bound, out);
                            }
                            _ => collect_expr(expr, env, bound, out),
                        },
                        BlockItem::When { cond, effect, .. }
                        | BlockItem::Unless { cond, effect, .. } => {
                            collect_expr(cond, env, bound, out);
                            collect_expr(effect, env, bound, out);
                        }
                        BlockItem::Given { cond, fail_expr, .. } => {
                            collect_expr(cond, env, bound, out);
                            collect_expr(fail_expr, env, bound, out);
                        }
                    }
                }
                bound.truncate(before);
            }
            Expr::Mock { substitutions, body, .. } => {
                for sub in substitutions {
                    if let Some(value) = &sub.value {
                        collect_expr(value, env, bound, out);
                    }
                }
                collect_expr(body, env, bound, out);
            }
        }
    }

    let mut bound = Vec::new();
    let mut out = HashSet::new();
    collect_expr(expr, env, &mut bound, &mut out);
    out
}

pub(crate) fn collect_implicit_field_names(expr: &Expr, env: &TypeEnv) -> HashSet<String> {
    collect_unbound_names(expr, env)
}

fn collect_query_implicit_field_names(
    expr: &Expr,
    env: &TypeEnv,
    method_names: &HashMap<String, Vec<String>>,
) -> HashSet<String> {
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
        method_names: &HashMap<String, Vec<String>>,
        bound: &mut Vec<String>,
        out: &mut HashSet<String>,
    ) {
        match expr {
            Expr::Ident(name) => {
                if name.name == "_" {
                    return;
                }
                let is_bound_local = bound.iter().rev().any(|b| b == &name.name);
                let is_method_name = method_names.contains_key(&name.name);
                let is_value_bound = env.get_all(&name.name).is_some();
                if !is_bound_local && (!is_value_bound || is_method_name) {
                    out.insert(name.name.clone());
                }
            }
            Expr::UnaryNeg { expr, .. } => {
                collect_expr(expr, env, method_names, bound, out);
            }
            Expr::Suffixed { base, .. } => {
                collect_expr(base, env, method_names, bound, out);
            }
            Expr::Literal(_) | Expr::Raw { .. } | Expr::FieldSection { .. } => {}
            Expr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let TextPart::Expr { expr, .. } = part {
                        collect_expr(expr, env, method_names, bound, out);
                    }
                }
            }
            Expr::List { items, .. } => {
                for item in items {
                    collect_expr(&item.expr, env, method_names, bound, out);
                }
            }
            Expr::Tuple { items, .. } => {
                for item in items {
                    collect_expr(item, env, method_names, bound, out);
                }
            }
            Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => {
                for field in fields {
                    for seg in &field.path {
                        if let PathSegment::Index(expr, _) = seg {
                            collect_expr(expr, env, method_names, bound, out);
                        }
                    }
                    collect_expr(&field.value, env, method_names, bound, out);
                }
            }
            Expr::FieldAccess { base, .. } => collect_expr(base, env, method_names, bound, out),
            Expr::Index { base, index, .. } => {
                collect_expr(base, env, method_names, bound, out);
                collect_expr(index, env, method_names, bound, out);
            }
            Expr::Call { func, args, .. } => {
                collect_expr(func, env, method_names, bound, out);
                for arg in args {
                    collect_expr(arg, env, method_names, bound, out);
                }
            }
            Expr::Lambda { params, body, .. } => {
                let before = bound.len();
                for param in params {
                    collect_pattern_binders(param, bound);
                }
                collect_expr(body, env, method_names, bound, out);
                bound.truncate(before);
            }
            Expr::Match { scrutinee, arms, .. } => {
                if let Some(scrutinee) = scrutinee.as_deref() {
                    collect_expr(scrutinee, env, method_names, bound, out);
                }
                for arm in arms {
                    let before = bound.len();
                    collect_pattern_binders(&arm.pattern, bound);
                    if let Some(guard) = &arm.guard {
                        collect_expr(guard, env, method_names, bound, out);
                    }
                    collect_expr(&arm.body, env, method_names, bound, out);
                    bound.truncate(before);
                }
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                collect_expr(cond, env, method_names, bound, out);
                collect_expr(then_branch, env, method_names, bound, out);
                collect_expr(else_branch, env, method_names, bound, out);
            }
            Expr::Binary { left, right, .. } => {
                collect_expr(left, env, method_names, bound, out);
                collect_expr(right, env, method_names, bound, out);
            }
            Expr::Flow { root, .. } => {
                collect_expr(root, env, method_names, bound, out);
            }
            Expr::Block { items, .. } => {
                let before = bound.len();
                for item in items {
                    match item {
                        BlockItem::Bind { pattern, expr, .. }
                        | BlockItem::Let { pattern, expr, .. } => {
                            collect_expr(expr, env, method_names, bound, out);
                            collect_pattern_binders(pattern, bound);
                        }
                        BlockItem::Expr { expr, .. }
                        | BlockItem::Filter { expr, .. }
                        | BlockItem::Yield { expr, .. } => {
                            collect_expr(expr, env, method_names, bound, out);
                        }
                        BlockItem::Recurse { expr, .. } => match expr {
                            Expr::Ident(_) => {}
                            Expr::Call { func, args, .. }
                                if matches!(func.as_ref(), Expr::Ident(_)) && args.len() == 1 =>
                            {
                                collect_expr(&args[0], env, method_names, bound, out);
                            }
                            _ => collect_expr(expr, env, method_names, bound, out),
                        }
                        BlockItem::When { cond, effect, .. }
                        | BlockItem::Unless { cond, effect, .. } => {
                            collect_expr(cond, env, method_names, bound, out);
                            collect_expr(effect, env, method_names, bound, out);
                        }
                        BlockItem::Given { cond, fail_expr, .. } => {
                            collect_expr(cond, env, method_names, bound, out);
                            collect_expr(fail_expr, env, method_names, bound, out);
                        }
                    }
                }
                bound.truncate(before);
            }
            Expr::Mock {
                substitutions, body, ..
            } => {
                for sub in substitutions {
                    if let Some(value) = &sub.value {
                        collect_expr(value, env, method_names, bound, out);
                    }
                }
                collect_expr(body, env, method_names, bound, out);
            }
        }
    }

    let mut out = HashSet::new();
    let mut bound = Vec::new();
    collect_expr(expr, env, method_names, &mut bound, &mut out);
    out
}

fn collect_non_method_implicit_field_names(
    expr: &Expr,
    env: &TypeEnv,
    method_names: &HashMap<String, Vec<String>>,
) -> HashSet<String> {
    let mut unbound = collect_unbound_names(expr, env);
    unbound.retain(|name| !method_names.contains_key(name));
    unbound
}

pub(crate) fn lift_implicit_field_expr(
    expr: &Expr,
    env: &TypeEnv,
    method_names: &HashMap<String, Vec<String>>,
    implicit_param: &str,
) -> Option<Expr> {
    if matches!(expr, Expr::Lambda { .. }) || expr_contains_placeholder(expr) {
        return None;
    }

    let unbound = collect_query_implicit_field_names(expr, env, method_names);
    if unbound.is_empty() && !expr_contains_field_section(expr) {
        return None;
    }

    let body = rewrite_implicit_field_vars(expr.clone(), implicit_param, &unbound);
    let span = expr_span(expr);
    Some(Expr::Lambda {
        params: vec![Pattern::Ident(SpannedName {
            name: implicit_param.to_string(),
            span: span.clone(),
        })],
        body: Box::new(body),
        span,
    })
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
                name: implicit_param.into(),
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
        Expr::FieldSection { field, span } => Expr::FieldAccess {
            base: Box::new(Expr::Ident(SpannedName {
                name: implicit_param.into(),
                span: span.clone(),
            })),
            field,
            span,
        },
        Expr::Ident(_) | Expr::Literal(_) | Expr::Raw { .. } => expr,
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
        Expr::Flow { root, lines, span } => Expr::Flow {
            root: Box::new(rewrite_implicit_field_vars(*root, implicit_param, unbound)),
            lines,
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
                        | BlockItem::Expr { expr, .. } => {
                            *expr =
                                rewrite_implicit_field_vars(expr.clone(), implicit_param, unbound);
                        }
                        BlockItem::Recurse { expr, .. } => {
                            *expr = match expr.clone() {
                                Expr::Ident(_) => expr.clone(),
                                Expr::Call { func, args, span }
                                    if matches!(func.as_ref(), Expr::Ident(_)) && args.len() == 1 =>
                                {
                                    Expr::Call {
                                        func,
                                        args: vec![rewrite_implicit_field_vars(
                                            args[0].clone(),
                                            implicit_param,
                                            unbound,
                                        )],
                                        span,
                                    }
                                }
                                other => {
                                    rewrite_implicit_field_vars(other, implicit_param, unbound)
                                }
                            };
                        }
                        BlockItem::When { cond, effect, .. }
                        | BlockItem::Unless { cond, effect, .. } => {
                            *cond = rewrite_implicit_field_vars(cond.clone(), implicit_param, unbound);
                            *effect = rewrite_implicit_field_vars(effect.clone(), implicit_param, unbound);
                        }
                        BlockItem::Given { cond, fail_expr, .. } => {
                            *cond = rewrite_implicit_field_vars(cond.clone(), implicit_param, unbound);
                            *fail_expr = rewrite_implicit_field_vars(fail_expr.clone(), implicit_param, unbound);
                        }
                    }
                    item
                })
                .collect(),
            span,
        },
        Expr::Mock { substitutions, body, span } => {
            let substitutions = substitutions
                .into_iter()
                .map(|mut sub| {
                    sub.value = sub.value.map(|v| rewrite_implicit_field_vars(v, implicit_param, unbound));
                    sub
                })
                .collect();
            Expr::Mock {
                substitutions,
                body: Box::new(rewrite_implicit_field_vars(*body, implicit_param, unbound)),
                span,
            }
        }
    }
}

pub(crate) fn lift_predicate_expr(
    expr: &Expr,
    env: &TypeEnv,
    method_names: &HashMap<String, Vec<String>>,
    implicit_param: &str,
) -> Option<Expr> {
    if matches!(expr, Expr::Lambda { .. }) || expr_contains_placeholder(expr) {
        return None;
    }

    let unbound = collect_non_method_implicit_field_names(expr, env, method_names);
    if unbound.is_empty() && !expr_contains_field_section(expr) {
        return None;
    }

    let body = rewrite_implicit_field_vars(expr.clone(), implicit_param, &unbound);
    let span = expr_span(expr);
    Some(Expr::Lambda {
        params: vec![Pattern::Ident(SpannedName {
            name: implicit_param.to_string(),
            span: span.clone(),
        })],
        body: Box::new(body),
        span,
    })
}

fn expr_contains_placeholder(expr: &Expr) -> bool {
    match expr {
        Expr::Ident(name) => name.name == "_",
        Expr::UnaryNeg { expr, .. } => expr_contains_placeholder(expr),
        Expr::Suffixed { base, .. } => expr_contains_placeholder(base),
        Expr::Literal(_) | Expr::Raw { .. } => false,
        Expr::FieldSection { .. } => false,
        Expr::TextInterpolate { parts, .. } => parts.iter().any(|part| match part {
            TextPart::Text { .. } => false,
            TextPart::Expr { expr, .. } => expr_contains_placeholder(expr),
        }),
        Expr::List { items, .. } => items.iter().any(|item| expr_contains_placeholder(&item.expr)),
        Expr::Tuple { items, .. } => items.iter().any(expr_contains_placeholder),
        Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => fields.iter().any(|field| {
            field.path.iter().any(|seg| match seg {
                PathSegment::Field(_) | PathSegment::All(_) => false,
                PathSegment::Index(expr, _) => expr_contains_placeholder(expr),
            }) || expr_contains_placeholder(&field.value)
        }),
        Expr::FieldAccess { base, .. } => expr_contains_placeholder(base),
        Expr::Index { base, index, .. } => {
            expr_contains_placeholder(base) || expr_contains_placeholder(index)
        }
        Expr::Call { func, args, .. } => {
            expr_contains_placeholder(func) || args.iter().any(expr_contains_placeholder)
        }
        Expr::Lambda { body, .. } => expr_contains_placeholder(body),
        Expr::Match {
            scrutinee, arms, ..
        } => {
            scrutinee
                .as_ref()
                .is_some_and(|expr| expr_contains_placeholder(expr))
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(expr_contains_placeholder)
                        || expr_contains_placeholder(&arm.body)
                })
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            expr_contains_placeholder(cond)
                || expr_contains_placeholder(then_branch)
                || expr_contains_placeholder(else_branch)
        }
        Expr::Binary { left, right, .. } => {
            expr_contains_placeholder(left) || expr_contains_placeholder(right)
        }
        Expr::Flow { root, .. } => expr_contains_placeholder(root),
        Expr::Block { items, .. } => items.iter().any(|item| match item {
            BlockItem::Bind { expr, .. }
            | BlockItem::Let { expr, .. }
            | BlockItem::Filter { expr, .. }
            | BlockItem::Yield { expr, .. }
            | BlockItem::Expr { expr, .. } => expr_contains_placeholder(expr),
            BlockItem::Recurse { expr, .. } => match expr {
                Expr::Ident(_) => false,
                Expr::Call { func, args, .. }
                    if matches!(func.as_ref(), Expr::Ident(_)) && args.len() == 1 =>
                {
                    expr_contains_placeholder(&args[0])
                }
                _ => expr_contains_placeholder(expr),
            },
            BlockItem::When { cond, effect, .. } | BlockItem::Unless { cond, effect, .. } => {
                expr_contains_placeholder(cond) || expr_contains_placeholder(effect)
            }
            BlockItem::Given { cond, fail_expr, .. } => {
                expr_contains_placeholder(cond) || expr_contains_placeholder(fail_expr)
            }
        }),
        Expr::Mock { .. } => false,
    }
}

fn expr_contains_field_section(expr: &Expr) -> bool {
    match expr {
        Expr::FieldSection { .. } => true,
        Expr::UnaryNeg { expr, .. } => expr_contains_field_section(expr),
        Expr::Suffixed { base, .. } => expr_contains_field_section(base),
        Expr::Ident(_) | Expr::Literal(_) | Expr::Raw { .. } => false,
        Expr::TextInterpolate { parts, .. } => parts.iter().any(|part| match part {
            TextPart::Text { .. } => false,
            TextPart::Expr { expr, .. } => expr_contains_field_section(expr),
        }),
        Expr::List { items, .. } => items.iter().any(|item| expr_contains_field_section(&item.expr)),
        Expr::Tuple { items, .. } => items.iter().any(expr_contains_field_section),
        Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => fields.iter().any(|field| {
            field.path.iter().any(|seg| match seg {
                PathSegment::Field(_) | PathSegment::All(_) => false,
                PathSegment::Index(expr, _) => expr_contains_field_section(expr),
            }) || expr_contains_field_section(&field.value)
        }),
        Expr::FieldAccess { base, .. } => expr_contains_field_section(base),
        Expr::Index { base, index, .. } => {
            expr_contains_field_section(base) || expr_contains_field_section(index)
        }
        Expr::Call { func, args, .. } => {
            expr_contains_field_section(func) || args.iter().any(expr_contains_field_section)
        }
        Expr::Lambda { body, .. } => expr_contains_field_section(body),
        Expr::Match {
            scrutinee, arms, ..
        } => {
            scrutinee
                .as_ref()
                .is_some_and(|expr| expr_contains_field_section(expr))
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(expr_contains_field_section)
                        || expr_contains_field_section(&arm.body)
                })
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            expr_contains_field_section(cond)
                || expr_contains_field_section(then_branch)
                || expr_contains_field_section(else_branch)
        }
        Expr::Binary { left, right, .. } => {
            expr_contains_field_section(left) || expr_contains_field_section(right)
        }
        Expr::Flow { root, .. } => expr_contains_field_section(root),
        Expr::Block { items, .. } => items.iter().any(|item| match item {
            BlockItem::Bind { expr, .. }
            | BlockItem::Let { expr, .. }
            | BlockItem::Filter { expr, .. }
            | BlockItem::Yield { expr, .. }
            | BlockItem::Expr { expr, .. } => expr_contains_field_section(expr),
            BlockItem::Recurse { expr, .. } => match expr {
                Expr::Ident(_) => false,
                Expr::Call { func, args, .. }
                    if matches!(func.as_ref(), Expr::Ident(_)) && args.len() == 1 =>
                {
                    expr_contains_field_section(&args[0])
                }
                _ => expr_contains_field_section(expr),
            },
            BlockItem::When { cond, effect, .. } | BlockItem::Unless { cond, effect, .. } => {
                expr_contains_field_section(cond) || expr_contains_field_section(effect)
            }
            BlockItem::Given { cond, fail_expr, .. } => {
                expr_contains_field_section(cond) || expr_contains_field_section(fail_expr)
            }
        }),
        Expr::Mock { .. } => false,
    }
}
