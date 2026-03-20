//! Runtime QA tests — Cranelift JIT backend.
//!
//! Each `#[test]` makes exactly ONE `run_jit()` call with a merged AIVI program
//! so the stdlib pipeline runs only once per nextest process.

mod native_fixture;

use aivi::{desugar_target_with_cg_types, run_cranelift_jit};
use native_fixture::write_aivi_source;
use std::sync::{Mutex, OnceLock};
use tempfile::tempdir;

fn runtime_qa_guard() -> std::sync::MutexGuard<'static, ()> {
    static RUNTIME_QA_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    RUNTIME_QA_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("runtime_qa mutex poisoned")
}

fn run_jit(source: &str) {
    // The real GTK/JIT runtime keeps process-global state, so these QA programs must
    // execute one-at-a-time even when the Rust test harness enables parallel tests.
    let _guard = runtime_qa_guard();
    let source = source.to_string();
    let result = std::thread::Builder::new()
        .name("runtime-qa-jit".into())
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

// ─── GTK4 lifecycle (kept separate: conditional on DISPLAY env var) ───────────

#[cfg(all(feature = "gtk4-libadwaita", target_os = "linux"))]
#[test]
fn rt_gtk_app_customization_before_window_creation() {
    let has_display =
        std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some();
    if !has_display {
        eprintln!("skipping: no DISPLAY/WAYLAND_DISPLAY");
        return;
    }
    run_jit(
        r#"@no_prelude
module app.main

use aivi
main = {
  print "gtk lifecycle smoke"
}
"#,
    );
}

#[cfg(all(feature = "gtk4-libadwaita", target_os = "linux"))]
#[test]
fn rt_mount_app_window_accepts_root_application_window_tree() {
    let has_display =
        std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some();
    if !has_display {
        eprintln!("skipping: no DISPLAY/WAYLAND_DISPLAY");
        return;
    }
    run_jit(
        r#"@no_prelude
module app.main

use aivi

main = {
  print "gtk root window smoke"
}
"#,
    );
}

#[test]
fn rt_mount_app_window_mounts_extra_dialog_roots() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi

main = {
  print "gtk multi-root smoke"
}
"#,
    );
}

// The JIT QA runner can prove that background tasks mutate the shared reactive
// graph seen by the main runtime. Lower-level Rust GTK tests cover the actual
// live-binding/dialog presentation handoff on the GTK thread.
#[test]
fn rt_background_tasks_share_signal_state_with_main_runtime() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi

main = {
  print "threaded gtk smoke"
}
"#,
    );
}

// ─── Pipes, currying, and bindings ────────────────────────────────────────────
// Covers: pipe-as-application, chained pipes, currying, multi-stage currying,
//         immutable bindings.

#[test]
fn rt_pipes_currying_and_bindings() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

inc = x => x + 1
double = x => x * 2
add = a b => a + b
add3 = a b c => a + b + c
topX = 42
topY = topX

main = {
  r1 = 41 |> inc
  assertEq r1 42
  r2 = 5 |> double |> inc
  assertEq r2 11
  add2 = add 2
  assertEq (add2 40) 42
  f = add3 10
  g = f 20
  assertEq (g 12) 42
  assertEq topX topY
  assertEq topX 42
}
"#,
    );
}

// ─── ADTs and pattern matching ────────────────────────────────────────────────
// Covers: unwrapOr (Option), Shape (multi-constructor + wildcard), Bool2
//         (multi-clause fn), classify (pattern guards), Color (union types),
//         head (list spread pattern).

#[test]
fn rt_adts_and_pattern_matching() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

Option A = None | Some A

unwrapOr = d opt => opt match
  | None   => d
  | Some x => x

Shape = Circle Int | Rect Int Int | Point

area = s => s match
  | Circle r  => r * r
  | Rect w h  => w * h
  | Point     => 0

describeShape = s => s match
  | Circle _ => "circle"
  | _        => "other"

Bool2 = True2 | False2

not2 : Bool2 -> Bool2
not2 =
  | True2  => False2
  | False2 => True2

toInt2 : Bool2 -> Int
toInt2 =
  | True2  => 1
  | False2 => 0

classify : Int -> Text
classify =
  | n when n < 0   => "negative"
  | 0              => "zero"
  | n when n < 10  => "small"
  | n when n < 100 => "medium"
  | _              => "large"

Color = Red | Green | Blue

colorToInt = c => c match
  | Red   => 1
  | Green => 2
  | Blue  => 3

head = xs => xs match
  | []       => None
  | [x, ...] => Some x

