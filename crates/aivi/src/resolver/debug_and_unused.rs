fn check_debug_decorators(def: &Def, diagnostics: &mut Vec<FileDiagnostic>, module: &Module) {
    fn expr_span(expr: &Expr) -> crate::diagnostics::Span {
        match expr {
            Expr::Ident(name) => name.span.clone(),
            Expr::Literal(literal) => match literal {
                Literal::Number { span, .. }
                | Literal::String { span, .. }
                | Literal::Sigil { span, .. }
                | Literal::Bool { span, .. }
                | Literal::DateTime { span, .. } => span.clone(),
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
            | Expr::Mock { span, .. }
            | Expr::Raw { span, .. } => span.clone(),
        }
    }

    let allowed = ["pipes", "args", "return", "time"];
    let has_debug = def.decorators.iter().any(|d| d.name.name == "debug");
    if !has_debug {
        return;
    }
    if def.params.is_empty() {
        diagnostics.push(file_diag(
            module,
            Diagnostic {
                code: "E2010".to_string(),
                severity: DiagnosticSeverity::Error,
                message: "`@debug` can only be applied to function definitions".to_string(),
                span: def.name.span.clone(),
                labels: Vec::new(),
                hints: Vec::new(),
                suggestion: None,
            },
        ));
    }

    for decorator in def.decorators.iter().filter(|d| d.name.name == "debug") {
        let mut params: Vec<crate::surface::SpannedName> = Vec::new();
        match &decorator.arg {
            None => {}
            Some(Expr::Tuple { items, .. }) => {
                for item in items {
                    match item {
                        Expr::Ident(name) => params.push(name.clone()),
                        other => {
                            diagnostics.push(file_diag(
                                module,
                                Diagnostic {
                                    code: "E2011".to_string(),
                                    severity: DiagnosticSeverity::Error,
                                    message:
                                        "`@debug` expects a list of parameter names (e.g. `@debug(pipes, args, return, time)`)".to_string(),
                                    span: expr_span(other),
                                    labels: Vec::new(),
                                    hints: Vec::new(),
                                    suggestion: None,
                                },
                            ));
                        }
                    }
                }
            }
            Some(Expr::Ident(name)) => params.push(name.clone()),
            Some(other) => {
                diagnostics.push(file_diag(
                    module,
                    Diagnostic {
                        code: "E2011".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message:
                            "`@debug` expects `@debug(pipes, args, return, time)` (or `@debug()`)"
                                .to_string(),
                        span: expr_span(other),
                        labels: Vec::new(),
                        hints: Vec::new(),
                        suggestion: None,
                    },
                ));
                continue;
            }
        }

        for param in params {
            if !allowed.contains(&param.name.as_str()) {
                diagnostics.push(file_diag(
                    module,
                    Diagnostic {
                        code: "E2012".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: format!(
                            "unknown `@debug` parameter `{}` (expected: pipes, args, return, time)",
                            param.name
                        ),
                        span: param.span,
                        labels: Vec::new(),
                        hints: Vec::new(),
                        suggestion: None,
                    },
                ));
            }
        }
    }
}

fn check_expr(
    expr: &Expr,
    scope: &mut HashMap<String, Option<String>>,
    diagnostics: &mut Vec<FileDiagnostic>,
    module: &Module,
    allow_unknown: bool,
) {
    match expr {
        Expr::UnaryNeg { expr, .. } => {
            check_expr(expr, scope, diagnostics, module, allow_unknown);
        }
        Expr::Suffixed { base, .. } => {
            check_expr(base, scope, diagnostics, module, allow_unknown);
        }
        Expr::TextInterpolate { parts, .. } => {
            for part in parts {
                if let TextPart::Expr { expr, .. } = part {
                    check_expr(expr, scope, diagnostics, module, allow_unknown);
                }
            }
        }
        Expr::Ident(name) => {
            if name.name == "_" {
                return;
            }
            if is_constructor_name(&name.name) {
                return;
            }
            if is_builtin_name(&name.name) {
                return;
            }
            if allow_unknown {
                return;
            }
            if let Some(Some(message)) = scope.get(&name.name) {
                diagnostics.push(file_diag(
                    module,
                    Diagnostic {
                        code: "W2500".to_string(),
                        severity: DiagnosticSeverity::Warning,
                        message: format!("use of deprecated name '{}': {}", name.name, message),
                        span: name.span.clone(),
                        labels: Vec::new(),
                        hints: Vec::new(),
                        suggestion: None,
                    },
                ));
            }
            if !scope.contains_key(&name.name) {
                let message = special_unknown_name_message(&name.name)
                    .unwrap_or_else(|| format!("unknown name '{}'", name.name));
                diagnostics.push(file_diag(
                    module,
                    Diagnostic {
                        code: "E2005".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message,
                        span: name.span.clone(),
                        labels: Vec::new(),
                        hints: Vec::new(),
                        suggestion: None,
                    },
                ));
            }
        }
        Expr::Literal(_) => {}
        Expr::List { items, .. } => {
            for item in items {
                check_expr(&item.expr, scope, diagnostics, module, allow_unknown);
            }
        }
        Expr::Tuple { items, .. } => {
            for item in items {
                check_expr(item, scope, diagnostics, module, allow_unknown);
            }
        }
        Expr::Record { fields, .. } => {
            for field in fields {
                check_expr(&field.value, scope, diagnostics, module, allow_unknown);
            }
        }
        Expr::PatchLit { fields, .. } => {
            for field in fields {
                check_expr(&field.value, scope, diagnostics, module, allow_unknown);
            }
        }
        Expr::FieldAccess { base, .. } => {
            check_expr(base, scope, diagnostics, module, allow_unknown);
        }
        Expr::FieldSection { .. } => {}
        Expr::Index { base, index, .. } => {
            check_expr(base, scope, diagnostics, module, allow_unknown);
            check_expr(index, scope, diagnostics, module, allow_unknown);
        }
        Expr::Call { func, args, .. } => {
            check_expr(func, scope, diagnostics, module, allow_unknown);
            for (index, arg) in args.iter().enumerate() {
                let arg_allow_unknown =
                    allow_unknown || call_arg_allows_function_lifting(func, index);
                check_expr(arg, scope, diagnostics, module, arg_allow_unknown);
            }
        }
        Expr::Lambda { params, body, .. } => {
            let mut inner_scope = scope.clone();
            collect_pattern_bindings(params, &mut inner_scope);
            check_expr(body, &mut inner_scope, diagnostics, module, allow_unknown);
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            if let Some(scrutinee) = scrutinee {
                check_expr(scrutinee, scope, diagnostics, module, allow_unknown);
            }
            for arm in arms {
                let mut arm_scope = scope.clone();
                collect_pattern_binding(&arm.pattern, &mut arm_scope);
                if let Some(guard) = &arm.guard {
                    check_expr(guard, &mut arm_scope, diagnostics, module, allow_unknown);
                }
                check_expr(
                    &arm.body,
                    &mut arm_scope,
                    diagnostics,
                    module,
                    allow_unknown,
                );
            }
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            check_expr(cond, scope, diagnostics, module, allow_unknown);
            check_expr(then_branch, scope, diagnostics, module, allow_unknown);
            check_expr(else_branch, scope, diagnostics, module, allow_unknown);
        }
        Expr::Binary { op, left, right, .. } => {
            check_expr(left, scope, diagnostics, module, allow_unknown);
            let right_allow_unknown =
                allow_unknown || binary_rhs_allows_function_lifting(op.as_str());
            check_expr(right, scope, diagnostics, module, right_allow_unknown);
        }
        Expr::Block { items, .. } => {
            let mut block_scope = scope.clone();
            for item in items {
                match item {
                    BlockItem::Bind { pattern, expr, .. } => {
                        check_expr(expr, &mut block_scope, diagnostics, module, allow_unknown);
                        collect_pattern_binding(pattern, &mut block_scope);
                    }
                    BlockItem::Let { pattern, expr, .. } => {
                        // Compiler-generated let bindings (e.g. `__loop` from
                        // loop/recurse desugaring) may be self-referential.
                        // Pre-add them to scope so the recursive reference
                        // passes the resolver check.  The runtime supports
                        // this via shared mutable environment capture.
                        if pattern_is_generated(pattern) {
                            collect_pattern_binding(pattern, &mut block_scope);
                        }
                        check_expr(expr, &mut block_scope, diagnostics, module, allow_unknown);
                        collect_pattern_binding(pattern, &mut block_scope);
                    }
                    BlockItem::Filter { expr, .. } => {
                        check_expr(expr, &mut block_scope, diagnostics, module, true);
                    }
                    BlockItem::Yield { expr, .. }
                    | BlockItem::Recurse { expr, .. }
                    | BlockItem::Expr { expr, .. } => {
                        check_expr(expr, &mut block_scope, diagnostics, module, allow_unknown);
                    }
                    BlockItem::When { cond, effect, .. }
                    | BlockItem::Unless { cond, effect, .. } => {
                        check_expr(cond, &mut block_scope, diagnostics, module, allow_unknown);
                        check_expr(effect, &mut block_scope, diagnostics, module, allow_unknown);
                    }
                    BlockItem::Given {
                        cond, fail_expr, ..
                    } => {
                        check_expr(cond, &mut block_scope, diagnostics, module, allow_unknown);
                        check_expr(
                            fail_expr,
                            &mut block_scope,
                            diagnostics,
                            module,
                            allow_unknown,
                        );
                    }
                }
            }
        }
        Expr::Raw { .. } => {}
        Expr::Mock { substitutions, body, .. } => {
            for sub in substitutions {
                if let Some(value) = &sub.value {
                    check_expr(value, scope, diagnostics, module, allow_unknown);
                }
            }
            check_expr(body, scope, diagnostics, module, allow_unknown);
        }
    }
}

fn special_unknown_name_message(name: &str) -> Option<String> {
    // Common “ported” keywords from other languages that AIVI intentionally does not have.
    match name {
        "return" => Some("unknown name 'return' (AIVI has no `return`; the last expression is the result)".to_string()),
        "mut" => Some("unknown name 'mut' (AIVI is immutable; use a new binding instead of mutation)".to_string()),
        "for" | "while" => Some(format!(
            "unknown name '{name}' (AIVI has no loops; use recursion, `generate`, or higher-order functions)"
        )),
        "null" | "undefined" => Some(format!(
            "unknown name '{name}' (AIVI has no nulls; use `Option`/`Result`)"
        )),
        _ => None,
    }
}

fn call_arg_allows_function_lifting(func: &Expr, index: usize) -> bool {
    let name = match func {
        Expr::Ident(name) => name.name.as_str(),
        Expr::FieldAccess { field, .. } => field.name.as_str(),
        _ => return false,
    };
    let base = name.rsplit('.').next().unwrap_or(name);

    matches!(
        (base, index),
        ("filter", 0)
            | ("find", 0)
            | ("takeWhile", 0)
            | ("dropWhile", 0)
            | ("map", 0)
            | ("partition", 0)
            | ("findMap", 0)
            | ("uniqueBy", 0)
            | ("sortBy", 0)
            | ("where", 0)
            | ("upd", 0)
            | ("del", 0)
            | ("ups", 0)
            | ("derive", 1)
    )
}

fn binary_rhs_allows_function_lifting(op: &str) -> bool {
    matches!(op, "|>" | "->>")
}

/// Returns true if the pattern is a compiler-generated binding (name starts with `__`).
fn pattern_is_generated(pattern: &Pattern) -> bool {
    matches!(pattern, Pattern::Ident(name) if name.name.starts_with("__"))
}

fn collect_pattern_bindings(patterns: &[Pattern], scope: &mut HashMap<String, Option<String>>) {
    for pattern in patterns {
        collect_pattern_binding(pattern, scope);
    }
}

fn collect_pattern_binding(pattern: &Pattern, scope: &mut HashMap<String, Option<String>>) {
    match pattern {
        Pattern::Wildcard(_) => {}
        Pattern::Ident(name) => {
            if !is_constructor_name(&name.name) {
                scope.insert(name.name.clone(), None);
            }
        }
        Pattern::SubjectIdent(name) => {
            if !is_constructor_name(&name.name) {
                scope.insert(name.name.clone(), None);
            }
        }
        Pattern::Literal(_) => {}
        Pattern::At { name, pattern, .. } => {
            if !is_constructor_name(&name.name) {
                scope.insert(name.name.clone(), None);
            }
            collect_pattern_binding(pattern, scope);
        }
        Pattern::Constructor { args, .. } => {
            for arg in args {
                collect_pattern_binding(arg, scope);
            }
        }
        Pattern::Tuple { items, .. } => {
            for item in items {
                collect_pattern_binding(item, scope);
            }
        }
        Pattern::List { items, rest, .. } => {
            for item in items {
                collect_pattern_binding(item, scope);
            }
            if let Some(rest) = rest {
                collect_pattern_binding(rest, scope);
            }
        }
        Pattern::Record { fields, .. } => {
            for field in fields {
                collect_pattern_binding(&field.pattern, scope);
            }
        }
    }
}

fn detect_cycles(module_map: &HashMap<String, &Module>) -> HashSet<String> {
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    let mut stack = Vec::new();
    let mut in_cycle = HashSet::new();

    for name in module_map.keys() {
        if visited.contains(name) {
            continue;
        }
        dfs(
            name,
            module_map,
            &mut visiting,
            &mut visited,
            &mut stack,
            &mut in_cycle,
        );
    }

    in_cycle
}

fn dfs(
    name: &str,
    module_map: &HashMap<String, &Module>,
    visiting: &mut HashSet<String>,
    visited: &mut HashSet<String>,
    stack: &mut Vec<String>,
    in_cycle: &mut HashSet<String>,
) {
    visiting.insert(name.to_string());
    stack.push(name.to_string());

    if let Some(module) = module_map.get(name) {
        for use_decl in &module.uses {
            let next = &use_decl.module.name;
            if !module_map.contains_key(next) {
                continue;
            }
            if visiting.contains(next) {
                if let Some(pos) = stack.iter().position(|entry| entry == next) {
                    for entry in &stack[pos..] {
                        in_cycle.insert(entry.clone());
                    }
                }
                continue;
            }
            if !visited.contains(next) {
                dfs(next, module_map, visiting, visited, stack, in_cycle);
            }
        }
    }

    visiting.remove(name);
    visited.insert(name.to_string());
    stack.pop();
}

fn is_constructor_name(name: &str) -> bool {
    name.chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false)
}

