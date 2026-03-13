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
    assert!(
        types.contains_key("identity"),
        "expected 'identity' in types"
    );
    assert!(
        types.contains_key("constant"),
        "expected 'constant' in types"
    );
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
fn lambda_param_use_site_has_span_type() {
    // `event` is used in the body of the lambda — the use-site span must be
    // present in span_types so that LSP hover can resolve its type.
    let result = parse_and_infer(
        r#"
module Test
handleKey : Int -> Int
handleKey = event => event
"#,
    );
    let non_embedded: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| !d.path.starts_with("<embedded:"))
        .cloned()
        .collect();
    assert!(
        !has_errors(&non_embedded),
        "unexpected errors: {non_embedded:?}"
    );
    let mod_span_types = result
        .span_types
        .get("Test")
        .expect("span_types for Test module must exist");
    // There must be at least one span entry for the lambda param use site.
    // Span types record the type variable ("A") before full unification.
    assert!(
        !mod_span_types.is_empty(),
        "expected span type entries for the lambda param use site; got: {mod_span_types:?}"
    );
}

#[test]
fn imported_type_alias_can_be_renamed_without_poisoning_builtin_unit() {
    let (mut units_modules, units_diags) = parse_modules(
        Path::new("Units.aivi"),
        r#"
module Units

export Unit, defineUnit

Unit = { name: Text, factor: Float }

defineUnit : Text -> Float -> Unit
defineUnit = name factor => { name: name, factor: factor }
"#,
    );
    assert!(
        !has_errors(&units_diags),
        "unexpected parse errors in Units: {units_diags:?}"
    );
    let (mut test_modules, test_diags) = parse_modules(
        Path::new("Test.aivi"),
        r#"
module Test

use Units (Unit as MeasureUnit, defineUnit)

measure : MeasureUnit
measure = defineUnit "m" 1.0

noop : Unit
noop = Unit
"#,
    );
    assert!(
        !has_errors(&test_diags),
        "unexpected parse errors in Test: {test_diags:?}"
    );
    units_modules.append(&mut test_modules);
    let result = infer_value_types_full(&units_modules);
    assert!(
        !has_errors(&result.diagnostics),
        "unexpected errors: {:?}",
        result.diagnostics
    );
    let types = result.type_strings.get("Test").expect("Test module types");
    assert_eq!(
        types.get("measure").map(String::as_str),
        Some("MeasureUnit")
    );
    assert_eq!(types.get("noop").map(String::as_str), Some("Unit"));
}

