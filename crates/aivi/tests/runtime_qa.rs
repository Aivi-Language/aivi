//! Runtime QA tests — Cranelift JIT backend.
//!
//! Tests runtime semantics: evaluation, effects, closures, generators,
//! pattern matching, ADTs, records, mutual recursion, do-blocks.
//! All tests execute via Cranelift JIT (the interpreter has been removed).
//!
//! Companion `.aivi` files in `integration-tests/runtime/` serve as
//! human-readable documentation of the same test scenarios.

mod native_fixture;

use aivi::{desugar_target_with_cg_types, run_cranelift_jit};
use native_fixture::write_aivi_source;
use tempfile::tempdir;

fn run_jit(source: &str) {
    let source = source.to_string();
    let result = std::thread::Builder::new()
        .name("runtime-qa-jit".into())
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

@test "gtk lifecycle ordering"
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

// ─── Core semantics: pipes, currying, application ───

#[test]
fn rt_pipe_is_application() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

inc = x => x + 1

@test "pipe is application"
main : Effect Text Unit
main = do Effect {
  result <- pure (41 |> inc)
  assertEq result 42
}
"#,
    );
}

#[test]
fn rt_chained_pipes() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

double = x => x * 2
inc = x => x + 1

@test "chained pipes"
main : Effect Text Unit
main = do Effect {
  result <- pure (5 |> double |> inc)
  assertEq result 11
}
"#,
    );
}

#[test]
fn rt_currying_partial_application() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

add = a b => a + b

@test "currying"
main : Effect Text Unit
main = do Effect {
  add2 <- pure (add 2)
  assertEq (add2 40) 42
}
"#,
    );
}

#[test]
fn rt_multi_stage_currying() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

add3 = a b c => a + b + c

@test "multi-stage currying"
main : Effect Text Unit
main = do Effect {
  f <- pure (add3 10)
  g <- pure (f 20)
  assertEq (g 12) 42
}
"#,
    );
}

#[test]
fn rt_immutable_bindings() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

x = 42
y = x

@test "immutable bindings"
main : Effect Text Unit
main = do Effect {
  assertEq x y
  assertEq x 42
}
"#,
    );
}

// ─── ADTs and pattern matching ───

#[test]
fn rt_adt_unwrap_or() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

Option A = None | Some A

unwrapOr = d opt => opt match
  | None   => d
  | Some x => x

@test "unwrapOr"
main : Effect Text Unit
main = do Effect {
  assertEq (unwrapOr 0 (Some 5)) 5
  assertEq (unwrapOr 7 None) 7
}
"#,
    );
}

#[test]
fn rt_multi_constructor_adt() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

Shape = Circle Int | Rect Int Int | Point

area = s => s match
  | Circle r  => r * r
  | Rect w h  => w * h
  | Point     => 0

@test "multi-constructor"
main : Effect Text Unit
main = do Effect {
  assertEq (area (Circle 5)) 25
  assertEq (area (Rect 3 4)) 12
  assertEq (area Point) 0
}
"#,
    );
}

#[test]
fn rt_wildcard_match() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

Shape = Circle Int | Rect Int Int | Point

describe = s => s match
  | Circle _ => "circle"
  | _        => "other"

@test "wildcard match"
main : Effect Text Unit
main = do Effect {
  assertEq (describe (Circle 1)) "circle"
  assertEq (describe (Rect 1 1)) "other"
  assertEq (describe Point) "other"
}
"#,
    );
}

// ─── Multi-clause functions ───

#[test]
fn rt_multi_clause_function() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

Bool2 = True2 | False2

not2 : Bool2 -> Bool2
not2 =
  | True2  => False2
  | False2 => True2

toInt : Bool2 -> Int
toInt =
  | True2  => 1
  | False2 => 0

@test "multi-clause"
main : Effect Text Unit
main = do Effect {
  assertEq (not2 True2) False2
  assertEq (not2 False2) True2
  assertEq (toInt True2) 1
  assertEq (toInt False2) 0
}
"#,
    );
}

