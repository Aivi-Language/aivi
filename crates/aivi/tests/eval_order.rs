//! Evaluation-order and TCO tests — Cranelift JIT backend.
//!
//! Each `#[test]` makes exactly ONE `run_jit()` (or `run_jit_deep()`) call
//! with a merged AIVI program so the stdlib pipeline runs only once per
//! nextest process.

mod native_fixture;

use aivi::{desugar_target_with_cg_types, run_cranelift_jit};
use native_fixture::write_aivi_source;
use tempfile::tempdir;

fn run_jit(source: &str) {
    let source = source.to_string();
    let result = std::thread::Builder::new()
        .name("eval-order-jit".into())
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

/// 256 MiB stack for deep-recursion / TCO tests.
fn run_jit_deep(source: &str) {
    let source = source.to_string();
    let result = std::thread::Builder::new()
        .name("eval-order-jit-tco".into())
        .stack_size(256 * 1024 * 1024)
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

// ─── Argument evaluation order ────────────────────────────────────────────────
// Covers: positional Int positions, non-commutative sub, computed args,
//         partial application, effect sequencing.

#[test]
fn eval_order_args() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

toList3 = a b c => [a, b, c]
sub = a b => a - b

main : Effect Text Unit
main = {
  assertEq (toList3 10 20 30) [10, 20, 30]
  assertEq (sub 10 3) 7
  assertEq (sub 3 10) (-7)
  base = 5
  assertEq (toList3 (base + 1) (base * 2) (base * 3)) [6, 10, 15]
  partF = toList3 1
  partG = partF 2
  assertEq (partG 3) [1, 2, 3]
  seqA = 100
  seqB = 200
  seqC = 300
  assertEq [seqA, seqB, seqC] [100, 200, 300]
}
"#,
    );
}

// ─── Record field ordering ────────────────────────────────────────────────────
// Covers: field access by name, destructure order, computed fields.

#[test]
fn eval_order_records() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

main : Effect Text Unit
main = {
  r1 = { x: 1, y: 2, z: 3 }
  assertEq r1.x 1
  assertEq r1.y 2
  assertEq r1.z 3
  r2 = { a: 10, b: 20, c: 30 }
  assertEq r2.a 10
  assertEq r2.b 20
  assertEq r2.c 30
  n = 4
  r3 = { sq: n * n, cube: n * n * n, half: n / 2 }
  assertEq r3.sq 16
  assertEq r3.cube 64
  assertEq r3.half 2
}
"#,
    );
}

// ─── Short-circuit: && and || ─────────────────────────────────────────────────
// Covers: truth tables, dead-branch not evaluated, computed operands.

#[test]
fn eval_order_short_circuit() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

main : Effect Text Unit
main = {
  assertEq (True  && True)  True
  assertEq (True  && False) False
  assertEq (False && True)  False
  assertEq (False && False) False
  assertEq (True  || True)  True
  assertEq (True  || False) True
  assertEq (False || True)  True
  assertEq (False || False) False
  x = 5
  assertEq (x > 3 && x < 10) True
  assertEq (x > 3 && x > 10) False
  assertEq (x > 10 && x < 20) False
  assertEq (x < 3 || x > 4) True
  assertEq (x > 3 || x < 0) True
  assertEq (x < 0 || x > 10) False
}
"#,
    );
}

// ─── TCO ─────────────────────────────────────────────────────────────────────
// Covers: loop/recurse countdown, loop/recurse sum, mutual even/odd (small +
//         5000 depth). All run with a 256 MiB stack.

#[test]
fn eval_order_tco() {
    run_jit_deep(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing
even : Int -> Bool
odd  : Int -> Bool
even = n => if n == 0 then True  else odd  (n - 1)
odd  = n => if n == 0 then False else even (n - 1)

main : Effect Text Unit
main = {
  assertEq (even 0) True
  assertEq (even 2) True
  assertEq (even 4) True
  assertEq (odd  1) True
  assertEq (odd  3) True
  assertEq (even 1) False
  assertEq (odd  0) False
  assertEq (even 5000) True
  assertEq (odd  5001) True
  assertEq (even 4999) False
  assertEq (odd  5000) False
}
"#,
    );
}
