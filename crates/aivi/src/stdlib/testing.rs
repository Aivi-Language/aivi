pub const MODULE_NAME: &str = "aivi.testing";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.testing
export assert, assert_eq, assertEq
export assertOk, assertErr, assertSome, assertNone
export assertSnapshot

use aivi

assert : Bool -> Effect Text Unit
assert = ok => if ok then pure Unit else fail "assertion failed"

assert_eq : A -> A -> Effect Text Unit
assert_eq = a b => if a == b then pure Unit else fail "assert_eq failed"

assertEq : A -> A -> Effect Text Unit
assertEq = a b => assert_eq a b

assertOk : Result E A -> Effect Text A
assertOk = result => result match
  | Ok value => pure value
  | Err _    => fail "assertOk failed: expected Ok"

assertErr : Result E A -> Effect Text E
assertErr = result => result match
  | Ok _    => fail "assertErr failed: expected Err"
  | Err err => pure err

assertSome : Option A -> Effect Text A
assertSome = value => value match
  | Some item => pure item
  | None      => fail "assertSome failed: expected Some"

assertNone : Option A -> Effect Text Unit
assertNone = value => value match
  | Some _ => fail "assertNone failed: expected None"
  | None   => pure Unit

assertSnapshot : Text -> A -> Effect Text Unit
assertSnapshot = name value => __assertSnapshot name value
"#;