// ─── Records ───

#[test]
fn rt_record_field_access() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "record field access"
main : Effect Text Unit
main = do Effect {
  r = { x: 1, y: 2, name: "test" }
  assertEq r.x 1
  assertEq r.y 2
  assertEq r.name "test"
}
"#,
    );
}

#[test]
fn rt_record_patch() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "record patch"
main : Effect Text Unit
main = do Effect {
  r = { x: 1, y: 2 }
  r2 = r <| { y: 9 }
  assertEq r2.y 9
  assertEq r2.x 1
}
"#,
    );
}

#[test]
fn rt_record_computed_patch() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "record computed patch"
main : Effect Text Unit
main = do Effect {
  r = { age: 30 }
  r2 = r <| { age: _ + 1 }
  assertEq r2.age 31
}
"#,
    );
}

// ─── Closures and higher-order functions ───

#[test]
fn rt_closure_captures() {
    run_jit(
        r#"@no_prelude
module app.main

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

#[test]
fn rt_map_lambda() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

map = f xs => xs match
  | []           => []
  | [x, ...rest] => [f x, ...(map f rest)]

@test "map with lambda"
main : Effect Text Unit
main = do Effect {
  result <- pure (map (x => x * 2) [1, 2, 3])
  assertEq result [2, 4, 6]
}
"#,
    );
}

#[test]
fn rt_function_composition() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

compose = f g x => f (g x)
double = x => x * 2
inc = x => x + 1

@test "function composition"
main : Effect Text Unit
main = do Effect {
  doubleInc <- pure (compose double inc)
  assertEq (doubleInc 3) 8
}
"#,
    );
}

#[test]
fn rt_apply_twice() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

applyTwice = f x => f (f x)

@test "apply twice"
main : Effect Text Unit
main = do Effect {
  assertEq (applyTwice (x => x + 1) 0) 2
  assertEq (applyTwice (x => x * 2) 3) 12
}
"#,
    );
}

// ─── Mutual recursion ───

#[test]
fn rt_mutual_recursion_even_odd() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

even : Int -> Bool
odd  : Int -> Bool

even = n => if n == 0 then True else odd (n - 1)
odd  = n => if n == 0 then False else even (n - 1)

@test "mutual recursion"
main : Effect Text Unit
main = do Effect {
  assertEq (even 0) True
  assertEq (even 4) True
  assertEq (even 10) True
  assertEq (odd 1) True
  assertEq (odd 3) True
  assertEq (odd 11) True
  assertEq (even 1) False
  assertEq (odd 0) False
}
"#,
    );
}

// ─── Generators ───

#[test]
fn rt_generator_fold() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

gen = generate {
  yield 10
  yield 20
  yield 30
}

@test "generator fold"
main : Effect Text Unit
main = do Effect {
  result <- pure (gen (a => b => a + b) 0)
  assertEq result 60
}
"#,
    );
}

#[test]
fn rt_generator_to_list() {
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

@test "generator to list"
main : Effect Text Unit
main = do Effect {
  assert (toList gen == [1, 2, 3])
}
"#,
    );
}

#[test]
fn rt_generator_bind() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

numbers = generate {
  yield 1
  yield 2
}

pairSums = generate {
  x <- numbers
  y <- numbers
  yield (x + y)
}

@test "generator bind"
main : Effect Text Unit
main = do Effect {
  result <- pure (pairSums (a => b => a + b) 0)
  assertEq result 12
}
"#,
    );
}

// ─── do-blocks: Option, Result, List ───

#[test]
fn rt_do_option_short_circuit() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "do option short-circuit"
main : Effect Text Unit
main = do Effect {
  result = do Option {
    a <- Some 10
    b <- None
    Some (a + b)
  }
  assertEq result None
}
"#,
    );
}

