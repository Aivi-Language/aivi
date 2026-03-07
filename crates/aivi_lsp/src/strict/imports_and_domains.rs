use std::collections::{HashMap, HashSet};

use aivi::{desugar_blocks, elaborate_expected_coercions, HirExpr, HirTextPart, Module};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, TextEdit};

use super::{diag_with_fix, expr_span, push_simple, StrictCategory, StrictConfig, StrictFix};
use crate::backend::Backend;

pub(super) fn strict_import_hygiene(file_modules: &[Module], out: &mut Vec<Diagnostic>) {
    for module in file_modules {
        let mut seen_use: HashSet<(&str, bool)> = HashSet::new();
        for use_decl in &module.uses {
            let key = (use_decl.module.name.as_str(), use_decl.wildcard);
            if !seen_use.insert(key) {
                push_simple(
                    out,
                    "AIVI-S200",
                    StrictCategory::Import,
                    DiagnosticSeverity::WARNING,
                    format!(
                        "AIVI-S200 [{}]\nDuplicate `use` declaration.\nFound: `use {}`.\nFix: Remove the duplicate import.",
                        StrictCategory::Import.as_str(),
                        use_decl.module.name
                    ),
                    use_decl.module.span.clone(),
                );
            }
        }
    }
}

pub(super) fn strict_missing_import_suggestions(
    file_modules: &[Module],
    all_modules: &[Module],
    out: &mut Vec<Diagnostic>,
) {
    // Best-effort: if a name is used but not defined in the module, suggest a `use`.
    // We intentionally keep this heuristic simple and only fire when there's a single obvious provider.
    let mut providers: HashMap<&str, Vec<&str>> = HashMap::new();
    for m in all_modules {
        for export in &m.exports {
            providers
                .entry(export.name.name.as_str())
                .or_default()
                .push(m.name.name.as_str());
        }
    }

    fn collect_idents(expr: &aivi::Expr, out: &mut Vec<aivi::SpannedName>) {
        match expr {
            aivi::Expr::Ident(n) => out.push(n.clone()),
            aivi::Expr::Tuple { items, .. } => items.iter().for_each(|e| collect_idents(e, out)),
            aivi::Expr::List { items, .. } => {
                items.iter().for_each(|i| collect_idents(&i.expr, out))
            }
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => {
                fields.iter().for_each(|f| collect_idents(&f.value, out))
            }
            aivi::Expr::Call { func, args, .. } => {
                collect_idents(func, out);
                args.iter().for_each(|a| collect_idents(a, out));
            }
            aivi::Expr::Lambda { body, .. } => collect_idents(body, out),
            aivi::Expr::Match {
                scrutinee, arms, ..
            } => {
                if let Some(s) = scrutinee {
                    collect_idents(s, out);
                }
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        collect_idents(guard, out);
                    }
                    collect_idents(&arm.body, out);
                }
            }
            aivi::Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                collect_idents(cond, out);
                collect_idents(then_branch, out);
                collect_idents(else_branch, out);
            }
            aivi::Expr::Binary { left, right, .. } => {
                collect_idents(left, out);
                collect_idents(right, out);
            }
            aivi::Expr::Block { items, .. } => {
                for item in items {
                    match item {
                        aivi::BlockItem::Bind { expr, .. }
                        | aivi::BlockItem::Let { expr, .. }
                        | aivi::BlockItem::Filter { expr, .. }
                        | aivi::BlockItem::Yield { expr, .. }
                        | aivi::BlockItem::Recurse { expr, .. }
                        | aivi::BlockItem::Expr { expr, .. } => collect_idents(expr, out),
                        aivi::BlockItem::When { cond, effect, .. }
                        | aivi::BlockItem::Unless { cond, effect, .. } => {
                            collect_idents(cond, out);
                            collect_idents(effect, out);
                        }
                        aivi::BlockItem::Given {
                            cond, fail_expr, ..
                        } => {
                            collect_idents(cond, out);
                            collect_idents(fail_expr, out);
                        }
                        aivi::BlockItem::On {
                            transition,
                            handler,
                            ..
                        } => {
                            collect_idents(transition, out);
                            collect_idents(handler, out);
                        }
                    }
                }
            }
            aivi::Expr::UnaryNeg { expr, .. } => collect_idents(expr, out),
            aivi::Expr::FieldAccess { base, .. }
            | aivi::Expr::Index { base, .. }
            | aivi::Expr::Suffixed { base, .. } => collect_idents(base, out),
            aivi::Expr::CapabilityScope { body, .. } => collect_idents(body, out),
            aivi::Expr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let aivi::TextPart::Expr { expr, .. } = part {
                        collect_idents(expr, out);
                    }
                }
            }
            aivi::Expr::Literal(_) | aivi::Expr::FieldSection { .. } | aivi::Expr::Raw { .. } => {}
            aivi::Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                for sub in substitutions {
                    if let Some(v) = &sub.value {
                        collect_idents(v, out);
                    }
                }
                collect_idents(body, out);
            }
        }
    }

    for module in file_modules {
        // Diagnostics are published per-document; only emit for modules defined in this file.

        let mut defined: HashSet<&str> = HashSet::new();
        for item in &module.items {
            match item {
                aivi::ModuleItem::Def(def) => {
                    defined.insert(def.name.name.as_str());
                }
                aivi::ModuleItem::TypeSig(sig) => {
                    defined.insert(sig.name.name.as_str());
                }
                _ => {}
            }
        }
        for use_decl in &module.uses {
            if use_decl.wildcard {
                // We treat wildcard as defining all exports; no suggestions.
                continue;
            }
            for it in &use_decl.items {
                defined.insert(it.name.name.as_str());
            }
        }

        for item in &module.items {
            let aivi::ModuleItem::Def(def) = item else {
                continue;
            };
            let mut used = Vec::new();
            collect_idents(&def.expr, &mut used);
            for name in used {
                if defined.contains(name.name.as_str()) {
                    continue;
                }
                // Prefer suggestions for UpperIdent constructors/types used as values.
                if !name
                    .name
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_uppercase())
                {
                    continue;
                }
                let Some(cands) = providers.get(name.name.as_str()) else {
                    continue;
                };
                if cands.len() != 1 {
                    continue;
                }
                let module_name = cands[0];
                let insert_span = module.name.span.clone();
                let insert_at = aivi::Span {
                    start: insert_span.start.clone(),
                    end: insert_span.start.clone(),
                };
                let edit = TextEdit {
                    range: Backend::span_to_range(insert_at),
                    new_text: format!("use {module_name}\n\n"),
                };
                out.push(diag_with_fix(
                    "AIVI-S201",
                    StrictCategory::Import,
                    DiagnosticSeverity::WARNING,
                    format!(
                        "AIVI-S201 [{}]\nName not in scope.\nFound: `{}`.\nFix: Insert `use {module_name}`.",
                        StrictCategory::Import.as_str(),
                        name.name
                    ),
                    Backend::span_to_range(name.span.clone()),
                    Some(StrictFix {
                        title: format!("Insert `use {module_name}`"),
                        edits: vec![edit],
                        is_preferred: false,
                    }),
                ));
            }
        }
    }
}

