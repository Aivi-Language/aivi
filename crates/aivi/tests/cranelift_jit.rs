//! Smoke tests for the Cranelift JIT backend.

mod native_fixture;

use aivi::{desugar_target_with_cg_types, run_cranelift_jit};
use native_fixture::write_aivi_source;
use tempfile::tempdir;

fn run_jit(source: &str) {
    let source = source.to_string();
    let result = std::thread::Builder::new()
        .name("cranelift-jit".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let dir = tempdir().expect("tempdir");
            let source_path_str = write_aivi_source(dir.path(), "main.aivi", &source);
            let (program, cg_types) =
                desugar_target_with_cg_types(&source_path_str).expect("desugar");
            run_cranelift_jit(program, cg_types).expect("cranelift jit");
        })
        .expect("spawn test thread")
        .join();
    match result {
        Ok(()) => {}
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

#[test]
fn cranelift_jit_smoke_hello() {
    run_jit(
        r#"module app.main
main : Effect Text Unit
main = do Effect {
  print "Hello from Cranelift!"
}
"#,
    );
}

#[test]
fn cranelift_jit_pure_function() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

add = a => b => a + b

@test "add works"
main : Effect Text Unit
main = do Effect {
  result <- pure (add 3 4)
  assertEq result 7
}
"#,
    );
}

#[test]
fn cranelift_jit_if_expression() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

clamp = x => if x > 10 then 10 else x

@test "clamp works"
main : Effect Text Unit
main = do Effect {
  assertEq (clamp 5) 5
  assertEq (clamp 15) 10
}
"#,
    );
}