#[test]
fn rt_do_option_happy_path() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "do option happy path"
main : Effect Text Unit
main = do Effect {
  result = do Option {
    a <- Some 10
    b <- Some 20
    c <- Some 12
    Some (a + b + c)
  }
  assertEq result (Some 42)
}
"#,
    );
}

#[test]
fn rt_do_result_short_circuit() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "do result short-circuit"
main : Effect Text Unit
main = do Effect {
  result = do Result {
    a <- Ok 10
    b <- Err "nope"
    Ok (a + b)
  }
  assertEq result (Err "nope")
}
"#,
    );
}

#[test]
fn rt_do_result_happy_path() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "do result happy"
main : Effect Text Unit
main = do Effect {
  result = do Result {
    a <- Ok 10
    b <- Ok 20
    Ok (a + b)
  }
  assertEq result (Ok 30)
}
"#,
    );
}

#[test]
fn rt_do_list_cartesian() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "do list cartesian"
main : Effect Text Unit
main = do Effect {
  result = do List {
    x <- [1, 2]
    y <- [10, 20]
    [x + y]
  }
  assertEq result [11, 21, 12, 22]
}
"#,
    );
}

// ─── Effects: attempt, given ───

#[test]
fn rt_attempt_catches_failure() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "attempt catches"
main : Effect Text Unit
main = do Effect {
  res <- attempt (fail "oops")
  res match
    | Ok _  => fail "unexpected ok"
    | Err e => assertEq e "oops"
}
"#,
    );
}

#[test]
fn rt_attempt_wraps_success() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "attempt success"
main : Effect Text Unit
main = do Effect {
  res <- attempt (pure 42)
  res match
    | Ok v  => assertEq v 42
    | Err _ => fail "unexpected err"
}
"#,
    );
}

#[test]
fn rt_given_precondition() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

validatePositive = n => do Effect {
  given n > 0 or fail "must be positive"
  pure n
}

@test "given precondition"
main : Effect Text Unit
main = do Effect {
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

// ─── Recursion ───

#[test]
fn rt_recursive_list_sum() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

sum = xs => xs match
  | []           => 0
  | [x, ...rest] => x + sum rest

@test "recursive sum"
main : Effect Text Unit
main = do Effect {
  assertEq (sum [1, 2, 3, 4, 5]) 15
}
"#,
    );
}

#[test]
fn rt_tail_recursive_counter() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

sumTo = i acc =>
  if i > 10 then acc else sumTo (i + 1) (acc + i)

@test "tail recursion"
main : Effect Text Unit
main = do Effect {
  assertEq (sumTo 1 0) 55
}
"#,
    );
}

// ─── Guards in pattern matching ───

#[test]
fn rt_pattern_guards() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

classify =
  | n when n < 0   => "negative"
  | 0              => "zero"
  | n when n < 10  => "small"
  | n when n < 100 => "medium"
  | _              => "large"

@test "pattern guards"
main : Effect Text Unit
main = do Effect {
  assertEq (classify (-1)) "negative"
  assertEq (classify 0) "zero"
  assertEq (classify 5) "small"
  assertEq (classify 42) "medium"
  assertEq (classify 200) "large"
}
"#,
    );
}

// ─── List spread patterns ───

#[test]
fn rt_list_spread_pattern() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

Option A = None | Some A

head = xs => xs match
  | []       => None
  | [x, ...] => Some x

@test "list spread"
main : Effect Text Unit
main = do Effect {
  assertEq (head [1, 2, 3]) (Some 1)
  assertEq (head []) None
}
"#,
    );
}

// ─── ADT union types ───

#[test]
fn rt_union_types() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

Color = Red | Green | Blue

toInt = c => c match
  | Red   => 1
  | Green => 2
  | Blue  => 3

@test "union types"
main : Effect Text Unit
main = do Effect {
  assertEq (toInt Red) 1
  assertEq (toInt Green) 2
  assertEq (toInt Blue) 3
}
"#,
    );
}
