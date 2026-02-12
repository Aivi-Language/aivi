use std::path::Path;

use aivi::{check_modules, check_types, parse_modules};

fn check_ok(source: &str) {
    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(diagnostics.is_empty(), "parse diagnostics: {diagnostics:?}");

    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        module_diags.is_empty(),
        "type diagnostics: {module_diags:?}"
    );
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
module test.core
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
}"#;
    check_ok(source);
}

#[test]
fn typecheck_domains_patching() {
    let source = r#"
module test.m7
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

updated = addWeek { year: 2024, month: 9, day: 1 }"#;
    check_ok(source);
}

#[test]
fn typecheck_domain_operator_overload_without_deltas() {
    let source = r#"
module test.domain_ops
export move

Vec2 = { x: Int, y: Int }

domain Vector over Vec2 = {
  (+) : Vec2 -> Vec2 -> Vec2
  (+) a b = { x: a.x + b.x, y: a.y + b.y }
}

move : Vec2 -> Vec2 -> Vec2
move pos vel = pos + vel"#;
    check_ok(source);
}

#[test]
fn typecheck_error_unknown_numeric_delta_literal() {
    let source = r#"
module test.delta_err
export value
value = 2w"#;
    check_err(source);
}

#[test]
fn typecheck_error_effect_final() {
    let source = r#"
module test.err
export main

main : Effect Text Unit
main = effect {
  1
}"#;
    check_err(source);
}

#[test]
fn typecheck_effect_block_pure_let_and_unit_statements() {
    let source = r#"
module test.effect_sugar
export main

// Minimal local stand-ins
Result E A = Err E | Ok A

main : Effect Text Unit
main = effect {
  n <- 41
  m <- n + 1

  res <- attempt (if m == 42 then fail "boom" else pure m)

  verdict = res ?
    | Ok _  => "ok"
    | Err _ => "err"

  print verdict

  if m > 40 then print "branch" else Unit
}"#;
    check_ok(source);
}

#[test]
fn typecheck_effect_block_statement_requires_unit() {
    let source = r#"
module test.effect_stmt_unit
export main, foo

foo : Effect Text Int
foo = pure 1

main : Effect Text Unit
main = effect {
  println "start"
  foo
  pure Unit
}"#;
    check_err(source);
}

#[test]
fn typecheck_effect_block_let_rejects_effect_expr() {
    let source = r#"
module test.effect_let_err
export main

main : Effect Text Unit
main = effect {
  x = print "nope"
  pure Unit
}"#;
    check_err(source);
}

#[test]
fn typecheck_flattened_constructor_chain_patterns() {
    let source = r#"
module test.pattern_chain
export msg

Result E A = Err E | Ok A
Error = NotFound Text | Other

res : Result Error Text
res = Err (NotFound "hi")

msg : Text
msg = res ?
  | Err NotFound m => m
  | _              => "no-msg""#;
    check_ok(source);
}

#[test]
fn typecheck_error_unknown_name() {
    let source = r#"
module test.err
export value
value = missing"#;
    check_err(source);
}

#[test]
fn typecheck_open_records_allow_extra_fields() {
    let source = r#"
module test.open
export value

getName : { name: Text } -> Text
getName user = user.name

value = getName { name: "Alice", id: 1 }"#;
    check_ok(source);
}

#[test]
fn typecheck_type_classes_resolve_instances() {
    let source = r#"
module test.classes
export value

class Eq A = {
  eq: A -> A -> Bool
}

instance Eq Bool = {
  eq: x y => x == y
}

value = eq True False"#;
    check_ok(source);
}

#[test]
fn typecheck_row_type_ops_and_patch_alias() {
    let source = r#"
module test.rows
export getName, getEmail, publicEmail, promote

User = { id: Int, email: Text, name: Text, isAdmin: Bool }

UserName = Pick (name) User
UserMaybe = User |> Optional (email)
UserReq = UserMaybe |> Required (email)
UserPublic = User |> Omit (isAdmin) |> Rename { email: email_address }

getName : UserName -> Text
getName u = u.name

getEmail : UserReq -> Text
getEmail u = u.email

publicEmail : UserPublic -> Text
publicEmail u = u.email_address

promote : Patch User
promote u = u <| { isAdmin: True }"#;
    check_ok(source);
}

#[test]
fn typecheck_patch_literal() {
    let source = r#"
module test.patch_literal
export promote

User = { id: Int, name: Text, age: Int }

promote : Patch User
promote = patch { age: _ + 1 }"#;
    check_ok(source);
}

#[test]
fn typecheck_row_op_errors() {
    let source = r#"
module test.row_errors
User = { id: Int, name: Text }

badPick : Pick (missing) User
badPick = { id: 1, name: "x" }

badRename : Rename { id: name } User
badRename = { name: 1 }"#;
    check_err(source);
}

#[test]
fn typecheck_type_classes_missing_instance_errors() {
    let source = r#"
module test.classes_err
export value

class Eq A = {
  eq: A -> A -> Bool
}

value = eq True False"#;
    check_err(source);
}

#[test]
fn typecheck_hkts_functor_map() {
    let source = r#"
module test.functor
export value

Option A = None | Some A

class Functor (F *) = {
  map: F A -> (A -> B) -> F B
}

instance Functor (Option *) = {
  map: opt f => opt ?
    | None => None
    | Some x => Some (f x)
}

inc x = x + 1
value = map (Some 1) inc"#;
    check_ok(source);
}
