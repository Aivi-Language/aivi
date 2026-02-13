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
fn parses_structured_sigil_map_literal() {
    let src = r#"
module Example

x = ~map{ "a" => 1 }
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

    assert!(
        !matches!(&def.expr, Expr::Literal(Literal::Sigil { tag, .. }) if tag == "map"),
        "expected ~map{{...}} to parse as a structured literal, not a sigil literal"
    );
}

#[test]
fn parses_decorator_on_class_decl() {
    let src = r#"
module Example

@inline
class Functor (F *) = { map: (A -> B) -> F A -> F B }
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let class_decl = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::ClassDecl(class_decl) if class_decl.name.name == "Functor" => {
                Some(class_decl)
            }
            _ => None,
        })
        .expect("Functor class decl");

    assert_eq!(class_decl.decorators.len(), 1);
    assert_eq!(class_decl.decorators[0].name.name, "inline");
}

#[test]
fn parses_named_instance_decl() {
    let src = r#"
module Example

instance MyFunctor : Functor (Option *) = {
  map: f opt => opt
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let instance_decl = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::InstanceDecl(instance_decl) => Some(instance_decl),
            _ => None,
        })
        .expect("instance decl");

    assert_eq!(instance_decl.name.name, "Functor");
    assert_eq!(
        instance_decl.label.as_ref().map(|name| name.name.as_str()),
        Some("MyFunctor")
    );
}

#[test]
fn rejects_multiple_modules_per_file() {
    let src = r#"
module A = {
  x = 1
}

module B = {
  y = 2
}
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1516".to_string()));
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
