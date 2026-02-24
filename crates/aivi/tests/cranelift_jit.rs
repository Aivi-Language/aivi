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

#[test]
fn cranelift_jit_match_expression_falls_back_to_interpreter() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

Option A = None | Some A

safeHead = xs => xs match
  | []       => None
  | [x, ...] => Some x

@test "match works"
main : Effect Text Unit
main = do Effect {
  assertEq (safeHead [1, 2, 3]) (Some 1)
  assertEq (safeHead []) None
}
"#,
    );
}

#[test]
fn cranelift_jit_lambda_closure() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

makeAdder = n => x => n + x

@test "closure captures work"
main : Effect Text Unit
main = do Effect {
  addFive <- pure (makeAdder 5)
  assertEq (addFive 3) 8
  assertEq (addFive 10) 15
}
"#,
    );
}

#[test]
fn cranelift_jit_pattern_matching_constructor() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

Option A = None | Some A

unwrapOr = default => opt => opt match
  | Some x => x
  | None   => default

@test "constructor pattern matching"
main : Effect Text Unit
main = do Effect {
  assertEq (unwrapOr 0 (Some 42)) 42
  assertEq (unwrapOr 0 None) 0
}
"#,
    );
}

#[test]
fn cranelift_jit_record_patching() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

updateAge = person => person <| { age: 99 }

@test "record patching"
main : Effect Text Unit
main = do Effect {
  result <- pure (updateAge { name: "Bob", age: 30 })
  assertEq result { name: "Bob", age: 99 }
}
"#,
    );
}

#[test]
fn cranelift_jit_generate_block() {
    // Generate blocks are desugared by the kernel into Church-encoded folds.
    // Three yields: `yield 10; yield 20; yield 30` becomes nested gen_append
    // combinators. The generator is called directly as a fold function.
    run_jit(
        r#"@no_prelude
module app.main

use aivi.testing

gen = generate {
  yield 10
  yield 20
  yield 30
}

@test "generate block"
main : Effect Text Unit
main = do Effect {
  result <- pure (gen (a => b => a + b) 0)
  assertEq result 60
}
"#,
    );
}

#[test]
fn cranelift_jit_resource_block() {
    // Resource blocks are preserved in the RustIR (not kernel-desugared).
    // The JIT delegates to the interpreter via rt_make_resource.
    // Here we verify that a module with a resource block compiles
    // and runs without crashing (matching integration test coverage).
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

managedResource = name => resource {
  yield name
}

@test "resource block"
main : Effect Text Unit
main = do Effect {
  assertEq (1 + 1) 2
}
"#,
    );
}
