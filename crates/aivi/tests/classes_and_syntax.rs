use std::path::Path;

use aivi::{check_types, parse_modules};

#[test]
fn class_inheritance_uses_type_and_combinator() {
    let src = r#"
module Test
export Functor, Monad

class Functor (F *) = {
  map: F A -> (A -> B) -> F B
}

class Monad (M *) =
  Functor {
    pure: A -> M A
    flatMap: M A -> (A -> M B) -> M B
  }
"#;

    let (modules, parse_diags) = parse_modules(Path::new("classes_and_syntax.aivi"), src);
    assert!(
        parse_diags.is_empty(),
        "unexpected parse diagnostics: {parse_diags:#?}"
    );

    let type_diags = check_types(&modules);
    assert!(
        type_diags.is_empty(),
        "unexpected type diagnostics: {type_diags:#?}"
    );
}

#[test]
fn instance_inherited_methods_can_delegate_to_super_instance() {
    let src = r#"
module Test

class Super A = {
  foo: A -> A
}

class Sub A = Super {
  bar: A -> A
}

instance Sub Int = {
  bar: x => x
}

instance Super Int = {
  foo: x => x
}
"#;

    let (modules, parse_diags) = parse_modules(Path::new("instance_extends_ok.aivi"), src);
    assert!(
        parse_diags.is_empty(),
        "unexpected parse diagnostics: {parse_diags:#?}"
    );

    let type_diags = check_types(&modules);
    assert!(
        type_diags.is_empty(),
        "unexpected type diagnostics: {type_diags:#?}"
    );
}

#[test]
fn instance_inherited_methods_error_without_super_instance() {
    let src = r#"
module Test

class Super A = {
  foo: A -> A
}

class Sub A = Super {
  bar: A -> A
}

instance Sub Int = {
  bar: x => x
}
"#;

    let (modules, parse_diags) = parse_modules(Path::new("instance_extends_err.aivi"), src);
    assert!(
        parse_diags.is_empty(),
        "unexpected parse diagnostics: {parse_diags:#?}"
    );

    let type_diags = check_types(&modules);
    assert!(
        type_diags.iter().any(|d| d
            .diagnostic
            .message
            .contains("missing instance method 'foo'")),
        "expected missing 'foo' diagnostic, got: {type_diags:#?}"
    );
}