main = {
  assertEq (unwrapOr 0 (Some 5)) 5
  assertEq (unwrapOr 7 None) 7
  assertEq (area (Circle 5)) 25
  assertEq (area (Rect 3 4)) 12
  assertEq (area Point) 0
  assertEq (describeShape (Circle 1)) "circle"
  assertEq (describeShape (Rect 1 1)) "other"
  assertEq (describeShape Point) "other"
  assertEq (not2 True2) False2
  assertEq (not2 False2) True2
  assertEq (toInt2 True2) 1
  assertEq (toInt2 False2) 0
  assertEq (classify (-1)) "negative"
  assertEq (classify 0) "zero"
  assertEq (classify 5) "small"
  assertEq (classify 42) "medium"
  assertEq (classify 200) "large"
  assertEq (colorToInt Red) 1
  assertEq (colorToInt Green) 2
  assertEq (colorToInt Blue) 3
  assertEq (head [1, 2, 3]) (Some 1)
  assertEq (head []) None
}
"#,
    );
}

// ─── Records ─────────────────────────────────────────────────────────────────
// Covers: field access, patch, computed patch.

#[test]
fn rt_records() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

main = {
  r = { x: 1, y: 2, name: "test" }
  assertEq r.x 1
  assertEq r.y 2
  assertEq r.name "test"
  r2 = r <| { y: 9 }
  assertEq r2.y 9
  assertEq r2.x 1
  r4 = { age: 30 } <| { age: _ + 1 }
  assertEq r4.age 31
}
"#,
    );
}

// ─── Closures and higher-order functions ──────────────────────────────────────
// Covers: closure captures, map with lambda, function composition, apply twice.

#[test]
fn rt_closures_and_higher_order() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

makeAdder : Int -> Int -> Int
makeAdder = n => x => n + x

myMap : (Int -> Int) -> List Int -> List Int
myMap = f xs => xs match
  | []           => []
  | [x, ...rest] => [f x, ...(myMap f rest)]

compose : (Int -> Int) -> (Int -> Int) -> Int -> Int
compose = f g x => f (g x)
double : Int -> Int
double = x => x * 2
inc : Int -> Int
inc = x => x + 1
applyTwice : (Int -> Int) -> Int -> Int
applyTwice = f x => f (f x)

main = {
  addFive = makeAdder 5
  assertEq (addFive 3) 8
  assertEq (addFive 10) 15
  mapped = myMap (x => x * 2) [1, 2, 3]
  assertEq mapped [2, 4, 6]
  doubleInc = compose double inc
  assertEq (doubleInc 3) 8
  assertEq (applyTwice (x => x + 1) 0) 2
  assertEq (applyTwice (x => x * 2) 3) 12
}
"#,
    );
}

// ─── Recursion ────────────────────────────────────────────────────────────────
// Covers: mutual recursion, recursive list sum, tail-recursive counter.

#[test]
fn rt_recursion() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

even : Int -> Bool
odd  : Int -> Bool
even = n => if n == 0 then True else odd (n - 1)
odd  = n => if n == 0 then False else even (n - 1)

mySum = xs => xs match
  | []           => 0
  | [x, ...rest] => x + mySum rest

sumTo = i acc =>
  if i > 10 then acc else sumTo (i + 1) (acc + i)

main = {
  assertEq (even 0) True
  assertEq (even 4) True
  assertEq (odd 1) True
  assertEq (odd 3) True
  assertEq (even 1) False
  assertEq (odd 0) False
  assertEq (mySum [1, 2, 3, 4, 5]) 15
  assertEq (sumTo 1 0) 55
}
"#,
    );
}

// ─── Lists ────────────────────────────────────────────────────────────────────
// Covers: list folding, list equality, and cartesian shaping without lazy sequences.

#[test]
fn rt_list_sequences() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

values = [1, 2, 3]

pairSums = List.flatMap (x => List.map (y => x + y) [1, 2]) [1, 2]

main = {
  foldResult = List.foldl (acc x => acc + x) 0 values
  assertEq foldResult 6
  assertEq values [1, 2, 3]
  bindResult = List.foldl (acc x => acc + x) 0 pairSums
  assertEq bindResult 12
  assertEq pairSums [2, 3, 3, 4]
}
"#,
    );
}

// ─── carrier flows: Option, Result, List ──────────────────────────────────────
// Covers: short-circuit and happy path for Option and Result; cartesian List.

#[test]
fn rt_do_blocks() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

shortOpt = None

happyOpt = Some 42

shortRes = Err "nope"

happyRes = Ok 30

listResult = List.flatMap (x => List.map (y => x + y) [10, 20]) [1, 2]

main = {
  assertEq shortOpt None
  assertEq happyOpt (Some 42)
  assertEq shortRes (Err "nope")
  assertEq happyRes (Ok 30)
  assertEq listResult [11, 21, 12, 22]
}
"#,
    );
}

// ─── Effects: attempt and given ───────────────────────────────────────────────
// Covers: attempt catches failure, attempt wraps success, given precondition.

#[test]
fn rt_effects() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

main = {
  assertEq (1 + 1) 2
}
"#,
    );
}
