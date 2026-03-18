mod native_fixture;

use native_fixture::run_jit_err;

fn assert_division_by_zero_runtime_error(thread_name: &str, source: &str) {
    let err = run_jit_err(thread_name, source);
    let rendered = err.render(false);
    assert!(
        rendered.contains("RT1204"),
        "expected RT1204 division-by-zero diagnostic:\n{rendered}"
    );
    assert!(
        rendered.contains("attempted to divide by zero"),
        "unexpected runtime error:\n{rendered}"
    );
}

#[test]
fn decimal_facade_division_by_zero_reports_runtime_error() {
    assert_division_by_zero_runtime_error(
        "number-runtime-decimal",
        r#"@no_prelude
module app.main

use aivi
use aivi.number as num
use aivi.number.decimal
use aivi.number.decimal (domain Decimal)

main : Effect Text Unit
main = do Effect {
  quotient = num.fromFloat 1.0 / num.fromFloat 0.0
  _ = num.toFloat quotient
  pure Unit
}
"#,
    );
}

#[test]
fn rational_facade_division_by_zero_reports_runtime_error() {
    assert_division_by_zero_runtime_error(
        "number-runtime-rational",
        r#"@no_prelude
module app.main

use aivi
use aivi.number as num
use aivi.number.rational
use aivi.number.rational (domain Rational)

main : Effect Text Unit
main = do Effect {
  half = num.fromBigInts (num.fromInt 1) (num.fromInt 2)
  zero = num.fromBigInts (num.fromInt 0) (num.fromInt 1)
  quotient = rational.div half zero
  _ = num.numerator quotient
  pure Unit
}
"#,
    );
}
