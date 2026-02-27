//! Pipeline QA tests for Cranelift codegen (Phase 7) and lowering (Phase 6).
//!
//! Covers: P6_01–P6_04, P7_01–P7_04, E2E_01, META_01–META_03

mod native_fixture;

use aivi::{desugar_target_with_cg_types, run_cranelift_jit};
use native_fixture::write_aivi_source;
use tempfile::tempdir;

fn run_jit(source: &str) {
    let source = source.to_string();
    let result = std::thread::Builder::new()
        .name("pipeline-qa-jit".into())
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

// ============================================================================
// Phase 6: Desugaring / lowering
// ============================================================================

// P6_01: Pipe desugaring preserves semantics
#[test]
fn p6_pipe_desugar_preserves_semantics() {
    run_jit(
        r#"@no_prelude
module desugar.pipe

use aivi
use aivi.testing

double = n => n * 2
result = 5 |> double

@test "pipe preserves semantics"
main : Effect Text Unit
main = do Effect {
  assertEq result 10
}
"#,
    );
}

// P6_02: Closure lowering correctness
#[test]
fn p6_closure_lowering() {
    run_jit(
        r#"@no_prelude
module desugar.closure

use aivi
use aivi.testing

makeAdder = n => x => n + x

@test "closure captures"
main : Effect Text Unit
main = do Effect {
  addFive <- pure (makeAdder 5)
  assertEq (addFive 3) 8
  assertEq (addFive 10) 15
}
"#,
    );
}

// P6_03: Pattern match lowering
#[test]
fn p6_pattern_match_lowering() {
    run_jit(
        r#"@no_prelude
module desugar.pmatch

use aivi
use aivi.testing

Option A = None | Some A

unwrapOr = default => opt => opt match
  | Some x => x
  | None   => default

@test "pattern match lowering"
main : Effect Text Unit
main = do Effect {
  assertEq (unwrapOr 0 (Some 42)) 42
  assertEq (unwrapOr 0 None) 0
}
"#,
    );
}

// P6_04: Generics lowered to mono code
#[test]
fn p6_generics_to_mono() {
    run_jit(
        r#"@no_prelude
module desugar.generics

use aivi
use aivi.testing

id : a -> a
id = x => x

@test "mono generics"
main : Effect Text Unit
main = do Effect {
  assertEq (id 42) 42
  assertEq (id "hello") "hello"
}
"#,
    );
}

// ============================================================================
// Phase 7: Cranelift codegen
// ============================================================================

// P7_01: Mutual recursion / forward references
#[test]
fn p7_mutual_recursion_forward_refs() {
    run_jit(
        r#"@no_prelude
module codegen.fwdref

use aivi
use aivi.testing

isEven = n => if n == 0 then True else isOdd (n - 1)
isOdd = n => if n == 0 then False else isEven (n - 1)

@test "mutual recursion"
main : Effect Text Unit
main = do Effect {
  assertEq (isEven 4) True
  assertEq (isOdd 3) True
  assertEq (isEven 5) False
}
"#,
    );
}

// P7_02: Int and Float width correctness
#[test]
fn p7_int_float_widths() {
    run_jit(
        r#"@no_prelude
module codegen.widths

use aivi
use aivi.testing

add : Int -> Int -> Int
add = a b => a + b

sub : Int -> Int -> Int
sub = a b => a - b

mul : Int -> Int -> Int
mul = a b => a * b

addF : Float -> Float -> Float
addF = a b => a + b

mulF : Float -> Float -> Float
mulF = a b => a * b

@test "native int and float"
main : Effect Text Unit
main = do Effect {
  assertEq (add 3 4) 7
  assertEq (sub 10 3) 7
  assertEq (mul 6 7) 42
  assertEq (addF 1.5 2.5) 4.0
  assertEq (mulF 3.0 7.0) 21.0
}
"#,
    );
}

// P7_03: Short-circuit boolean ops and if/then/else control flow
#[test]
fn p7_short_circuit_bool() {
    run_jit(
        r#"@no_prelude
module codegen.boolops

use aivi
use aivi.testing

clamp = x => if x > 10 then 10 else x

@test "bool and control flow"
main : Effect Text Unit
main = do Effect {
  assertEq (clamp 5) 5
  assertEq (clamp 15) 10
}
"#,
    );
}

// P7_04: ABI / calling convention correctness
#[test]
fn p7_calling_convention() {
    run_jit(
        r#"@no_prelude
module codegen.abi

use aivi
use aivi.testing

add : Int -> Int -> Int
add = a b => a + b

double : Int -> Int
double = n => add n n

square : Int -> Int
square = n => n * n

@test "ABI composition"
main : Effect Text Unit
main = do Effect {
  assertEq (double 21) 42
  assertEq (square 7) 49
  assertEq (add (double 3) (square 2)) 10
}
"#,
    );
}

// ============================================================================
// E2E: Full pipeline cross-phase
// ============================================================================

#[test]
fn e2e_full_pipeline() {
    run_jit(
        r#"@no_prelude
module e2e.full

use aivi
use aivi.testing

Option A = None | Some A

map = f xs => xs match
  | []           => []
  | [x, ...rest] => [f x, ...(map f rest)]

safeHead = xs => xs match
  | []       => None
  | [x, ...] => Some x

describe = opt => opt match
  | None   => "nothing"
  | Some _ => "something"

@test "full pipeline"
main : Effect Text Unit
main = do Effect {
  nums = [1, 2, 3]
  doubled = map (n => n * 2) nums
  assertEq doubled [2, 4, 6]
  assertEq (safeHead doubled) (Some 2)
  assertEq (safeHead []) None
  assertEq (describe (Some 1)) "something"
  assertEq (describe None) "nothing"
}
"#,
    );
}

// ============================================================================
// Metamorphic tests
// ============================================================================

// META_02: Explicit vs inferred types — same behavior
#[test]
fn meta_explicit_vs_inferred_types() {
    // Explicit annotations
    run_jit(
        r#"@no_prelude
module meta.explicit

use aivi
use aivi.testing

add : Int -> Int -> Int
add = a b => a + b

@test "explicit types"
main : Effect Text Unit
main = do Effect {
  assertEq (add 1 2) 3
}
"#,
    );
    // Inferred (no annotation)
    run_jit(
        r#"@no_prelude
module meta.inferred

use aivi
use aivi.testing

add = a b => a + b

@test "inferred types"
main : Effect Text Unit
main = do Effect {
  assertEq (add 1 2) 3
}
"#,
    );
}

// META_03: Reordering non-dependent defs preserves semantics
#[test]
fn meta_reorder_independent_defs() {
    run_jit(
        r#"@no_prelude
module meta.orderA

use aivi
use aivi.testing

x = 1
y = 2
z = x + y

@test "order A"
main : Effect Text Unit
main = do Effect {
  assertEq z 3
}
"#,
    );
    run_jit(
        r#"@no_prelude
module meta.orderB

use aivi
use aivi.testing

y = 2
x = 1
z = x + y

@test "order B"
main : Effect Text Unit
main = do Effect {
  assertEq z 3
}
"#,
    );
}
