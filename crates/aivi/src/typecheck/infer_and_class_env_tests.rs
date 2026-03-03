use std::path::Path;

use crate::surface::parse_modules;
use crate::typecheck::{check_types, infer_value_types, infer_value_types_full};

fn has_errors(diags: &[crate::diagnostics::FileDiagnostic]) -> bool {
    diags
        .iter()
        .any(|d| d.diagnostic.severity == crate::diagnostics::DiagnosticSeverity::Error)
}

fn parse_and_infer(src: &str) -> crate::typecheck::InferResult {
    let (modules, parse_diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        !has_errors(&parse_diags),
        "unexpected parse errors: {parse_diags:?}"
    );
    infer_value_types_full(&modules)
}

fn parse_and_check(src: &str) -> Vec<crate::diagnostics::FileDiagnostic> {
    let (modules, parse_diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        !has_errors(&parse_diags),
        "unexpected parse errors: {parse_diags:?}"
    );
    check_types(&modules)
        .into_iter()
        .filter(|d| !d.path.starts_with("<embedded:"))
        .collect()
}

// ---- infer.rs: basic type inference ----

#[test]
fn infer_simple_function_types() {
    let result = parse_and_infer(
        r#"
module Test

add : Int -> Int -> Int
add = a => b => a + b

identity : A -> A
identity = x => x

constant = 42
"#,
    );
    assert!(
        !has_errors(&result.diagnostics),
        "unexpected errors: {:?}",
        result.diagnostics
    );
    let types = result.type_strings.get("Test").expect("Test module types");
    assert!(types.contains_key("add"), "expected 'add' in types");
    assert!(types.contains_key("identity"), "expected 'identity' in types");
    assert!(types.contains_key("constant"), "expected 'constant' in types");
}

#[test]
fn infer_result_has_span_types() {
    let result = parse_and_infer(
        r#"
module Test
x = 1 + 2
"#,
    );
    assert!(!has_errors(&result.diagnostics));
    let _ = result.span_types;
}

#[test]
fn infer_value_types_tuple_convenience_form() {
    let (modules, _) = parse_modules(
        Path::new("test.aivi"),
        r#"
module Test
f = x => (x, x + 1)
"#,
    );
    let (diags, type_strings, span_types) = infer_value_types(&modules);
    let non_embedded: Vec<_> = diags
        .into_iter()
        .filter(|d| !d.path.starts_with("<embedded:"))
        .collect();
    assert!(!has_errors(&non_embedded));
    assert!(type_strings.contains_key("Test"));
    let _ = span_types;
}

