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
use aivi.testing
use aivi.ui.gtk4

main : Effect Text Unit
main = do Effect {
  appId <- appNew "com.aivi.regression.lifecycle"
  _ <- iconThemeAddSearchPath "."
  _ <- appSetCss appId ""
  windowId <- windowNew appId "AIVI GTK Regression" 320 180
  _ <- windowClose windowId
  pure Unit
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
use aivi.testing
use aivi.ui.gtk4

root = ~<gtk>
  <AdwApplicationWindow id="root-window" title="AIVI GTK Root" defaultWidth={320} defaultHeight={180}>
    <GtkBox orientation="vertical" spacing={8}>
      <GtkLabel label="Hello" />
    </GtkBox>
  </AdwApplicationWindow>
</gtk>

main : Effect Text Unit
main = do Effect {
  _ <- init Unit
  appId <- appNew "com.aivi.regression.root-window"
  windowId <- mountAppWindow appId [root]
  lookedUp <- widgetById "root-window"
  assertEq lookedUp windowId
  _ <- windowClose windowId
  pure Unit
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
use aivi.testing
use aivi.ui.gtk4

windowRoot = ~<gtk>
  <AdwApplicationWindow id="root-window" title="AIVI GTK Root" defaultWidth={320} defaultHeight={180}>
    <GtkBox orientation="vertical" spacing={8}>
      <GtkLabel label="Hello" />
    </GtkBox>
  </AdwApplicationWindow>
</gtk>

dialogRoot = ~<gtk>
  <AdwPreferencesDialog id="prefs-dialog" title="Preferences" open={True}>
    <AdwPreferencesPage title="General" />
  </AdwPreferencesDialog>
</gtk>

main : Effect Text Unit
main = do Effect {
  _ <- init Unit
  appId <- appNew "com.aivi.regression.multi-root-window"
  windowId <- mountAppWindow appId [windowRoot, dialogRoot]
  lookedUp <- widgetById "root-window"
  dialogId <- widgetById "prefs-dialog"
  assertEq lookedUp windowId
  assertEq False (dialogId == 0)
  _ <- windowClose windowId
  pure Unit
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
use aivi.concurrency
use aivi.reactive
use aivi.testing
use aivi.ui.gtk4

entryTextSignal = signal "before"

windowRoot = ~<gtk>
  <AdwApplicationWindow id="root-window" title="AIVI GTK Root" defaultWidth={320} defaultHeight={180}>
    <GtkBox orientation="vertical" spacing={8}>
      <GtkEntry id="threaded-entry" text={entryTextSignal} />
    </GtkBox>
  </AdwApplicationWindow>
</gtk>

main : Effect Text Unit
main = do Effect {
  _ <- init Unit
  appId <- appNew "com.aivi.regression.dialog-threading"
  (phaseTx, phaseRx) <- make "phase"
  windowId <- mountAppWindow appId [windowRoot]
  _ <- windowPresent windowId
  worker <- spawn do Effect {
    _ <- sleep 40
    _ = set entryTextSignal "after"
    _ <- sleep 200
    _ <- send phaseTx "updated"
    pure Unit
  }
  entryId <- widgetById "threaded-entry"
  phase <- timeoutWith 1000 "missing threaded GTK update" (recv phaseRx)
  phase match
    | Ok msg =>
        if msg == "updated" then pure Unit
        else fail "unexpected threaded GTK phase message"
    | Err _  => fail "threaded GTK phase receiver closed"
  assertEq False (entryId == 0)
  currentSignal <- pure (get entryTextSignal)
  if currentSignal == "after" then pure Unit
  else fail "threaded signal value did not update"
  _ <- worker.join
  _ <- windowClose windowId
  pure Unit
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

main : Effect Text Unit
main = do Effect {
  r1 <- pure (41 |> inc)
  assertEq r1 42
  r2 <- pure (5 |> double |> inc)
  assertEq r2 11
  add2 <- pure (add 2)
  assertEq (add2 40) 42
  f <- pure (add3 10)
  g <- pure (f 20)
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

main : Effect Text Unit
main = do Effect {
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

main : Effect Text Unit
main = do Effect {
  r = { x: 1, y: 2, name: "test" }
  assertEq r.x 1
  assertEq r.y 2
  assertEq r.name "test"
  r2 = r <| { y: 9 }
  assertEq r2.y 9
  assertEq r2.x 1
  r3 = { age: 30 }
  r4 = r3 <| { age: _ + 1 }
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

makeAdder = n => x => n + x

myMap = f xs => xs match
  | []           => []
  | [x, ...rest] => [f x, ...(myMap f rest)]

compose = f g x => f (g x)
double = x => x * 2
inc = x => x + 1
applyTwice = f x => f (f x)

main : Effect Text Unit
main = do Effect {
  addFive <- pure (makeAdder 5)
  assertEq (addFive 3) 8
  assertEq (addFive 10) 15
  mapped <- pure (myMap (x => x * 2) [1, 2, 3])
  assertEq mapped [2, 4, 6]
  doubleInc <- pure (compose double inc)
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

main : Effect Text Unit
main = do Effect {
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

// ─── Generators ───────────────────────────────────────────────────────────────
// Covers: generator fold, to list, and bind (cartesian product).

#[test]
fn rt_generators() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing
use aivi.generator

gen = generate {
  yield 1
  yield 2
  yield 3
}

numbers = generate {
  yield 1
  yield 2
}

pairSums = generate {
  x <- numbers
  y <- numbers
  yield (x + y)
}

main : Effect Text Unit
main = do Effect {
  foldResult <- pure (gen (a => b => a + b) 0)
  assertEq foldResult 6
  assert (toList gen == [1, 2, 3])
  bindResult <- pure (pairSums (a => b => a + b) 0)
  assertEq bindResult 12
}
"#,
    );
}

// ─── do-blocks: Option, Result, List ──────────────────────────────────────────
// Covers: short-circuit and happy path for Option and Result; cartesian List.

#[test]
fn rt_do_blocks() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

main : Effect Text Unit
main = do Effect {
  shortOpt = do Option {
    a <- Some 10
    b <- None
    Some (a + b)
  }
  assertEq shortOpt None
  happyOpt = do Option {
    a <- Some 10
    b <- Some 20
    c <- Some 12
    Some (a + b + c)
  }
  assertEq happyOpt (Some 42)
  shortRes = do Result {
    a <- Ok 10
    b <- Err "nope"
    Ok (a + b)
  }
  assertEq shortRes (Err "nope")
  happyRes = do Result {
    a <- Ok 10
    b <- Ok 20
    Ok (a + b)
  }
  assertEq happyRes (Ok 30)
  listResult = do List {
    x <- [1, 2]
    y <- [10, 20]
    [x + y]
  }
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

validatePositive = n => do Effect {
  given n > 0 or fail "must be positive"
  pure n
}

main : Effect Text Unit
main = do Effect {
  caught <- attempt (fail "oops")
  caught match
    | Ok _  => fail "unexpected ok"
    | Err e => assertEq e "oops"
  wrapped <- attempt (pure 42)
  wrapped match
    | Ok v  => assertEq v 42
    | Err _ => fail "unexpected err"
  ok <- attempt (validatePositive 5)
  ok match
    | Ok v  => assertEq v 5
    | Err _ => fail "unexpected error"
  bad <- attempt (validatePositive (-1))
  bad match
    | Err e => assertEq e "must be positive"
    | Ok _  => fail "should have failed"
}
"#,
    );
}
