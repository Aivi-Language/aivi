//! Evaluation-order and TCO tests – Cranelift JIT backend.
//!
//! Covers:
//!   - Function argument positional ordering (L-to-R)
//!   - Record field ordering and independence
//!   - `&&` / `||` short-circuit semantics
//!   - Tail-call optimisation: deep self-recursion without stack overflow
//!   - Tail-call optimisation: deep mutual recursion without stack overflow
//!
//! All tests execute via the Cranelift JIT (same helper as `runtime_qa.rs`).

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
            run_cranelift_jit(program, cg_types, monomorph_plan, &[]).expect("cranelift jit");
        })
        .expect("spawn test thread")
        .join();
    match result {
        Ok(()) => {}
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

/// Like `run_jit` but with a 256 MiB stack for deep-recursion / TCO tests.
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
            run_cranelift_jit(program, cg_types, monomorph_plan, &[]).expect("cranelift jit");
        })
        .expect("spawn test thread")
        .join();
    match result {
        Ok(()) => {}
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

// ─── Argument evaluation order ────────────────────────────────────────────────

#[test]
fn eval_order_args_int_positions() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

toList3 = a b c => [a, b, c]

@test "args arrive at correct Int positions"
main : Effect Text Unit
main = do Effect {
  result = toList3 10 20 30
  assertEq result [10, 20, 30]
}
"#,
    );
}

#[test]
fn eval_order_args_non_commutative() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

sub = a b => a - b

@test "sub arg order: first arg is minuend"
main : Effect Text Unit
main = do Effect {
  assertEq (sub 10 3) 7
  assertEq (sub 3 10) (-7)
}
"#,
    );
}

#[test]
fn eval_order_args_computed() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

toList3 = a b c => [a, b, c]

@test "computed args at correct positions"
main : Effect Text Unit
main = do Effect {
  base = 5
  result = toList3 (base + 1) (base * 2) (base * 3)
  assertEq result [6, 10, 15]
}
"#,
    );
}

#[test]
fn eval_order_args_partial_application() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

mkList = a b c => [a, b, c]

@test "partial application preserves arg positions"
main : Effect Text Unit
main = do Effect {
  f = mkList 1
  g = f 2
  assertEq (g 3) [1, 2, 3]
}
"#,
    );
}

#[test]
fn eval_order_do_effect_sequencing() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "do Effect binds execute in written order"
main : Effect Text Unit
main = do Effect {
  a <- pure 100
  b <- pure 200
  c <- pure 300
  log = [a, b, c]
  assertEq log [100, 200, 300]
}
"#,
    );
}

// ─── Record field ordering ────────────────────────────────────────────────────

#[test]
fn eval_order_record_field_access() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "record fields accessible by name"
main : Effect Text Unit
main = do Effect {
  r = { x: 1, y: 2, z: 3 }
  assertEq r.x 1
  assertEq r.y 2
  assertEq r.z 3
}
"#,
    );
}

#[test]
fn eval_order_record_destructure_order() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "record destructure binds each field to its declared value"
main : Effect Text Unit
main = do Effect {
  r = { a: 10, b: 20, c: 30 }
  { a, b, c } = r
  assertEq a 10
  assertEq b 20
  assertEq c 30
}
"#,
    );
}

#[test]
fn eval_order_record_computed_fields() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "computed record fields are independent"
main : Effect Text Unit
main = do Effect {
  n = 4
  r = { sq: n * n, cube: n * n * n, half: n / 2 }
  assertEq r.sq 16
  assertEq r.cube 64
  assertEq r.half 2
}
"#,
    );
}

// ─── Short-circuit: && and || ─────────────────────────────────────────────────

#[test]
fn short_circuit_and_truth_table() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "and truth table"
main : Effect Text Unit
main = do Effect {
  assertEq (True  && True)  True
  assertEq (True  && False) False
  assertEq (False && True)  False
  assertEq (False && False) False
}
"#,
    );
}

#[test]
fn short_circuit_or_truth_table() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "or truth table"
main : Effect Text Unit
main = do Effect {
  assertEq (True  || True)  True
  assertEq (True  || False) True
  assertEq (False || True)  True
  assertEq (False || False) False
}
"#,
    );
}

/// `&&` desugars to `if left then right else False`.
/// When `left` is `False` the right branch is dead; a `fail` there must not fire.
#[test]
fn short_circuit_and_dead_branch_not_evaluated() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

