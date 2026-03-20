mod native_fixture;

use aivi::{desugar_target_with_cg_types, run_cranelift_jit};
use native_fixture::write_aivi_source;
use tempfile::tempdir;

fn run_jit(source: &str) {
    let source = source.to_string();
    let result = std::thread::Builder::new()
        .name("flow-runtime".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let dir = tempdir().expect("tempdir");
            let source_path_str = write_aivi_source(dir.path(), "main.aivi", &source);
            let (program, cg_types, monomorph_plan) =
                desugar_target_with_cg_types(&source_path_str).expect("desugar");
            run_cranelift_jit(
                program,
                cg_types,
                monomorph_plan,
                std::collections::HashMap::new(),
                &[],
            )
            .expect("cranelift jit");
        })
        .expect("spawn test thread")
        .join();
    match result {
        Ok(()) => {}
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

#[test]
fn flow_attempt_recovery_runtime() {
    run_jit(
        r#"module app.main
export main

use aivi (pure, fail)
use aivi.testing

risky = n => if n == 42 then fail "forty-two" else pure n

main : Effect Text Int
main =
  pure 42
   ?|>current => risky current
   !|>err => pure 7
   ~|>recovered => assertEq recovered 7
"#,
    );
}

#[test]
fn flow_tap_binding_runtime() {
    run_jit(
        r#"module app.main
export main

use aivi.testing

main : Effect Text Int
main =
  pure 1
    ~|> tapped => assertEq tapped 1
    ~|> tapped => assertEq tapped 1
"#,
    );
}

#[test]
fn flow_attempt_nested_match_if_runtime() {
    run_jit(
        r#"module app.main
export main

use aivi (attempt, fail, pure)
use aivi.testing

wrapOk : Int -> Result Text Int
wrapOk = value => Ok value

step : Int -> Effect Text Int
step = n => {
  res = wrapOk n
  res match
    | Err _ =>
        fail "closed"
    | Ok value =>
        if value < 2 then fail "no" else pure value
}

main : Effect Text Unit
main =
  attempt (step 2)
    |> result => result match
      | Ok value => assertEq value 2
      | Err _    => assert False
"#,
    );
}
