
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
            for arg in args {
                check_expr(arg, scope, diagnostics, module, allow_unknown);
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
        Expr::Binary { left, right, .. } => {
            check_expr(left, scope, diagnostics, module, allow_unknown);
            check_expr(right, scope, diagnostics, module, allow_unknown);
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
                        check_expr(expr, &mut block_scope, diagnostics, module, allow_unknown);
                        collect_pattern_binding(pattern, &mut block_scope);
                    }
                    BlockItem::Filter { expr, .. }
                    | BlockItem::Yield { expr, .. }
                    | BlockItem::Recurse { expr, .. }
                    | BlockItem::Expr { expr, .. } => {
                        check_expr(expr, &mut block_scope, diagnostics, module, allow_unknown);
                    }
                    BlockItem::When { cond, effect, .. } => {
                        check_expr(cond, &mut block_scope, diagnostics, module, allow_unknown);
                        check_expr(effect, &mut block_scope, diagnostics, module, allow_unknown);
                    }
                    BlockItem::Given { cond, fail_expr, .. } => {
                        check_expr(cond, &mut block_scope, diagnostics, module, allow_unknown);
                        check_expr(fail_expr, &mut block_scope, diagnostics, module, allow_unknown);
                    }
                    BlockItem::On { transition, handler, .. } => {
                        check_expr(transition, &mut block_scope, diagnostics, module, allow_unknown);
                        check_expr(handler, &mut block_scope, diagnostics, module, allow_unknown);
                    }
                }
            }
        }
        Expr::Raw { .. } => {}
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
    matches!(
        name,
        "print"
            | "println"
            | "attempt"
            | "load"
            | "file"
            | "env"
            | "pure"
            | "fail"
            | "Unit"
            | "Text"
            | "Char"
            | "Int"
            | "Float"
            | "Bool"
            | "Bytes"
            | "List"
            | "Effect"
            | "Stream"
            | "Listener"
            | "Connection"
            | "Some"
            | "None"
            | "Ok"
            | "Err"
            | "True"
            | "False"
            | "Map"
            | "Set"
            | "Queue"
            | "Deque"
            | "Heap"
            | "text"
            | "regex"
            | "math"
            | "calendar"
            | "color"
            | "bigint"
            | "rational"
            | "decimal"
            | "url"
            | "system"
            | "logger"
            | "database"
            | "http"
            | "https"
            | "collections"
            | "linalg"
            | "signal"
            | "graph"
            | "console"
            | "crypto"
            | "clock"
            | "random"
            | "channel"
            | "concurrent"
            | "httpServer"
            | "ui"
            | "sockets"
            | "streams"
            | "i18n"
    )
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

        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);

        let errors: Vec<_> = diags
            .into_iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(errors.is_empty(), "unexpected unknown-name errors: {errors:#?}");
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

        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);

        let errors: Vec<_> = diags
            .into_iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(errors.is_empty(), "unexpected unknown-name errors: {errors:#?}");
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
}