fn is_builtin_name(name: &str) -> bool {
    crate::builtin_names::is_builtin_name(name)
}

fn file_diag(module: &Module, diagnostic: Diagnostic) -> FileDiagnostic {
    FileDiagnostic {
        path: module.path.clone(),
        diagnostic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_aliasing_rewrites_and_resolves_wildcard_imports() {
        let source = r#"
module test.db_alias
use aivi.database as db

// `db.*` gets rewritten to `aivi.database.*` during parsing; resolver must treat these as in-scope.
x = db.table
y = db.applyDelta
z = db.configure
"#;

        let path = std::path::Path::new("test.aivi");
        let (mut modules, diags) = crate::surface::parse_modules(path, source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        assert!(!modules.is_empty(), "expected at least one parsed module");

        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);

        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(
            errors.is_empty(),
            "unexpected unknown-name errors: {errors:#?}"
        );
        // Verify no other errors either
        let all_errors: Vec<_> = diags
            .iter()
            .filter(|d| {
                d.path == "test.aivi"
                    && d.diagnostic.severity == crate::DiagnosticSeverity::Error
            })
            .collect();
        assert!(
            all_errors.is_empty(),
            "unexpected errors in test module: {all_errors:#?}"
        );
    }

    #[test]
    fn module_aliasing_handles_call_and_index_syntax() {
        let source = r#"
module test.db_alias_syntax
use aivi.database as db

User = { id: Int, name: Text }
userTable = db.table "users"[]

main = do Effect {
  _ <- db.configure { driver: db.Sqlite, url: ":memory:" }
  _ <- db.runMigrations[userTable]
  _ <- userTable + db.ins { id: 1, name: "Alice" }
  db.load userTable
}
"#;

        let path = std::path::Path::new("test.aivi");
        let (mut modules, diags) = crate::surface::parse_modules(path, source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        assert!(!modules.is_empty(), "expected at least one parsed module");

        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);

        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(
            errors.is_empty(),
            "unexpected unknown-name errors: {errors:#?}"
        );
        // Verify no other errors either
        let all_errors: Vec<_> = diags
            .iter()
            .filter(|d| {
                d.path == "test.aivi"
                    && d.diagnostic.severity == crate::DiagnosticSeverity::Error
            })
            .collect();
        assert!(
            all_errors.is_empty(),
            "unexpected errors in test module: {all_errors:#?}"
        );
    }

    #[test]
    fn gtk4_native_record_is_resolved_as_builtin() {
        let source = r#"
module test.gtk_builtin
use aivi.ui.gtk4

x = gtk4.appRun
"#;

        let path = std::path::Path::new("test.aivi");
        let (mut modules, diags) = crate::surface::parse_modules(path, source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        assert!(!modules.is_empty(), "expected at least one parsed module");

        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);

        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(
            errors.is_empty(),
            "unexpected unknown-name errors: {errors:#?}"
        );
        // Verify no other errors for the test module
        let all_errors: Vec<_> = diags
            .iter()
            .filter(|d| {
                d.path == "test.aivi"
                    && d.diagnostic.severity == crate::DiagnosticSeverity::Error
            })
            .collect();
        assert!(
            all_errors.is_empty(),
            "unexpected errors in test module: {all_errors:#?}"
        );
    }

    #[test]
    fn debug_unknown_param_is_error() {
        let source = r#"
module test.debug_params

@debug(pipes, nope, time)
f = x => x
"#;
        let (modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let diags = check_modules(&modules);
        assert!(
            diags.iter().any(|d| d.diagnostic.code == "E2012"),
            "expected E2012, got: {diags:?}"
        );
    }

    #[test]
    fn debug_non_identifier_param_is_error() {
        let source = r#"
module test.debug_params

@debug(pipes, 1, time)
f = x => x
"#;
        let (modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let diags = check_modules(&modules);
        assert!(
            diags.iter().any(|d| d.diagnostic.code == "E2011"),
            "expected E2011, got: {diags:?}"
        );
    }

    #[test]
    fn debug_requires_function_binding() {
        let source = r#"
module test.debug_params

@debug()
x = 1
"#;
        let (modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let diags = check_modules(&modules);
        assert!(
            diags.iter().any(|d| d.diagnostic.code == "E2010"),
            "expected E2010, got: {diags:?}"
        );
    }

    #[test]
    fn warns_on_unused_imports_and_private_bindings() {
        let source = r#"
module test.unused

use aivi.console (print)

x = 1
"#;
        let (mut modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");

        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);

        let codes: Vec<_> = diags
            .iter()
            .filter(|d| d.path == "test.aivi")
            .map(|d| d.diagnostic.code.as_str())
            .collect();
        assert!(codes.contains(&"W2100"), "expected W2100, got: {codes:?}");
        assert!(codes.contains(&"W2101"), "expected W2101, got: {codes:?}");
    }

    #[test]
    fn does_not_warn_for_domain_import_used_via_operators() {
        let source = r#"
module test.domain_import

// Domain imports can be used implicitly (operators/suffix literals), so the resolver must not warn.
use aivi.duration (domain Duration)

// Reference a suffix literal so this test continues to exercise "implicit domain usage"
// paths, but without requiring the imported domain name to appear as an identifier.
x = 30s
"#;
        let (modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let diags = check_modules(&modules);
        assert!(
            !diags
                .iter()
                .any(|d| d.path == "test.aivi" && d.diagnostic.code == "W2100"),
            "expected no unused-import warnings for domain import, got: {diags:?}"
        );
    }

    #[test]
    fn predicate_call_arguments_allow_bare_field_names() {
        let source = r#"
module test.predicate_calls
use aivi.list
use aivi.logic
use aivi.database
use aivi.database (domain Database)

rows = [{ id: 1 }, { id: 2 }]
out = filter (id == 1) rows

Product = { id: Int, active: Bool }
productTable : Table Product
productTable = table "products" [
  { name: "id", type: IntType, constraints: [], default: None }
  { name: "active", type: BoolType, constraints: [], default: None }
]
query = where active (from productTable)

tableRows = table "rows"[]
updated = tableRows + upd (id == 1) (row => row)
"#;

        let path = std::path::Path::new("test.aivi");
        let (mut modules, diags) = crate::surface::parse_modules(path, source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");

        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);

        let unknown_name_errors: Vec<_> = diags
            .into_iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(
            unknown_name_errors.is_empty(),
            "expected no unknown-name errors for predicate-position bare fields, got {unknown_name_errors:#?}"
        );
    }

    #[test]
    fn pipe_right_hand_sides_allow_bare_field_names() {
        let source = r#"
module test.pipe_rhs
use aivi.reactive

next = { count: 1 } |> count + 1

derived = do Effect {
  state = signal { count: 1 }
  nextSignal = state ->> count + 1
  pure nextSignal
}
"#;

        let path = std::path::Path::new("test.aivi");
        let (mut modules, diags) = crate::surface::parse_modules(path, source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");

        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);

        let unknown_name_errors: Vec<_> = diags
            .into_iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(
            unknown_name_errors.is_empty(),
            "expected no unknown-name errors for pipe-position bare fields, got {unknown_name_errors:#?}"
        );
    }

    // ---- resolver/debug_and_unused.rs: additional tests ----

    #[test]
    fn resolver_detects_unknown_name() {
        let source = r#"
module test.unknown
x = unknownFunc 42
"#;
        let (mut modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);
        assert!(
            diags.iter().any(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005"),
            "expected E2005 for unknown name, got: {:?}",
            diags.iter().filter(|d| d.path == "test.aivi").collect::<Vec<_>>()
        );
    }

    #[test]
    fn resolver_lambda_params_are_in_scope() {
        let source = r#"
module test.lambda_scope

f = x => y => x + y
"#;
        let (mut modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(errors.is_empty(), "unexpected unknown-name errors: {errors:?}");
    }

    #[test]
    fn resolver_match_pattern_binders_in_scope() {
        let source = r#"
module test.match_scope

f = opt =>
  opt match
    | Some x => x
    | None => 0
"#;
        let (mut modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(errors.is_empty(), "unexpected unknown-name errors: {errors:?}");
    }

    #[test]
    fn resolver_block_let_binding_in_scope() {
        let source = r#"
module test.block_scope

f = do Effect {
  x <- pure 1
  y = 2
  pure (x + y)
}
"#;
        let (mut modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(errors.is_empty(), "unexpected unknown-name errors: {errors:?}");
    }

    #[test]
    fn resolver_constructor_names_always_valid() {
        let source = r#"
module test.constructors

x = Some 42
y = None
z = True
w = False
"#;
        let (mut modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(errors.is_empty(), "unexpected unknown-name errors: {errors:?}");
    }

    #[test]
    fn resolver_as_pattern_binder_in_scope() {
        let source = r#"
module test.as_pattern

f = all as (Some _) => all
f = None => None
"#;
        let (mut modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(errors.is_empty(), "unexpected unknown-name errors: {errors:?}");
    }

    #[test]
    fn resolver_list_pattern_rest_in_scope() {
        let source = r#"
module test.list_pattern

tail = [_, ...rest] => rest
tail = _ => []
"#;
        let (mut modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(errors.is_empty(), "unexpected unknown-name errors: {errors:?}");
    }

    #[test]
    fn resolver_special_unknown_name_messages() {
        let source = r#"
module test.special_names

x = return
"#;
        let (mut modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(
            !errors.is_empty(),
            "expected error for 'return' keyword"
        );
        assert!(
            errors.iter().any(|d| d.diagnostic.message.contains("AIVI has no `return`")),
            "expected special message for 'return', got: {errors:?}"
        );
    }

    #[test]
    fn resolver_special_message_for_null() {
        let source = r#"
module test.null_name

x = null
"#;
        let (mut modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(
            !errors.is_empty(),
            "expected error for 'null'"
        );
        assert!(
            errors.iter().any(|d| d.diagnostic.message.contains("AIVI has no nulls")),
            "expected special message for 'null', got: {errors:?}"
        );
    }

    #[test]
    fn resolver_special_message_for_loop_keywords() {
        let source = r#"
module test.loop_name

x = while
"#;
        let (mut modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(
            !errors.is_empty(),
            "expected error for 'while'"
        );
        assert!(
            errors.iter().any(|d| d.diagnostic.message.contains("AIVI has no loops")),
            "expected special message for 'while', got: {errors:?}"
        );
    }

    #[test]
    fn resolver_tuple_pattern_binders_in_scope() {
        let source = r#"
module test.tuple_scope

fst = (a, _) => a
snd = (_, b) => b
"#;
        let (mut modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(errors.is_empty(), "unexpected unknown-name errors: {errors:?}");
    }

    #[test]
    fn resolver_guard_in_match_scope() {
        let source = r#"
module test.guard_scope

classify = x =>
  x match
    | n when n > 0 => "positive"
    | 0 => "zero"
    | _ => "negative"
"#;
        let (mut modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(errors.is_empty(), "unexpected unknown-name errors: {errors:?}");
    }

    #[test]
    fn debug_decorator_on_lambda_def_fires_e2010() {
        let source = r#"
module test.debug_lambda

@debug(pipes, args, return, time)
f = x => x + 1
"#;
        let (modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let diags = check_modules(&modules);
        // Lambda-style defs have empty params, so E2010 fires
        assert!(
            diags.iter().any(|d| d.diagnostic.code == "E2010"),
            "expected E2010 for lambda-style function, got: {diags:?}"
        );
    }

    #[test]
    fn debug_decorator_unknown_param_still_fires_e2012() {
        let source = r#"
module test.debug_e2012

@debug(pipes, nope)
f = x => x
"#;
        let (modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let diags = check_modules(&modules);
        assert!(
            diags.iter().any(|d| d.diagnostic.code == "E2012"),
            "expected E2012 for unknown debug param, got: {diags:?}"
        );
    }
}