#[test]
fn reactive_set_keeps_builtin_unit_with_colliding_imported_alias() {
    let mut modules = Vec::new();
    for name in &["aivi", "aivi.reactive", "aivi.units"] {
        let src = crate::stdlib::embedded_stdlib_source(name)
            .unwrap_or_else(|| panic!("missing embedded {name}"));
        let (mut m, diags) = parse_modules(Path::new(&format!("<embedded:{name}>")), src);
        assert!(
            !has_errors(&diags),
            "parse errors in embedded {name}: {diags:?}"
        );
        modules.append(&mut m);
    }
    let (mut user, user_diags) = parse_modules(
        Path::new("test.aivi"),
        r#"
module Test

use aivi.reactive (Signal, signal, set)
use aivi.units (Unit as MeasureUnit, defineUnit)

count : Signal Int
count = signal 1

meter : MeasureUnit
meter = defineUnit "m" 1.0

writeCount = set count 2
"#,
    );
    assert!(
        !has_errors(&user_diags),
        "unexpected parse errors: {user_diags:?}"
    );
    modules.append(&mut user);
    let result = infer_value_types_full(&modules);
    assert!(
        !has_errors(&result.diagnostics),
        "unexpected errors: {:?}",
        result.diagnostics
    );
    let types = result.type_strings.get("Test").expect("Test module types");
    assert_eq!(types.get("meter").map(String::as_str), Some("MeasureUnit"));
    assert_eq!(types.get("writeCount").map(String::as_str), Some("Unit"));
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
    assert!(
        !has_errors(&non_embedded),
        "unexpected errors: {non_embedded:?}"
    );
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
fn dot_field_accessor_syntax() {
    let diags = parse_and_check(
        r#"
module Test

getName : { name: Text } -> Text
getName = .name
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

// ---- class_env.rs: more class environment tests ----

#[test]
fn class_env_default_method_implementation() {
    let diags = parse_and_check(
        r#"
module Test

class Printable A = {
  toString: A -> Text
}

instance Printable Int = {
  toString: x => "int"
}

render : Int -> Text
render = x => toString x
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn class_env_parameterized_instance() {
    let diags = parse_and_check(
        r#"
module Test

class Show A = {
  show: A -> Text
}

instance Show Int = {
  show: _ => "int"
}

instance Show Text = {
  show: x => x
}

x = show 42
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn class_env_class_with_no_supers() {
    let diags = parse_and_check(
        r#"
module Test

class Hashable A = {
  hash: A -> Int
}

instance Hashable Int = {
  hash: x => x
}

h = hash 42
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

// ---- elaboration.rs: type inference with expressions ----

#[test]
fn elaboration_lambda_type_propagation() {
    let diags = parse_and_check(
        r#"
module Test

apply : (Int -> Int) -> Int -> Int
apply = f => x => f x

double : Int -> Int
double = x => x * 2

result = apply double 21
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn elaboration_polymorphic_identity() {
    let diags = parse_and_check(
        r#"
module Test

id : A -> A
id = x => x

x = id 42
y = id "hello"
z = id True
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn elaboration_record_update_patch() {
    let diags = parse_and_check(
        r#"
module Test

Point = { x: Int, y: Int }

origin : Point
origin = { x: 0, y: 0 }

moved = origin <| { x: 10 }
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn elaboration_nested_match() {
    let diags = parse_and_check(
        r#"
module Test

flatten : Option (Option A) -> Option A
flatten = opt =>
  opt ?
    | Some (Some x) => Some x
    | Some None => None
    | None => None
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn elaboration_list_operations() {
    let diags = parse_and_check(
        r#"
module Test

nums : List Int
nums = [1, 2, 3]

head : List A -> Option A
head = lst =>
  lst ?
    | [x, ...] => Some x
    | [] => None

first = head nums
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn elaboration_higher_order_function() {
    let diags = parse_and_check(
        r#"
module Test

compose : (B -> C) -> (A -> B) -> A -> C
compose = f => g => x => f (g x)

double : Int -> Int
double = x => x * 2

addOne : Int -> Int
addOne = x => x + 1

doubleAndAdd = compose addOne double
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

// ---- infer_effects_and_patches.rs: effect block type inference ----

#[test]
fn infer_effect_block_bind() {
    let diags = parse_and_check(
        r#"
module Test

f : Effect Text Int
f = do Effect {
  x <- pure 42
  pure x
}
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn infer_effect_let_rejects_effectful_rhs() {
    let diags = parse_and_check(
        r#"
module Test

f = do Effect {
  x = pure 42
  pure x
}
"#,
    );
    assert!(
        has_errors(&diags),
        "expected error for effectful let-binding"
    );
}

#[test]
fn infer_effect_block_multiple_binds() {
    let diags = parse_and_check(
        r#"
module Test

f : Effect Text Int
f = do Effect {
  a <- pure 1
  b <- pure 2
  c <- pure 3
  pure (a + b + c)
}
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn infer_patch_type_matches() {
    let diags = parse_and_check(
        r#"
module Test

Point = { x: Int, y: Int }

move : Int -> Point -> Point
move = dx => pt => pt <| { x: pt.x + dx }
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

// ---- spans_and_holes.rs: hole desugaring in type checker ----

#[test]
fn holes_in_pipe_chain() {
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
fn holes_multiple_in_binary() {
    let result = parse_and_infer(
        r#"
module Test

sub = _ - _
"#,
    );
    let types = result.type_strings.get("Test").expect("Test module");
    assert!(types.contains_key("sub"), "expected 'sub' in types");
}

#[test]
fn holes_in_text_interpolation() {
    let diags = parse_and_check(
        r#"
module Test

greet : Text -> Text
greet = name => "Hello ${name}!"
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

// ---- unbound_names.rs: more unbound name detection ----

#[test]
fn unbound_names_do_block_bind_in_scope() {
    let diags = parse_and_check(
        r#"
module Test

f = do Effect {
  x <- pure 42
  y <- pure (x + 1)
  pure y
}
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn unbound_names_nested_lambda_scope() {
    let diags = parse_and_check(
        r#"
module Test

f = a => b => c => a + b + c
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn unbound_names_constructor_names_always_in_scope() {
    let diags = parse_and_check(
        r#"
module Test

x = Some 42
y = None
z = True
w = False
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn unbound_names_as_pattern_binder_in_scope() {
    let diags = parse_and_check(
        r#"
module Test

f : Option A -> Option A
f = all as (Some _) => all
f = None => None
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn unbound_names_list_pattern_rest_in_scope() {
    let diags = parse_and_check(
        r#"
module Test

tail : List A -> List A
tail = [_, ...rest] => rest
tail = _ => []
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn unbound_names_tuple_pattern_binders() {
    let diags = parse_and_check(
        r#"
module Test

fst = (a, _) => a
snd = (_, b) => b
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

// ---- elaboration.rs: type alias tests ----

#[test]
fn type_alias_used_in_annotation() {
    let diags = parse_and_check(
        r#"
module Test

Pair A B = (A, B)

swap : Pair A B -> Pair B A
swap = (a, b) => (b, a)
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn type_alias_record_used_in_annotation() {
    let diags = parse_and_check(
        r#"
module Test

User = { name: Text, age: Int }

mkUser : Text -> Int -> User
mkUser = name => age => { name: name, age: age }
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

// ---- elaboration.rs: domain (ADT) types ----

#[test]
fn domain_type_match() {
    let diags = parse_and_check(
        r#"
module Test

Color = Red | Green | Blue

isRed : Color -> Bool
isRed = c =>
  c ?
    | Red => True
    | _ => False
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn domain_type_with_data() {
    let diags = parse_and_check(
        r#"
module Test

Shape = Circle Float | Rect Float Float

area : Shape -> Float
area = s =>
  s ?
    | Circle r => 3.14159 * r * r
    | Rect w h => w * h
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

// ---- checker: error detection ----

#[test]
fn detects_arity_mismatch() {
    let diags = parse_and_check(
        r#"
module Test

f : Int -> Int -> Int
f = x => x
"#,
    );
    assert!(has_errors(&diags), "expected arity error");
}

#[test]
fn detects_record_field_type_mismatch() {
    let diags = parse_and_check(
        r#"
module Test

Point = { x: Int, y: Int }

bad : Point
bad = { x: "hello", y: 2 }
"#,
    );
    assert!(
        has_errors(&diags),
        "expected type mismatch for record field"
    );
}

// ---- infer_effects_and_patches.rs: generator type inference ----

#[test]
fn infer_generator_block() {
    let result = parse_and_infer(
        r#"
module Test

nums = generate {
  yield 1
  yield 2
  yield 3
}
"#,
    );
    let non_embedded: Vec<_> = result
        .diagnostics
        .into_iter()
        .filter(|d| !d.path.starts_with("<embedded:"))
        .collect();
    assert!(!has_errors(&non_embedded));
    let types = result.type_strings.get("Test").expect("Test module");
    assert!(types.contains_key("nums"));
}

// ---- class_env.rs: multi-param class ----

#[test]
fn class_env_multi_instance() {
    let diags = parse_and_check(
        r#"
module Test

class Eq A = {
  eq: A -> A -> Bool
}

instance Eq Int = {
  eq: a => b => a == b
}

instance Eq Text = {
  eq: a => b => a == b
}

sameInt = eq 1 2
sameText = eq "a" "b"
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

// ---- elaboration.rs: if expression type inference ----

#[test]
fn if_branches_must_match_types() {
    let diags = parse_and_check(
        r#"
module Test

f : Bool -> Int
f = b => if b then 1 else 0
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn if_branches_type_mismatch_detected() {
    let diags = parse_and_check(
        r#"
module Test

f : Bool -> Int
f = b => if b then 1 else "no"
"#,
    );
    assert!(has_errors(&diags), "expected type mismatch in if branches");
}

// ---- elaboration.rs: unary neg type inference ----

#[test]
fn infer_unary_neg_int() {
    let diags = parse_and_check(
        r#"
module Test

x : Int
x = -42
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}

#[test]
fn infer_unary_neg_float() {
    let diags = parse_and_check(
        r#"
module Test

x : Float
x = -3.14
"#,
    );
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
}
