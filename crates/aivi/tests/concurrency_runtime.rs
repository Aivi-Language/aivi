mod native_fixture;

use native_fixture::run_jit_err;

fn assert_retry_attempts_runtime_error(attempts: i64) {
    let err = run_jit_err(
        "concurrency-runtime",
        &format!(
            r#"@no_prelude
module app.main

use aivi
use aivi.concurrency

main : Effect Text Unit
main = retry {attempts} (pure Unit)
"#
        ),
    );
    let rendered = err.render(false);
    assert!(
        rendered.contains("concurrent.retry expects attempts > 0"),
        "unexpected runtime error:\n{rendered}"
    );
}

#[test]
fn retry_rejects_zero_and_negative_attempt_counts() {
    assert_retry_attempts_runtime_error(0);
    assert_retry_attempts_runtime_error(-1);
}
