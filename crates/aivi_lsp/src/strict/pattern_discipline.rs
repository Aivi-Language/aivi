use aivi::{BlockKind, Module};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

use super::{push_simple, StrictCategory};

pub(super) fn strict_pattern_discipline(file_modules: &[Module], out: &mut Vec<Diagnostic>) {
    fn pattern_binds_name(pat: &aivi::Pattern, name: &str) -> bool {
        match pat {
            aivi::Pattern::Ident(n) | aivi::Pattern::SubjectIdent(n) => n.name == name,
            aivi::Pattern::At {
                name: n, pattern, ..
            } => n.name == name || pattern_binds_name(pattern, name),
            aivi::Pattern::Tuple { items, .. } => items.iter().any(|p| pattern_binds_name(p, name)),
            aivi::Pattern::List { items, rest, .. } => {
                items.iter().any(|p| pattern_binds_name(p, name))
                    || rest.as_deref().is_some_and(|p| pattern_binds_name(p, name))
            }
            aivi::Pattern::Record { fields, .. } => {
                fields.iter().any(|f| pattern_binds_name(&f.pattern, name))
            }
            aivi::Pattern::Constructor { args, .. } => {
                args.iter().any(|p| pattern_binds_name(p, name))
            }
            aivi::Pattern::Wildcard(_) | aivi::Pattern::Literal(_) => false,
        }
    }

    fn collect_pattern_binders(pat: &aivi::Pattern, out: &mut Vec<aivi::SpannedName>) {
        match pat {
            aivi::Pattern::Ident(n) | aivi::Pattern::SubjectIdent(n) => out.push(n.clone()),
            aivi::Pattern::At { name, pattern, .. } => {
                out.push(name.clone());
                collect_pattern_binders(pattern, out);
            }
            aivi::Pattern::Tuple { items, .. } => {
                items.iter().for_each(|p| collect_pattern_binders(p, out))
            }
            aivi::Pattern::List { items, rest, .. } => {
                items.iter().for_each(|p| collect_pattern_binders(p, out));
                if let Some(rest) = rest.as_deref() {
                    collect_pattern_binders(rest, out);
                }
            }
            aivi::Pattern::Record { fields, .. } => fields
                .iter()
                .for_each(|f| collect_pattern_binders(&f.pattern, out)),
            aivi::Pattern::Constructor { args, .. } => {
                args.iter().for_each(|p| collect_pattern_binders(p, out))
            }
            aivi::Pattern::Wildcard(_) | aivi::Pattern::Literal(_) => {}
        }
    }

    fn expr_uses_name_free(expr: &aivi::Expr, name: &str) -> bool {
        match expr {
            aivi::Expr::Ident(n) => n.name == name,
            aivi::Expr::Tuple { items, .. } => items.iter().any(|e| expr_uses_name_free(e, name)),
            aivi::Expr::List { items, .. } => items
                .iter()
                .any(|item| expr_uses_name_free(&item.expr, name)),
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => {
                fields.iter().any(|f| expr_uses_name_free(&f.value, name))
            }
            aivi::Expr::Call { func, args, .. } => {
                expr_uses_name_free(func, name) || args.iter().any(|a| expr_uses_name_free(a, name))
            }
            aivi::Expr::Lambda { params, body, .. } => {
                if params.iter().any(|p| pattern_binds_name(p, name)) {
                    false
                } else {
                    expr_uses_name_free(body, name)
                }
            }
            aivi::Expr::Match {
                scrutinee, arms, ..
            } => {
                let scrutinee_uses = scrutinee
                    .as_ref()
                    .is_some_and(|e| expr_uses_name_free(e, name));
                if scrutinee_uses {
                    return true;
                }
                arms.iter().any(|arm| {
                    if pattern_binds_name(&arm.pattern, name) {
                        false
                    } else {
                        arm.guard
                            .as_ref()
                            .is_some_and(|g| expr_uses_name_free(g, name))
                            || expr_uses_name_free(&arm.body, name)
                    }
                })
            }
            aivi::Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                expr_uses_name_free(cond, name)
                    || expr_uses_name_free(then_branch, name)
                    || expr_uses_name_free(else_branch, name)
            }
            aivi::Expr::Binary { left, right, .. } => {
                expr_uses_name_free(left, name) || expr_uses_name_free(right, name)
            }
            aivi::Expr::Block { items, .. } => {
                let mut shadowed = false;
                for item in items {
                    match item {
                        aivi::BlockItem::Bind { pattern, expr, .. }
                        | aivi::BlockItem::Let { pattern, expr, .. } => {
                            if !shadowed && expr_uses_name_free(expr, name) {
                                return true;
                            }
                            if pattern_binds_name(pattern, name) {
                                shadowed = true;
                            }
                        }
                        aivi::BlockItem::Filter { expr, .. }
                        | aivi::BlockItem::Yield { expr, .. }
                        | aivi::BlockItem::Recurse { expr, .. }
                        | aivi::BlockItem::Expr { expr, .. } => {
                            if !shadowed && expr_uses_name_free(expr, name) {
                                return true;
                            }
                        }
                        aivi::BlockItem::When { cond, effect, .. }
                        | aivi::BlockItem::Unless { cond, effect, .. } => {
                            if !shadowed
                                && (expr_uses_name_free(cond, name)
                                    || expr_uses_name_free(effect, name))
                            {
                                return true;
                            }
                        }
                        aivi::BlockItem::Given {
                            cond, fail_expr, ..
                        } => {
                            if !shadowed
                                && (expr_uses_name_free(cond, name)
                                    || expr_uses_name_free(fail_expr, name))
                            {
                                return true;
                            }
                        }
                    }
                }
                false
            }
            aivi::Expr::FieldAccess { base, .. }
            | aivi::Expr::Index { base, .. }
            | aivi::Expr::Suffixed { base, .. } => expr_uses_name_free(base, name),
            aivi::Expr::UnaryNeg { expr, .. } => expr_uses_name_free(expr, name),
            aivi::Expr::TextInterpolate { parts, .. } => parts.iter().any(|p| match p {
                aivi::TextPart::Text { .. } => false,
                aivi::TextPart::Expr { expr, .. } => expr_uses_name_free(expr, name),
            }),
            aivi::Expr::Literal(_) | aivi::Expr::FieldSection { .. } | aivi::Expr::Raw { .. } => {
                false
            }
            aivi::Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                substitutions.iter().any(|sub| {
                    sub.value
                        .as_ref()
                        .is_some_and(|v| expr_uses_name_free(v, name))
                }) || expr_uses_name_free(body, name)
            }
            aivi::Expr::Flow { root, .. } => expr_uses_name_free(root, name),
        }
    }

    fn check_arms(arms: &[aivi::MatchArm], out: &mut Vec<Diagnostic>) {
        let mut saw_wildcard = false;
        for arm in arms {
            let mut binders = Vec::new();
            collect_pattern_binders(&arm.pattern, &mut binders);
            for binder in binders {
                if binder.name.starts_with('_') {
                    continue;
                }
                let used = arm
                    .guard
                    .as_ref()
                    .is_some_and(|g| expr_uses_name_free(g, &binder.name))
                    || expr_uses_name_free(&arm.body, &binder.name);
                if !used {
                    push_simple(
                        out,
                        "AIVI-S301",
                        StrictCategory::Pattern,
                        DiagnosticSeverity::WARNING,
                        format!(
                            "AIVI-S301 [{}]\nUnused pattern binding.\nFound: `{}`.\nFix: Use the value, or rename to `_`/`_name` to mark it intentionally unused. If you only want to assert the field exists, prefer matching with `_` (e.g. `age: _`).",
                            StrictCategory::Pattern.as_str(),
                            binder.name
                        ),
                        binder.span.clone(),
                    );
                }
            }
            if saw_wildcard {
                push_simple(
                    out,
                    "AIVI-S300",
                    StrictCategory::Pattern,
                    DiagnosticSeverity::WARNING,
                    format!(
                        "AIVI-S300 [{}]\nUnreachable match arm.\nReason: a previous arm is a wildcard `_`.\nFix: Move `_` arm to the end, or remove unreachable arms.",
                        StrictCategory::Pattern.as_str()
                    ),
                    arm.span.clone(),
                );
            }
            if matches!(arm.pattern, aivi::Pattern::Wildcard(_)) {
                saw_wildcard = true;
            }
        }
    }

    fn walk_expr(expr: &aivi::Expr, out: &mut Vec<Diagnostic>) {
        match expr {
            aivi::Expr::Match { arms, .. } => {
                check_arms(arms, out);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        walk_expr(guard, out);
                    }
                    walk_expr(&arm.body, out);
                }
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
            aivi::Expr::Flow { root, .. } => walk_expr(root, out),
            aivi::Expr::Match { .. } => unreachable!(),
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

pub(super) fn strict_block_shape(file_modules: &[Module], out: &mut Vec<Diagnostic>) {
    fn names_in_pattern(pat: &aivi::Pattern, out: &mut Vec<String>) {
        match pat {
            aivi::Pattern::Ident(name) => out.push(name.name.clone()),
            aivi::Pattern::SubjectIdent(name) => out.push(name.name.clone()),
            aivi::Pattern::At { name, pattern, .. } => {
                out.push(name.name.clone());
                names_in_pattern(pattern, out);
            }
            aivi::Pattern::Tuple { items, .. } => {
                items.iter().for_each(|p| names_in_pattern(p, out))
            }
            aivi::Pattern::List { items, rest, .. } => {
                items.iter().for_each(|p| names_in_pattern(p, out));
                if let Some(rest) = rest.as_ref() {
                    names_in_pattern(rest, out);
                }
            }
            aivi::Pattern::Record { fields, .. } => fields
                .iter()
                .for_each(|f| names_in_pattern(&f.pattern, out)),
            aivi::Pattern::Constructor { args, .. } => {
                args.iter().for_each(|p| names_in_pattern(p, out))
            }
            aivi::Pattern::Wildcard(_) | aivi::Pattern::Literal(_) => {}
        }
    }

    fn expr_uses_name(expr: &aivi::Expr, name: &str) -> bool {
        match expr {
            aivi::Expr::Ident(n) => n.name == name,
            aivi::Expr::Tuple { items, .. } => items.iter().any(|e| expr_uses_name(e, name)),
            aivi::Expr::List { items, .. } => items.iter().any(|i| expr_uses_name(&i.expr, name)),
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => {
                fields.iter().any(|f| expr_uses_name(&f.value, name))
            }
            aivi::Expr::Call { func, args, .. } => {
                expr_uses_name(func, name) || args.iter().any(|a| expr_uses_name(a, name))
            }
            aivi::Expr::Lambda { params, body, .. } => {
                // Conservative: if the lambda binds the name, treat it as not used in body for outer scope.
                let mut bound = Vec::new();
                for p in params {
                    names_in_pattern(p, &mut bound);
                }
                if bound.iter().any(|b| b == name) {
                    false
                } else {
                    expr_uses_name(body, name)
                }
            }
            aivi::Expr::Match {
                scrutinee, arms, ..
            } => {
                scrutinee.as_ref().is_some_and(|e| expr_uses_name(e, name))
                    || arms.iter().any(|arm| {
                        expr_uses_name(&arm.body, name)
                            || arm.guard.as_ref().is_some_and(|g| expr_uses_name(g, name))
                    })
            }
            aivi::Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                expr_uses_name(cond, name)
                    || expr_uses_name(then_branch, name)
                    || expr_uses_name(else_branch, name)
            }
            aivi::Expr::Binary { left, right, .. } => {
                expr_uses_name(left, name) || expr_uses_name(right, name)
            }
            aivi::Expr::Block { items, .. } => items.iter().any(|item| match item {
                aivi::BlockItem::Bind { expr, .. }
                | aivi::BlockItem::Let { expr, .. }
                | aivi::BlockItem::Filter { expr, .. }
                | aivi::BlockItem::Yield { expr, .. }
                | aivi::BlockItem::Recurse { expr, .. }
                | aivi::BlockItem::Expr { expr, .. } => expr_uses_name(expr, name),
                aivi::BlockItem::When { cond, effect, .. }
                | aivi::BlockItem::Unless { cond, effect, .. } => {
                    expr_uses_name(cond, name) || expr_uses_name(effect, name)
                }
                aivi::BlockItem::Given {
                    cond, fail_expr, ..
                } => expr_uses_name(cond, name) || expr_uses_name(fail_expr, name),
            }),
            aivi::Expr::FieldAccess { base, .. }
            | aivi::Expr::Index { base, .. }
            | aivi::Expr::Suffixed { base, .. } => expr_uses_name(base, name),
            aivi::Expr::UnaryNeg { expr, .. } => expr_uses_name(expr, name),
            aivi::Expr::TextInterpolate { parts, .. } => parts.iter().any(|p| match p {
                aivi::TextPart::Text { .. } => false,
                aivi::TextPart::Expr { expr, .. } => expr_uses_name(expr, name),
            }),
            aivi::Expr::Literal(_) | aivi::Expr::FieldSection { .. } | aivi::Expr::Raw { .. } => {
                false
            }
            aivi::Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                substitutions
                    .iter()
                    .any(|sub| sub.value.as_ref().is_some_and(|v| expr_uses_name(v, name)))
                    || expr_uses_name(body, name)
            }
            aivi::Expr::Flow { root, .. } => expr_uses_name(root, name),
        }
    }

    fn check_block(kind: BlockKind, items: &[aivi::BlockItem], out: &mut Vec<Diagnostic>) {
        // Rule: block last item should be an expression/yield, not a binding.
        if let Some(last) = items.last() {
            if matches!(
                last,
                aivi::BlockItem::Let { .. } | aivi::BlockItem::Bind { .. }
            ) {
                let span = match last {
                    aivi::BlockItem::Let { span, .. } | aivi::BlockItem::Bind { span, .. } => {
                        span.clone()
                    }
                    _ => unreachable!(),
                };
                let cat = match kind {
                    BlockKind::Do { .. } => StrictCategory::Effect,
                    _ => StrictCategory::Style,
                };
                push_simple(
                    out,
                    "AIVI-S220",
                    cat,
                    DiagnosticSeverity::WARNING,
                    format!(
                        "AIVI-S220 [{}]\nBlock ends with a binding.\nFix: Add a final expression (the block result), or convert the binding into a pure expression.",
                        cat.as_str()
                    ),
                    span,
                );
            }
        }

        // Rule: unused bound names inside a block (simple forward-use check).
        for (idx, item) in items.iter().enumerate() {
            let (pat, expr, span) = match item {
                aivi::BlockItem::Bind {
                    pattern,
                    expr,
                    span,
                } => (Some(pattern), Some(expr), Some(span)),
                aivi::BlockItem::Let {
                    pattern,
                    expr,
                    span,
                } => (Some(pattern), Some(expr), Some(span)),
                _ => (None, None, None),
            };
            let (Some(pat), Some(_expr), Some(span)) = (pat, expr, span) else {
                continue;
            };
            let mut bound = Vec::new();
            names_in_pattern(pat, &mut bound);
            if bound.is_empty() {
                continue;
            }
            let rest_items = &items[idx + 1..];
            for name in bound {
                let used_later = rest_items.iter().any(|it| match it {
                    aivi::BlockItem::Bind { expr, .. }
                    | aivi::BlockItem::Let { expr, .. }
                    | aivi::BlockItem::Filter { expr, .. }
                    | aivi::BlockItem::Yield { expr, .. }
                    | aivi::BlockItem::Recurse { expr, .. }
                    | aivi::BlockItem::Expr { expr, .. } => expr_uses_name(expr, &name),
                    aivi::BlockItem::When { cond, effect, .. }
                    | aivi::BlockItem::Unless { cond, effect, .. } => {
                        expr_uses_name(cond, &name) || expr_uses_name(effect, &name)
                    }
                    aivi::BlockItem::Given {
                        cond, fail_expr, ..
                    } => expr_uses_name(cond, &name) || expr_uses_name(fail_expr, &name),
                });
                if !used_later && !name.starts_with('_') {
                    push_simple(
                        out,
                        "AIVI-S221",
                        StrictCategory::Style,
                        DiagnosticSeverity::WARNING,
                        format!(
                            "AIVI-S221 [{}]\nUnused binding in block.\nFound: `{name}`.\nFix: Use the value, or rename to `_`/`_name` to mark it intentionally unused.",
                            StrictCategory::Style.as_str()
                        ),
                        span.clone(),
                    );
                }
            }
        }
    }

    fn walk_expr(expr: &aivi::Expr, out: &mut Vec<Diagnostic>) {
        if let aivi::Expr::Block { kind, items, .. } = expr {
            check_block(kind.clone(), items, out);
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
        // Keep it small: other passes already walk expressions.
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
