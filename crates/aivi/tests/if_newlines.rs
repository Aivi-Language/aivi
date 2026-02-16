use std::path::Path;

use aivi::{parse_modules, DiagnosticSeverity, Expr, ModuleItem};

#[test]
fn parses_if_with_newlines_before_else() {
    let src = r#"
module tmp
export f

f = x =>
  if x == 0 then
    1
  else
    2
"#;

    let (modules, diags) = parse_modules(Path::new("<test>"), src);
    assert!(
        diags
            .iter()
            .all(|d| d.diagnostic.severity != DiagnosticSeverity::Error),
        "unexpected parse diagnostics: {diags:?}"
    );

    let module = modules
        .iter()
        .find(|m| m.name.name == "tmp")
        .expect("module tmp");

    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "f" => Some(def),
            _ => None,
        })
        .expect("def f");

    let Expr::Lambda { body, .. } = &def.expr else {
        panic!("expected f to be a lambda, got: {:?}", def.expr);
    };

    assert!(
        matches!(body.as_ref(), Expr::If { .. }),
        "expected lambda body to be an if expression, got: {body:?}"
    );
}

