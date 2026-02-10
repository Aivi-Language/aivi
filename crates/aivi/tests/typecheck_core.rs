use std::path::Path;

use aivi::{check_modules, check_types, parse_modules};

fn check_ok(source: &str) {
    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(diagnostics.is_empty(), "parse diagnostics: {diagnostics:?}");

    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(module_diags.is_empty(), "type diagnostics: {module_diags:?}");
}

fn check_err(source: &str) {
    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(diagnostics.is_empty(), "parse diagnostics: {diagnostics:?}");

    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(!module_diags.is_empty(), "expected diagnostics");
}

#[test]
fn typecheck_effects_resources() {
    let source = r#"
module test.core = {
  export main

  main : Effect Text Unit
  main = effect {
    f <- resource {
      handle <- file.open "Cargo.toml"
      yield handle
      _ <- file.close handle
    }
    _ <- file.readAll f
    _ <- print "ok"
    pure Unit
  }
}
"#;
    check_ok(source);
}

#[test]
fn typecheck_domains_patching() {
    let source = r#"
module test.m7 = {
  export addWeek, updated

  Date = { year: Int, month: Int, day: Int }

  domain Calendar over Date = {
    type Delta = Day Int | Week Int

    (+) : Date -> Delta -> Date
    (+) d delta = delta ?
      | Day n => addDays d n
      | Week n => addDays d (n * 7)

    1w = Week 1
  }

  addDays : Date -> Int -> Date
  addDays d n = d <| { day: _ + n }

  addWeek : Date -> Date
  addWeek d = d + 2w

  updated = addWeek { year: 2024, month: 9, day: 1 }
}
"#;
    check_ok(source);
}

#[test]
fn typecheck_domain_operator_overload_without_deltas() {
    let source = r#"
module test.domain_ops = {
  export move

  Vec2 = { x: Int, y: Int }

  domain Vector over Vec2 = {
    (+) : Vec2 -> Vec2 -> Vec2
    (+) a b = { x: a.x + b.x, y: a.y + b.y }
  }

  move : Vec2 -> Vec2 -> Vec2
  move pos vel = pos + vel
}
"#;
    check_ok(source);
}

#[test]
fn typecheck_error_unknown_numeric_delta_literal() {
    let source = r#"
module test.delta_err = {
  export value
  value = 2w
}
"#;
    check_err(source);
}

#[test]
fn typecheck_error_effect_final() {
    let source = r#"
module test.err = {
  export main

  main : Effect Text Unit
  main = effect {
    1
  }
}
"#;
    check_err(source);
}

#[test]
fn typecheck_error_unknown_name() {
    let source = r#"
module test.err = {
  export value
  value = missing
}
"#;
    check_err(source);
}

#[test]
fn typecheck_open_records_allow_extra_fields() {
    let source = r#"
module test.open = {
  export value

  getName : { name: Text } -> Text
  getName user = user.name

  value = getName { name: "Alice", id: 1 }
}
"#;
    check_ok(source);
}
