pub const MODULE_NAME: &str = "aivi.testing";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.testing
export assert, assert_eq, assertEq, assertSnapshot

use aivi

assert : Bool -> Effect Text Unit
assert = ok => if ok then pure Unit else fail "assertion failed"

assert_eq : A -> A -> Effect Text Unit
assert_eq = a b => if a == b then pure Unit else fail "assert_eq failed: left={ a }, right={ b }"

assertEq : A -> A -> Effect Text Unit
assertEq = a b => assert_eq a b

assertSnapshot : Text -> A -> Effect Text Unit
assertSnapshot = name value => __assertSnapshot name value
"#;