pub(super) fn strict_domain_operator_heuristics(
    file_modules: &[Module],
    out: &mut Vec<Diagnostic>,
) {
    // This is intentionally heuristic until we have typed spans in the LSP.
    // Emit a warning for `Date + Int`-like shapes (common footgun: missing unit/delta domain).
    fn walk_expr(expr: &aivi::Expr, out: &mut Vec<Diagnostic>) {
        match expr {
            aivi::Expr::Binary {
                op,
                left,
                right,
                span,
            } if op == "+" || op == "-" => {
                let left_is_date_like = matches!(
                    &**left,
                    aivi::Expr::Ident(n) if n.name.to_lowercase().contains("date")
                );
                let right_is_number =
                    matches!(&**right, aivi::Expr::Literal(aivi::Literal::Number { .. }));
                if left_is_date_like && right_is_number {
                    push_simple(
                        out,
                        "AIVI-S400",
                        StrictCategory::Domain,
                        DiagnosticSeverity::WARNING,
                        format!(
                            "AIVI-S400 [{}]\nPotential domain ambiguity.\nFound: date-like value `{}` {} numeric literal.\nFix: Use an explicit delta/unit (e.g. `1day`, `24h`) or a domain-specific add function.",
                            StrictCategory::Domain.as_str(),
                            match &**left { aivi::Expr::Ident(n) => &n.name, _ => "?" },
                            op
                        ),
                        span.clone(),
                    );
                }
                walk_expr(left, out);
                walk_expr(right, out);
            }
            _ => {}
        }
        // Keep this pass shallow; other checks already traverse expressions.
    }

    for module in file_modules {
        for item in &module.items {
            if let aivi::ModuleItem::Def(def) = item {
                walk_expr(&def.expr, out);
            }
        }
    }
}