#[test]
fn infer_multiple_modules() {
    let src = r#"
module Test

double : Int -> Int
double = x => x * 2

result = double 21
"#;
    let (modules, parse_diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(!has_errors(&parse_diags));
    let result = infer_value_types_full(&modules);
    let non_embedded: Vec<_> = result
        .diagnostics
        .into_iter()
        .filter(|d| !d.path.starts_with("<embedded:"))
        .collect();
    assert!(!has_errors(&non_embedded));
}

#[test]
fn infer_records_and_field_access() {
    let result = parse_and_infer(
        r#"
module Test

getName : { name: Text } -> Text
getName = rec => rec.name

person = { name: "Alice", age: 30 }
"#,
    );
    assert!(!has_errors(
        &result
            .diagnostics
            .iter()
            .filter(|d| !d.path.starts_with("<embedded:"))
            .cloned()
            .collect::<Vec<_>>()
    ));
}

#[test]
fn infer_option_type() {
    let result = parse_and_infer(
        r#"
module Test

findFirst : (A -> Bool) -> List A -> Option A
findFirst = pred => lst =>
  lst match
    | [] => None
    | [x, ...rest] => if pred x then Some x else findFirst pred rest
"#,
    );
    let non_embedded: Vec<_> = result
        .diagnostics
        .into_iter()
        .filter(|d| !d.path.starts_with("<embedded:"))
        .collect();
    assert!(!has_errors(&non_embedded));
}

#[test]
fn infer_detects_type_mismatch() {
    let src = r#"
module Test

bad : Int -> Text
bad = x => x
"#;
    let diags = parse_and_check(src);
    assert!(
        has_errors(&diags),
        "expected type error for Int->Text mismatch, got: {diags:?}"
    );
}

// ---- class_env.rs: class environment construction ----

#[test]
fn class_env_simple_class_and_instance() {
    let diags = parse_and_check(
        r#"
module Test

class Show A = {
  show: A -> Text
}

instance Show Int = {
  show: x => "int"
}

printIt = x => show x
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn class_env_superclass_inheritance() {
    let diags = parse_and_check(
        r#"
module Test

class Eq A = {
  eq: A -> A -> Bool
}

class Ord A = given (A: Eq) {
  lt: A -> A -> Bool
}

instance Eq Int = {
  eq: a => b => a == b
}

instance Ord Int = {
  lt: a => b => a < b
}

sortable = a => b => lt a b
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn class_env_hkt_class() {
    let diags = parse_and_check(
        r#"
module Test

class Functor (F A) = {
  fmap: (A -> B) -> F A -> F B
}

instance Functor (Option A) = {
  fmap: f => opt =>
    opt ?
      | None => None
      | Some x => Some (f x)
}
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn class_env_multiple_class_constraints() {
    let diags = parse_and_check(
        r#"
module Test

class Eq A = {
  eq: A -> A -> Bool
}

class Show A = {
  show: A -> Text
}

instance Eq Int = {
  eq: a => b => a == b
}

instance Show Int = {
  show: a => "int"
}

printIfEqual = a => b =>
  if eq a b then show a else "not equal"
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn class_env_exported_class_is_usable() {
    let src = r#"
module Test

class Printable A = {
  prettyPrint: A -> Text
}

instance Printable Int = {
  prettyPrint = n => "num"
}

display = x => prettyPrint x
"#;
    let (modules, parse_diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(!has_errors(&parse_diags));
    let type_diags = check_types(&modules);
    let non_embedded: Vec<_> = type_diags
        .into_iter()
        .filter(|d| !d.path.starts_with("<embedded:"))
        .collect();
    assert!(!has_errors(&non_embedded), "unexpected errors: {non_embedded:?}");
}

// ---- checker/type_expr_and_rows.rs: row types ----

#[test]
fn type_expr_record_type_checking() {
    let diags = parse_and_check(
        r#"
module Test

Point = { x: Int, y: Int }

origin : Point
origin = { x: 0, y: 0 }

moveX : Int -> Point -> Point
moveX = dx => pt => pt <| { x: pt.x + dx }
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn type_expr_tuple_types() {
    let diags = parse_and_check(
        r#"
module Test

swap : (A, B) -> (B, A)
swap = (a, b) => (b, a)

fst : (A, B) -> A
fst = (a, _) => a
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn type_expr_function_types() {
    let diags = parse_and_check(
        r#"
module Test

apply : (A -> B) -> A -> B
apply = f => x => f x

compose : (B -> C) -> (A -> B) -> A -> C
compose = f => g => x => f (g x)
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

// ---- checker/spans_and_holes.rs: hole/placeholder desugaring ----

#[test]
fn holes_desugar_in_function_args() {
    let diags = parse_and_check(
        r#"
module Test

add1 = map (_ + 1)
"#,
    );
    let _ = diags;
}

#[test]
fn holes_in_binary_expression() {
    let diags = parse_and_check(
        r#"
module Test

double : Int -> Int
double = _ * 2
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn holes_in_field_access() {
    let diags = parse_and_check(
        r#"
module Test

getName : { name: Text } -> Text
getName = _.name
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

// ---- checker/unbound_names.rs: unbound name detection ----

#[test]
fn unbound_names_not_reported_for_known_names() {
    let diags = parse_and_check(
        r#"
module Test

x = 42
y = x + 1
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn unbound_names_lambda_param_in_scope() {
    let diags = parse_and_check(
        r#"
module Test

f = x => y => x + y
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn unbound_names_match_pattern_binders_in_scope() {
    let diags = parse_and_check(
        r#"
module Test

head : List A -> Option A
head = lst =>
  lst match
    | [] => None
    | [x, ...] => Some x
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn unbound_names_let_binding_in_scope() {
    let diags = parse_and_check(
        r#"
module Test

compute = do {
  result = 42
  pure result
}
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn unbound_names_record_field_shorthand() {
    let diags = parse_and_check(
        r#"
module Test

describe : { name: Text, age: Int } -> Text
describe = { name, age } => name
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}
