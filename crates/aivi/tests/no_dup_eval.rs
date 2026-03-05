//! No-duplicate-evaluation tests — runtime evaluation semantics.
//!
//! Each `#[test]` makes exactly ONE `run_jit()` call with a merged AIVI program
//! so the stdlib pipeline runs only once per nextest process.
//!
//! The machine tests are merged into single programs by defining each machine
//! type in the same module and shadowing `currentState` after each test section
//! (safe because each section completes before the name is rebound).

mod native_fixture;

use aivi::{desugar_target_with_cg_types_and_surface, run_cranelift_jit};
use native_fixture::write_aivi_source;
use tempfile::tempdir;

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
            run_cranelift_jit(program, cg_types, monomorph_plan, std::collections::HashMap::new(), &surface_modules)
                .expect("cranelift jit");
        })
        .expect("spawn test thread")
        .join();
    match result {
        Ok(()) => {}
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

// ─── Scrutinee, pipeline, and pure expression ─────────────────────────────────
// Covers:
//   - StepCounter: bound effect must not re-run during match on bound value.
//   - PipeCounter: piping a bound value must not re-execute the effect.
//   - Pure scrutinee: `n * n` evaluated once; all match arms see same value.

#[test]
fn no_dup_eval_scrutinee_and_pipeline() {
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

machine PipeCounter =
  {
             -> PipeZero : reset {}
    PipeZero -> PipeOne  : tap {}
    PipeOne  -> PipeTwo  : tap {}
  }

main : Effect Text Unit
main = do Effect {
  { step, currentState } = StepCounter
  res <- attempt (step {})
  assertEq (constructorName (currentState Unit)) "Count1"
  res match
    | Ok _  => assertEq (constructorName (currentState Unit)) "Count1"
    | Err _ => fail "step should succeed from Count0"

  { tap, currentState } = PipeCounter
  v <- tap {}
  assertEq (constructorName (currentState Unit)) "PipeOne"
  pipeResult = v |> (x => x)
  assertEq pipeResult Unit
  assertEq (constructorName (currentState Unit)) "PipeOne"

  n = 6
  scrutResult = (n * n) match
    | 36 => "thirty-six"
    | _  => "wrong"
  assertEq scrutResult "thirty-six"
}
"#,
    );
}

// ─── Guards and record spread ─────────────────────────────────────────────────
// Covers:
//   - SpreadCounter: `<|` patch must not re-evaluate the base.
//   - classify: guard selects correct arm with correct values.
//   - ArmCounter: only the first matching guard arm's body fires.

#[test]
fn no_dup_eval_guards_and_spread() {
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

machine ArmCounter =
  {
             -> ArmNone  : reset {}
    ArmNone  -> ArmOne   : hitOne {}
    ArmNone  -> ArmTwo   : hitTwo {}
    ArmNone  -> ArmThree : hitThree {}
  }

classify : Int -> Text
classify = n => n match
  | n when n < 0   => "negative"
  | 0              => "zero"
  | n when n < 10  => "small"
  | n when n < 100 => "medium"
  | _              => "large"

main : Effect Text Unit
main = do Effect {
  { tap, currentState } = SpreadCounter
  _ <- tap {}
  assertEq (constructorName (currentState Unit)) "SpreadOne"
  base    = { x: 10, y: 20, z: 30 }
  patched = base <| { y: 99 }
  assertEq patched.x 10
  assertEq patched.y 99
  assertEq patched.z 30
  assertEq (constructorName (currentState Unit)) "SpreadOne"

  assertEq (classify (-1)) "negative"
  assertEq (classify 0)    "zero"
  assertEq (classify 5)    "small"
  assertEq (classify 42)   "medium"
  assertEq (classify 200)  "large"

  { hitOne, hitTwo, hitThree, currentState } = ArmCounter
  armN = 5
  _ <- armN match
    | n when n < 0  => hitOne {}
    | n when n < 10 => hitTwo {}
    | _             => hitThree {}
  assertEq (constructorName (currentState Unit)) "ArmTwo"
}
"#,
    );
}