pub(super) fn strict_expected_type_coercions(
    file_module_names: &HashSet<String>,
    all_modules: &[Module],
    config: &StrictConfig,
    out: &mut Vec<Diagnostic>,
) {
    // Best-effort: run expected-type elaboration on a clone, then detect inserted `toText`/`TextNode`.
    // This catches the most impactful implicit coercions without requiring exposing typed spans yet.
    let mut modules = all_modules.to_vec();
    let diags = elaborate_expected_coercions(&mut modules);
    // If elaboration itself emits type errors, the normal typechecker already covers that.
    let _ = diags;

    fn collect_calls(expr: &aivi::Expr, out: &mut Vec<(String, aivi::Span, aivi::Span)>) {
        match expr {
            aivi::Expr::Call { func, args, span } => {
                if let aivi::Expr::Ident(name) = &**func {
                    if name.name == "toText" || name.name == "TextNode" {
                        let arg_span = args.first().map(expr_span);
                        if let Some(arg_span) = arg_span {
                            out.push((name.name.clone(), span.clone(), arg_span));
                        }
                    }
                }
                collect_calls(func, out);
                for arg in args {
                    collect_calls(arg, out);
                }
            }
            aivi::Expr::Tuple { items, .. } => items.iter().for_each(|e| collect_calls(e, out)),
            aivi::Expr::List { items, .. } => {
                items.iter().for_each(|i| collect_calls(&i.expr, out))
            }
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => {
                fields.iter().for_each(|f| collect_calls(&f.value, out))
            }
            aivi::Expr::Lambda { body, .. } => collect_calls(body, out),
            aivi::Expr::Match {
                scrutinee, arms, ..
            } => {
                if let Some(s) = scrutinee {
                    collect_calls(s, out);
                }
                for arm in arms {
                    if let Some(g) = &arm.guard {
                        collect_calls(g, out);
                    }
                    collect_calls(&arm.body, out);
                }
            }
            aivi::Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                collect_calls(cond, out);
                collect_calls(then_branch, out);
                collect_calls(else_branch, out);
            }
            aivi::Expr::Binary { left, right, .. } => {
                collect_calls(left, out);
                collect_calls(right, out);
            }
            aivi::Expr::Block { items, .. } => {
                for item in items {
                    match item {
                        aivi::BlockItem::Bind { expr, .. }
                        | aivi::BlockItem::Let { expr, .. }
                        | aivi::BlockItem::Filter { expr, .. }
                        | aivi::BlockItem::Yield { expr, .. }
                        | aivi::BlockItem::Recurse { expr, .. }
                        | aivi::BlockItem::Expr { expr, .. } => collect_calls(expr, out),
                        aivi::BlockItem::When { cond, effect, .. }
                        | aivi::BlockItem::Unless { cond, effect, .. } => {
                            collect_calls(cond, out);
                            collect_calls(effect, out);
                        }
                        aivi::BlockItem::Given {
                            cond, fail_expr, ..
                        } => {
                            collect_calls(cond, out);
                            collect_calls(fail_expr, out);
                        }
                        aivi::BlockItem::On {
                            transition,
                            handler,
                            ..
                        } => {
                            collect_calls(transition, out);
                            collect_calls(handler, out);
                        }
                    }
                }
            }
            aivi::Expr::UnaryNeg { expr, .. } => collect_calls(expr, out),
            aivi::Expr::FieldAccess { base, .. }
            | aivi::Expr::Index { base, .. }
            | aivi::Expr::Suffixed { base, .. } => collect_calls(base, out),
            aivi::Expr::CapabilityScope { body, .. } => collect_calls(body, out),
            aivi::Expr::TextInterpolate { parts, .. } => {
                for p in parts {
                    if let aivi::TextPart::Expr { expr, .. } = p {
                        collect_calls(expr, out);
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
                        collect_calls(v, out);
                    }
                }
                collect_calls(body, out);
            }
        }
    }

    // Compare elaborated defs to a non-elaborated parse to find newly-introduced coercions.
    // This is span-based; if the user already wrote `toText`, we won't flag unless the coercion
    // appears at a span that used to be non-coercing.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct SpanKey {
        sl: usize,
        sc: usize,
        el: usize,
        ec: usize,
    }

    impl From<&aivi::Span> for SpanKey {
        fn from(span: &aivi::Span) -> Self {
            Self {
                sl: span.start.line,
                sc: span.start.column,
                el: span.end.line,
                ec: span.end.column,
            }
        }
    }

    let mut original: HashMap<(String, String), HashSet<(String, SpanKey)>> = HashMap::new(); // (module, def) -> {(callee, call_span)}
    for m in all_modules.iter().cloned() {
        for item in &m.items {
            if let aivi::ModuleItem::Def(def) = item {
                let mut calls = Vec::new();
                collect_calls(&def.expr, &mut calls);
                let mut set = HashSet::new();
                for (callee, call_span, _arg_span) in calls {
                    set.insert((callee, SpanKey::from(&call_span)));
                }
                original.insert((m.name.name.clone(), def.name.name.clone()), set);
            }
        }
    }

    for m in &modules {
        if !file_module_names.contains(&m.name.name) {
            continue;
        }
        for item in &m.items {
            let aivi::ModuleItem::Def(def) = item else {
                continue;
            };
            let mut calls = Vec::new();
            collect_calls(&def.expr, &mut calls);
            let key = (m.name.name.clone(), def.name.name.clone());
            let before = original.get(&key);
            for (callee, call_span, arg_span) in calls {
                if before.is_some_and(|b| b.contains(&(callee.clone(), SpanKey::from(&call_span))))
                {
                    continue;
                }
                if call_span.start.line == 0 {
                    continue;
                }
                let sev = if config.forbid_implicit_coercions {
                    DiagnosticSeverity::ERROR
                } else {
                    DiagnosticSeverity::WARNING
                };
                let code = "AIVI-S500";
                out.push(diag_with_fix(
                    code,
                    StrictCategory::Type,
                    sev,
                    format!(
                        "{code} [{}]\nImplicit coercion inserted.\nFound: compiler-inserted `{callee}`.\nFix: Write the coercion explicitly (e.g. `{callee} <expr>`) or adjust types to avoid it.",
                        StrictCategory::Type.as_str(),
                    ),
                    Backend::span_to_range(arg_span),
                    None,
                ));
            }
        }
    }
}