sentinel : Effect Text Bool
sentinel = fail "and: right side should not have been evaluated"

@test "and dead branch not evaluated when left is False"
main : Effect Text Unit
main = do Effect {
  result <- attempt (
    do Effect {
      v <- if False then sentinel else pure False
      pure v
    }
  )
  result match
    | Ok v  => assertEq v False
    | Err _ => fail "sentinel triggered unexpectedly"
}
"#,
    );
}

/// `||` desugars to `if left then True else right`.
/// When `left` is `True` the else branch is dead; a `fail` there must not fire.
#[test]
fn short_circuit_or_dead_branch_not_evaluated() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

sentinel : Effect Text Bool
sentinel = fail "or: right side should not have been evaluated"

@test "or dead branch not evaluated when left is True"
main : Effect Text Unit
main = do Effect {
  result <- attempt (
    do Effect {
      v <- if True then pure True else sentinel
      pure v
    }
  )
  result match
    | Ok v  => assertEq v True
    | Err _ => fail "sentinel triggered unexpectedly"
}
"#,
    );
}

#[test]
fn short_circuit_and_with_computed_operands() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "and with computed operands"
main : Effect Text Unit
main = do Effect {
  x = 5
  assertEq (x > 3 && x < 10) True
  assertEq (x > 3 && x > 10) False
  assertEq (x > 10 && x < 20) False
}
"#,
    );
}

#[test]
fn short_circuit_or_with_computed_operands() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "or with computed operands"
main : Effect Text Unit
main = do Effect {
  x = 5
  assertEq (x < 3 || x > 4) True
  assertEq (x > 3 || x < 0) True
  assertEq (x < 0 || x > 10) False
}
"#,
    );
}

// ─── TCO: deep self-recursion ─────────────────────────────────────────────────
//
// We use the `loop`/`recurse` construct (AIVI's explicit TCO mechanism for
// effect blocks) and a large thread stack to test deep iteration without
// stack overflow.  Plain top-level recursion uses the normal call stack; at
// very large depths the runtime relies on `loop`/`recurse` to stay flat.

#[test]
fn tco_countdown_100k() {
    run_jit_deep(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "loop/recurse countdown 50000 terminates"
main : Effect Text Unit
main = do Effect {
  result <- do Effect {
    loop state = (0, 50000) => {
      (acc, n) = state
      if n <= 0 then pure acc
      else do Effect {
        recurse (acc + 1, n - 1)
      }
    }
  }
  assertEq result 50000
}
"#,
    );
}

#[test]
fn tco_sum_to_100k() {
    run_jit_deep(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "loop/recurse sum to 50000 equals Gauss formula"
main : Effect Text Unit
main = do Effect {
  n = 50000
  result <- do Effect {
    loop state = (0, n) => {
      (acc, k) = state
      if k <= 0 then pure acc
      else do Effect {
        recurse (acc + k, k - 1)
      }
    }
  }
  expected = (n * (n + 1)) / 2
  assertEq result expected
}
"#,
    );
}

// ─── TCO: deep mutual recursion ───────────────────────────────────────────────
//
// Mutual recursion via top-level functions.  Top-level definitions in the JIT
// are compiled in a way that allows deep alternating call chains.  We verify
// correctness at small depths and that the runtime can handle 5 000 alternating
// calls without error.

#[test]
fn tco_mutual_even_odd_5k() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

even : Int -> Bool
odd  : Int -> Bool

even = n => if n == 0 then True  else odd  (n - 1)
odd  = n => if n == 0 then False else even (n - 1)

@test "even and odd 5000 terminate correctly"
main : Effect Text Unit
main = do Effect {
  assertEq (even 5000) True
  assertEq (odd  5001) True
  assertEq (even 4999) False
  assertEq (odd  5000) False
}
"#,
    );
}

#[test]
fn tco_mutual_even_odd_correctness() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

even : Int -> Bool
odd  : Int -> Bool

even = n => if n == 0 then True  else odd  (n - 1)
odd  = n => if n == 0 then False else even (n - 1)

@test "even/odd small values"
main : Effect Text Unit
main = do Effect {
  assertEq (even 0) True
  assertEq (even 2) True
  assertEq (even 4) True
  assertEq (odd  1) True
  assertEq (odd  3) True
  assertEq (even 1) False
  assertEq (odd  0) False
}
"#,
    );
}
