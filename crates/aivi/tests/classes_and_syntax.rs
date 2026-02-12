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
  Functor (M *) & {
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

