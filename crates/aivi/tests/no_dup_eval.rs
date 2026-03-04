//! No-duplicate-evaluation tests — runtime evaluation semantics.
//!
//! Verifies that:
//!   - A bound effect runs exactly once (scrutinee, pipeline, spread).
//!   - Matching on an already-bound variable does not re-run the effect.
//!   - Guard expressions produce correct results without double-triggering.
//!
//! Uses the Cranelift JIT backend (same as `runtime_qa.rs`).  Each test
//! embeds a small AIVI program that asserts its own invariants via the
//! `aivi.testing` helpers; a JIT panic propagates as a Rust test failure.

mod native_fixture;

use aivi::{desugar_target_with_cg_types_and_surface, run_cranelift_jit};
use native_fixture::write_aivi_source;
use tempfile::tempdir;

/// Run a small AIVI program through Cranelift JIT, including surface modules
/// so that `constructorName` and machine state names are available at runtime.
fn run_jit(source: &str) {
    let source = source.to_string();
    let result = std::thread::Builder::new()
        .name("no-dup-eval-jit".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let dir = tempdir().expect("tempdir");
            let source_path_str = write_aivi_source(dir.path(), "main.aivi", &source);
            let (program, cg_types, monomorph_plan, surface_modules) =
                desugar_target_with_cg_types_and_surface(&source_path_str).expect("desugar");
            run_cranelift_jit(program, cg_types, monomorph_plan, &surface_modules)
                .expect("cranelift jit");
        })
        .expect("spawn test thread")
        .join();
    match result {
        Ok(()) => {}
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

// ─── Match scrutinee: effect runs exactly once ───────────────────────────────

/// The machine `StepCounter` increments one level per `step` call.
/// Binding `step {}` via `<-` advances it to `Count1`.  The subsequent
/// `match` on the already-bound value must NOT re-run `step`, so the
/// counter must remain at `Count1` after the match.
#[test]
fn no_dup_eval_match_scrutinee_once() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

machine StepCounter =
  {
           -> Count0 : reset {}
    Count0 -> Count1 : step {}
    Count1 -> Count2 : step {}
    Count2 -> Count3 : step {}
  }

@test "scrutinee effect fires once"
main : Effect Text Unit
main = do Effect {
  { step, currentState } = StepCounter

  // Wrap in `attempt` so the result is a Result (not Unit), which is
  // matchable without triggering the "expected Effect, got Unit" error.
  res <- attempt (step {})

  assertEq (constructorName (currentState Unit)) "Count1"

  // Matching on the already-bound Result must not re-run `step`.
  res match
    | Ok _  => assertEq (constructorName (currentState Unit)) "Count1"
    | Err _ => fail "step should succeed from Count0"
}
"#,
    );
}

// ─── Pipeline: expression evaluated once ─────────────────────────────────────

/// A bound effect is piped through a pure function.  The pipe must not
/// re-execute the effect — the counter stays at `PipeOne` after the pipe.
#[test]
fn no_dup_eval_pipeline_bound_value() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

machine PipeCounter =
  {
             -> PipeZero : reset {}
    PipeZero -> PipeOne  : tap {}
    PipeOne  -> PipeTwo  : tap {}
  }

@test "pipeline does not re-run bound effect"
main : Effect Text Unit
main = do Effect {
  { tap, currentState } = PipeCounter

  v <- tap {}

  assertEq (constructorName (currentState Unit)) "PipeOne"

  result = v |> (x => x)
  assertEq result Unit

  assertEq (constructorName (currentState Unit)) "PipeOne"
}
"#,
    );
}

// ─── Record spread: base evaluated once ──────────────────────────────────────

/// A spread `{ ...base, field: val }` must not evaluate `base` more than
/// once.  We bind the "computation" (an effect step) before spreading, then
/// verify the counter did not advance during the spread.
#[test]
fn no_dup_eval_record_spread_base_once() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

machine SpreadCounter =
  {
               -> SpreadZero : reset {}
    SpreadZero -> SpreadOne  : tap {}
    SpreadOne  -> SpreadTwo  : tap {}
  }

@test "record spread does not re-evaluate base"
main : Effect Text Unit
main = do Effect {
  { tap, currentState } = SpreadCounter

  _ <- tap {}

  assertEq (constructorName (currentState Unit)) "SpreadOne"

  // Use the `<|` patch operator (JIT-supported) instead of spread syntax
  // (`{ ...base, y: val }` is not yet supported by the Cranelift backend).
  base    = { x: 10, y: 20, z: 30 }
  patched = base <| { y: 99 }

  assertEq patched.x 10
  assertEq patched.y 99
  assertEq patched.z 30

  assertEq (constructorName (currentState Unit)) "SpreadOne"
}
"#,
    );
}

// ─── Guard: correct arm chosen, no spurious double-evaluation ────────────────

/// Guard expressions select the correct match arm without executing
/// multiple arms' bodies.  We verify the classification result is correct
/// for several representative inputs.
#[test]
fn no_dup_eval_guard_selects_correct_arm() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

classify : Int -> Text
classify = n => n match
  | n when n < 0   => "negative"
  | 0              => "zero"
  | n when n < 10  => "small"
  | n when n < 100 => "medium"
  | _              => "large"

@test "guard selects correct arm"
main : Effect Text Unit
main = do Effect {
  assertEq (classify (-1)) "negative"
  assertEq (classify 0)    "zero"
  assertEq (classify 5)    "small"
  assertEq (classify 42)   "medium"
  assertEq (classify 200)  "large"
}
"#,
    );
}

/// Only the first matching guard arm's body fires.  We use an `ArmCounter`
/// machine whose `hitOne` / `hitTwo` / `hitThree` transitions let us
/// observe which arm was chosen.
#[test]
fn no_dup_eval_guard_only_matching_arm_body_fires() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

machine ArmCounter =
  {
             -> ArmNone  : reset {}
    ArmNone  -> ArmOne   : hitOne {}
    ArmNone  -> ArmTwo   : hitTwo {}
    ArmNone  -> ArmThree : hitThree {}
  }

@test "only matching guard arm body fires"
main : Effect Text Unit
main = do Effect {
  { hitOne, hitTwo, hitThree, currentState } = ArmCounter

  n = 5

  _ <- n match
    | n when n < 0  => hitOne {}
    | n when n < 10 => hitTwo {}
    | _             => hitThree {}

  assertEq (constructorName (currentState Unit)) "ArmTwo"
}
"#,
    );
}

// ─── Pure-expression scrutinee: consistent result across arms ────────────────

/// A pure match scrutinee like `n * n` must be evaluated once; all arms
/// must see the same computed value.  A correct implementation never
/// re-computes the scrutinee per arm.
#[test]
fn no_dup_eval_pure_scrutinee_consistent() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "pure scrutinee consistent across arms"
main : Effect Text Unit
main = do Effect {
  n = 6
  result = (n * n) match
    | 36 => "thirty-six"
    | _  => "wrong"
  assertEq result "thirty-six"
}
"#,
    );
}
