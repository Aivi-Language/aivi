use std::path::Path;

use crate::hir::HirExpr;

fn without_embedded(
    diags: Vec<crate::diagnostics::FileDiagnostic>,
) -> Vec<crate::diagnostics::FileDiagnostic> {
    diags
        .into_iter()
        .filter(|diag| !diag.path.starts_with("<embedded:"))
        .collect()
}

fn hir_contains_var(expr: &HirExpr, name: &str) -> bool {
    match expr {
        HirExpr::Var { name: n, .. } => n == name,
        HirExpr::Lambda { body, .. } => hir_contains_var(body, name),
        HirExpr::App { func, arg, .. } => {
            hir_contains_var(func, name) || hir_contains_var(arg, name)
        }
        HirExpr::Call { func, args, .. } => {
            hir_contains_var(func, name) || args.iter().any(|arg| hir_contains_var(arg, name))
        }
        HirExpr::DebugFn { arg_vars, body, .. } => {
            arg_vars.iter().any(|v| v == name) || hir_contains_var(body, name)
        }
        HirExpr::Pipe { func, arg, .. } => {
            hir_contains_var(func, name) || hir_contains_var(arg, name)
        }
        HirExpr::TextInterpolate { parts, .. } => parts.iter().any(|part| match part {
            crate::hir::HirTextPart::Text { .. } => false,
            crate::hir::HirTextPart::Expr { expr } => hir_contains_var(expr, name),
        }),
        HirExpr::List { items, .. } => items.iter().any(|item| hir_contains_var(&item.expr, name)),
        HirExpr::Tuple { items, .. } => items.iter().any(|item| hir_contains_var(item, name)),
        HirExpr::Record { fields, .. } => fields
            .iter()
            .any(|field| hir_contains_var(&field.value, name)),
        HirExpr::Patch { target, fields, .. } => {
            hir_contains_var(target, name)
                || fields
                    .iter()
                    .any(|field| hir_contains_var(&field.value, name))
        }
        HirExpr::FieldAccess { base, .. } => hir_contains_var(base, name),
        HirExpr::Index { base, index, .. } => {
            hir_contains_var(base, name) || hir_contains_var(index, name)
        }
        HirExpr::Binary { left, right, .. } => {
            hir_contains_var(left, name) || hir_contains_var(right, name)
        }
        HirExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            hir_contains_var(cond, name)
                || hir_contains_var(then_branch, name)
                || hir_contains_var(else_branch, name)
        }
        HirExpr::Match {
            scrutinee, arms, ..
        } => {
            hir_contains_var(scrutinee, name)
                || arms.iter().any(|arm| {
                    hir_contains_var(&arm.body, name)
                        || arm
                            .guard
                            .as_ref()
                            .is_some_and(|g| hir_contains_var(g, name))
                })
        }
        HirExpr::Block { items, .. } => items.iter().any(|item| match item {
            crate::hir::HirBlockItem::Bind { expr, .. } => hir_contains_var(expr, name),
            crate::hir::HirBlockItem::Filter { expr } => hir_contains_var(expr, name),
            crate::hir::HirBlockItem::Yield { expr } => hir_contains_var(expr, name),
            crate::hir::HirBlockItem::Recurse { expr } => hir_contains_var(expr, name),
            crate::hir::HirBlockItem::Expr { expr } => hir_contains_var(expr, name),
        }),
        HirExpr::Raw { .. }
        | HirExpr::LitNumber { .. }
        | HirExpr::LitString { .. }
        | HirExpr::LitSigil { .. }
        | HirExpr::LitBool { .. }
        | HirExpr::LitDateTime { .. } => false,
    }
}

#[test]
fn inserts_to_text_for_record_when_text_expected() {
    let source = r#"
module test.coerce

needsText : Text -> Int
needsText = x => text.length x

x = needsText { name: "A" }
"#;

    let (mut modules, diags) = crate::surface::parse_modules(Path::new("test.aivi"), source);
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let mut all_modules = crate::stdlib::embedded_stdlib_modules();
    all_modules.append(&mut modules);

    let diags = without_embedded(crate::resolver::check_modules(&all_modules));
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let diags = without_embedded(crate::typecheck::elaborate_expected_coercions(
        &mut all_modules,
    ));
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let program = crate::hir::desugar_modules(&all_modules);
    let module = program
        .modules
        .iter()
        .find(|m| m.name == "test.coerce")
        .expect("expected test.coerce module");
    let x_def = module
        .defs
        .iter()
        .find(|d| d.name == "x")
        .expect("expected x def");

    assert!(
        hir_contains_var(&x_def.expr, "toText"),
        "expected elaboration to insert a `toText` call"
    );
}

#[test]
fn inserts_to_text_for_int_when_text_expected() {
    let source = r#"
module test.no_coerce

needsText : Text -> Int
needsText = x => text.length x

x = needsText 123
"#;

    let (mut modules, diags) = crate::surface::parse_modules(Path::new("test.aivi"), source);
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let mut all_modules = crate::stdlib::embedded_stdlib_modules();
    all_modules.append(&mut modules);

    let diags = without_embedded(crate::resolver::check_modules(&all_modules));
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let diags = without_embedded(crate::typecheck::elaborate_expected_coercions(
        &mut all_modules,
    ));
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let program = crate::hir::desugar_modules(&all_modules);
    let module = program
        .modules
        .iter()
        .find(|m| m.name == "test.no_coerce")
        .expect("expected test.no_coerce module");
    let x_def = module
        .defs
        .iter()
        .find(|d| d.name == "x")
        .expect("expected x def");

    assert!(
        hir_contains_var(&x_def.expr, "toText"),
        "expected elaboration to insert a `toText` call"
    );
}
