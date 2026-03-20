mod native_fixture;

use native_fixture::run_jit_err;

fn assert_linalg_runtime_error(thread_name: &str, source: &str, expected: &str) {
    let err = run_jit_err(thread_name, source);
    let rendered = err.render(false);
    assert!(
        rendered.contains(expected),
        "unexpected runtime error:\n{rendered}"
    );
}

#[test]
fn linear_algebra_domain_operators_report_shape_errors() {
    assert_linalg_runtime_error(
        "linalg-runtime-add",
        r#"@no_prelude
module app.main

use aivi
use aivi.linear_algebra
use aivi.testing

main : Effect Text Unit
main =
   |> pure ({ size: 2, data: [1.0, 2.0] } + { size: 3, data: [3.0, 4.0, 5.0] })#bad
   |> bad => assertEq bad.size bad.size
"#,
        "linalg.addVec expects vectors of equal size",
    );

    assert_linalg_runtime_error(
        "linalg-runtime-sub",
        r#"@no_prelude
module app.main

use aivi
use aivi.linear_algebra
use aivi.testing

main : Effect Text Unit
main =
   |> pure ({ size: 2, data: [1.0, 2.0] } - { size: 3, data: [3.0, 4.0, 5.0] })#bad
   |> bad => assertEq bad.size bad.size
"#,
        "linalg.subVec expects vectors of equal size",
    );

    assert_linalg_runtime_error(
        "linalg-runtime-scale",
        r#"@no_prelude
module app.main

use aivi
use aivi.linear_algebra
use aivi.testing

main : Effect Text Unit
main =
   |> pure ({ size: 3, data: [1.0, 2.0] } * 2.0)#bad
   |> bad => assertEq bad.size bad.size
"#,
        "linalg.scaleVec Vec.size does not match data length",
    );
}

#[test]
fn linalg_facade_reexports_the_same_runtime_validation() {
    assert_linalg_runtime_error(
        "linalg-runtime-facade",
        r#"@no_prelude
module app.main

use aivi
use aivi.linalg
use aivi.testing

main : Effect Text Unit
main =
   |> pure ({ size: 2, data: [1.0, 2.0] } + { size: 3, data: [3.0, 4.0, 5.0] })#bad
   |> bad => assertEq bad.size bad.size
"#,
        "linalg.addVec expects vectors of equal size",
    );
}
