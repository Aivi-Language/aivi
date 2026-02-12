use std::path::Path;

use crate::surface::{parse_modules, Expr, Literal, ModuleItem};

fn diag_codes(diags: &[crate::FileDiagnostic]) -> Vec<String> {
    let mut codes: Vec<String> = diags.iter().map(|d| d.diagnostic.code.clone()).collect();
    codes.sort();
    codes
}

#[test]
fn parses_decorator_with_argument_on_def() {
    let src = r#"
module Example

@deprecated "use `y` instead"
x = 1
"#;

    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    assert_eq!(def.decorators.len(), 1);
    assert_eq!(def.decorators[0].name.name, "deprecated");
    assert!(
        matches!(
            def.decorators[0].arg,
            Some(Expr::Literal(Literal::String { .. }))
        ),
        "expected @deprecated string literal argument"
    );
}

#[test]
fn rejects_unknown_item_decorator() {
    let src = r#"
module Example

@sql
x = 1
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1506".to_string()));
}

#[test]
fn rejects_deprecated_without_argument() {
    let src = r#"
module Example

@deprecated
x = 1
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1511".to_string()));
}

#[test]
fn rejects_argument_on_inline() {
    let src = r#"
module Example

@inline "nope"
f x = x
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1513".to_string()));
}

#[test]
fn module_decorator_no_prelude_rejects_argument() {
    let src = r#"
@no_prelude "nope"
module Example
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1512".to_string()));
}

#[test]
fn rejects_result_or_success_arms() {
    let src = r#"
module Example

Result E A = Err E | Ok A

value = (Ok 1) or
  | Ok x  => x
  | Err _ => 0
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1530".to_string()));
}