pub(super) fn strict_kernel_consistency(
    all_modules: &[Module],
    span_hint: aivi::Span,
    out: &mut Vec<Diagnostic>,
) {
    // Best-effort: lower to kernel and validate shallow invariants.
    let kernel = match std::panic::catch_unwind(|| {
        let hir = aivi::desugar_modules(all_modules);
        desugar_blocks(hir)
    }) {
        Ok(k) => k,
        Err(_) => return,
    };

    // Kernel does not carry spans. Attach "compiler bug" invariants to the current file's
    // first module span so they are visible but scoped to the active document.
    let mut seen_ids = HashSet::new();

    fn walk_expr(
        expr: &HirExpr,
        seen_ids: &mut HashSet<u32>,
        span_hint: &aivi::Span,
        out: &mut Vec<Diagnostic>,
    ) {
        let id = match expr {
            HirExpr::Var { id, .. }
            | HirExpr::LitNumber { id, .. }
            | HirExpr::LitString { id, .. }
            | HirExpr::TextInterpolate { id, .. }
            | HirExpr::LitSigil { id, .. }
            | HirExpr::LitBool { id, .. }
            | HirExpr::LitDateTime { id, .. }
            | HirExpr::Lambda { id, .. }
            | HirExpr::App { id, .. }
            | HirExpr::Call { id, .. }
            | HirExpr::DebugFn { id, .. }
            | HirExpr::Pipe { id, .. }
            | HirExpr::List { id, .. }
            | HirExpr::Tuple { id, .. }
            | HirExpr::Record { id, .. }
            | HirExpr::Patch { id, .. }
            | HirExpr::FieldAccess { id, .. }
            | HirExpr::Index { id, .. }
            | HirExpr::Match { id, .. }
            | HirExpr::If { id, .. }
            | HirExpr::Binary { id, .. }
            | HirExpr::Raw { id, .. }
            | HirExpr::Mock { id, .. }
            | HirExpr::Block { id, .. } => *id,
        };

        if !seen_ids.insert(id) {
            push_simple(
                out,
                "AIVI-S900",
                StrictCategory::Kernel,
                DiagnosticSeverity::WARNING,
                format!(
                    "AIVI-S900 [{}]\nKernel invariant violated.\nFound: duplicate expression id `{id}`.\nFix: Report a compiler bug (kernel ids must be unique).",
                    StrictCategory::Kernel.as_str()
                ),
                span_hint.clone(),
            );
        }

        match expr {
            HirExpr::Lambda { body, .. } => walk_expr(body, seen_ids, span_hint, out),
            HirExpr::App { func, arg, .. } => {
                walk_expr(func, seen_ids, span_hint, out);
                walk_expr(arg, seen_ids, span_hint, out);
            }
            HirExpr::Call { func, args, .. } => {
                walk_expr(func, seen_ids, span_hint, out);
                for a in args {
                    walk_expr(a, seen_ids, span_hint, out);
                }
            }
            HirExpr::DebugFn { body, .. } => walk_expr(body, seen_ids, span_hint, out),
            HirExpr::Pipe { func, arg, .. } => {
                walk_expr(func, seen_ids, span_hint, out);
                walk_expr(arg, seen_ids, span_hint, out);
            }
            HirExpr::TextInterpolate { parts, .. } => {
                for p in parts {
                    if let HirTextPart::Expr { expr } = p {
                        walk_expr(expr, seen_ids, span_hint, out);
                    }
                }
            }
            HirExpr::List { items, .. } => {
                for it in items {
                    walk_expr(&it.expr, seen_ids, span_hint, out);
                }
            }
            HirExpr::Tuple { items, .. } => {
                if items.is_empty() {
                    push_simple(
                        out,
                        "AIVI-S901",
                        StrictCategory::Kernel,
                        DiagnosticSeverity::WARNING,
                        format!(
                            "AIVI-S901 [{}]\nKernel invariant violated.\nFound: empty tuple.\nFix: Report a compiler bug (tuples are 2+ items in surface syntax).",
                            StrictCategory::Kernel.as_str()
                        ),
                        span_hint.clone(),
                    );
                }
                for it in items {
                    walk_expr(it, seen_ids, span_hint, out);
                }
            }
            HirExpr::Record { fields, .. } => {
                for f in fields {
                    walk_expr(&f.value, seen_ids, span_hint, out);
                }
            }
            HirExpr::Patch { target, fields, .. } => {
                walk_expr(target, seen_ids, span_hint, out);
                for f in fields {
                    walk_expr(&f.value, seen_ids, span_hint, out);
                }
            }
            HirExpr::FieldAccess { base, .. } => walk_expr(base, seen_ids, span_hint, out),
            HirExpr::Index { base, index, .. } => {
                walk_expr(base, seen_ids, span_hint, out);
                walk_expr(index, seen_ids, span_hint, out);
            }
            HirExpr::Match {
                scrutinee, arms, ..
            } => {
                walk_expr(scrutinee, seen_ids, span_hint, out);
                for arm in arms {
                    if let Some(g) = &arm.guard {
                        walk_expr(g, seen_ids, span_hint, out);
                    }
                    walk_expr(&arm.body, seen_ids, span_hint, out);
                }
            }
            HirExpr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                walk_expr(cond, seen_ids, span_hint, out);
                walk_expr(then_branch, seen_ids, span_hint, out);
                walk_expr(else_branch, seen_ids, span_hint, out);
            }
            HirExpr::Binary { left, right, .. } => {
                walk_expr(left, seen_ids, span_hint, out);
                walk_expr(right, seen_ids, span_hint, out);
            }
            HirExpr::Var { .. }
            | HirExpr::LitNumber { .. }
            | HirExpr::LitString { .. }
            | HirExpr::LitSigil { .. }
            | HirExpr::LitBool { .. }
            | HirExpr::LitDateTime { .. }
            | HirExpr::Raw { .. } => {}
            HirExpr::Mock {
                substitutions,
                body,
                ..
            } => {
                for sub in substitutions {
                    if let Some(v) = &sub.value {
                        walk_expr(v, seen_ids, span_hint, out);
                    }
                }
                walk_expr(body, seen_ids, span_hint, out);
            }
            HirExpr::Block { .. } => {
                // Blocks should be desugared away; if one slips through, flag it.
                push_simple(
                    out,
                    "AIVI-S902",
                    StrictCategory::Kernel,
                    DiagnosticSeverity::WARNING,
                    format!(
                        "AIVI-S902 [{}]\nKernel invariant violated.\nFound: Block node after desugaring.\nFix: Report a compiler bug.",
                        StrictCategory::Kernel.as_str()
                    ),
                    span_hint.clone(),
                );
            }
        }
    }

    for module in &kernel.modules {
        for def in &module.defs {
            walk_expr(&def.expr, &mut seen_ids, &span_hint, out);
        }
    }
}
