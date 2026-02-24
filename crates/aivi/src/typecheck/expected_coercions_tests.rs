use std::path::Path;

use crate::diagnostics::DiagnosticSeverity;
use crate::hir::HirExpr;
use crate::surface::{Expr, ModuleItem, PathSegment};

fn without_embedded(
    diags: Vec<crate::diagnostics::FileDiagnostic>,
) -> Vec<crate::diagnostics::FileDiagnostic> {
    diags
        .into_iter()
        .filter(|diag| !diag.path.starts_with("<embedded:"))
        .collect()
}

fn without_embedded_errors(
    diags: Vec<crate::diagnostics::FileDiagnostic>,
) -> Vec<crate::diagnostics::FileDiagnostic> {
    without_embedded(diags)
        .into_iter()
        .filter(|diag| diag.diagnostic.severity == DiagnosticSeverity::Error)
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

fn find_def_expr<'a>(module: &'a crate::surface::Module, def_name: &str) -> &'a Expr {
    module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == def_name => Some(&def.expr),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected def '{def_name}'"))
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

    let diags = without_embedded_errors(crate::resolver::check_modules(&all_modules));
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let diags = without_embedded_errors(crate::typecheck::elaborate_expected_coercions(
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

    let diags = without_embedded_errors(crate::resolver::check_modules(&all_modules));
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let diags = without_embedded_errors(crate::typecheck::elaborate_expected_coercions(
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

#[test]
fn fills_record_defaults_for_enabled_builtin_markers() {
    let source = r#"
module test.defaults_builtin
use aivi.defaults (Option, List, Bool)

mk : { name: Text, nick: Option Text, tags: List Text, active: Bool } -> { name: Text, nick: Option Text, tags: List Text, active: Bool }
mk = rec => rec

x = mk { name: "Ada" }
"#;

    let (mut modules, diags) = crate::surface::parse_modules(Path::new("test.aivi"), source);
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let mut all_modules = crate::stdlib::embedded_stdlib_modules();
    all_modules.append(&mut modules);

    let diags = without_embedded_errors(crate::resolver::check_modules(&all_modules));
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let diags = without_embedded_errors(crate::typecheck::elaborate_expected_coercions(
        &mut all_modules,
    ));
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let module = all_modules
        .iter()
        .find(|m| m.name.name == "test.defaults_builtin")
        .expect("expected test.defaults_builtin module");
    let x_expr = find_def_expr(module, "x");
    let Expr::Call { args, .. } = x_expr else {
        panic!("expected call expression for x");
    };
    let Expr::Record { fields, .. } = &args[0] else {
        panic!("expected record argument in x call");
    };

    assert_eq!(
        fields.len(),
        4,
        "expected synthesized defaults plus explicit field"
    );

    let mut saw_nick = false;
    let mut saw_tags = false;
    let mut saw_active = false;
    let mut saw_name = false;
    for field in fields {
        let Some(PathSegment::Field(name)) = field.path.first() else {
            continue;
        };
        match name.name.as_str() {
            "nick" => {
                saw_nick = true;
                assert!(matches!(&field.value, Expr::Ident(value) if value.name == "None"));
            }
            "tags" => {
                saw_tags = true;
                assert!(matches!(&field.value, Expr::List { items, .. } if items.is_empty()));
            }
            "active" => {
                saw_active = true;
                assert!(matches!(
                    &field.value,
                    Expr::Literal(crate::surface::Literal::Bool { value: false, .. })
                ));
            }
            "name" => {
                saw_name = true;
                assert!(matches!(
                    &field.value,
                    Expr::Literal(crate::surface::Literal::String { text, .. }) if text == "Ada"
                ));
            }
            _ => {}
        }
    }
    assert!(
        saw_name && saw_nick && saw_tags && saw_active,
        "missing synthesized defaults"
    );
}

#[test]
fn fills_record_defaults_via_todefault_class_marker() {
    let source = r#"
module test.defaults_typeclass
use aivi.defaults (ToDefault)

mk : { id: Int, label: Text } -> { id: Int, label: Text }
mk = rec => rec

x = mk { id: 1 }
"#;

    let (mut modules, diags) = crate::surface::parse_modules(Path::new("test.aivi"), source);
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let mut all_modules = crate::stdlib::embedded_stdlib_modules();
    all_modules.append(&mut modules);

    let diags = without_embedded_errors(crate::resolver::check_modules(&all_modules));
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let diags = without_embedded_errors(crate::typecheck::elaborate_expected_coercions(
        &mut all_modules,
    ));
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let module = all_modules
        .iter()
        .find(|m| m.name.name == "test.defaults_typeclass")
        .expect("expected test.defaults_typeclass module");
    let x_expr = find_def_expr(module, "x");
    let Expr::Call { args, .. } = x_expr else {
        panic!("expected call expression for x");
    };
    let Expr::Record { fields, .. } = &args[0] else {
        panic!("expected record argument in x call");
    };
    assert_eq!(
        fields.len(),
        2,
        "expected one synthesized and one explicit field"
    );

    let mut saw_label_default = false;
    let mut saw_id = false;
    for field in fields {
        let Some(PathSegment::Field(name)) = field.path.first() else {
            continue;
        };
        match name.name.as_str() {
            "label" => {
                saw_label_default = true;
                assert!(matches!(
                    &field.value,
                    Expr::Call { func, args, .. }
                    if args.is_empty()
                        && matches!(func.as_ref(), Expr::Ident(name) if name.name == "toDefault")
                ));
            }
            "id" => {
                saw_id = true;
                assert!(matches!(
                    &field.value,
                    Expr::Literal(crate::surface::Literal::Number { text, .. }) if text == "1"
                ));
            }
            _ => {}
        }
    }
    assert!(
        saw_label_default && saw_id,
        "expected both id and defaulted label fields"
    );
}
