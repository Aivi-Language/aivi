//! Smoke tests for the Cranelift JIT backend.
//!
//! Each `#[test]` makes exactly ONE `run_jit()` call with a merged AIVI program
//! that covers all sub-cases, so the stdlib parse → typecheck → infer → desugar
//! pipeline runs only once per nextest process.

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

// ─── Canary ──────────────────────────────────────────────────────────────────

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

// ─── Arithmetic ──────────────────────────────────────────────────────────────
// Covers: typed Int, literal, typed Float, int modulo, float ops, comparisons.

#[test]
fn cranelift_jit_arithmetic() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

addInt : Int -> Int -> Int
addInt = a b => a + b

subInt : Int -> Int -> Int
subInt = a b => a - b

mulInt : Int -> Int -> Int
mulInt = a b => a * b

modInt : Int -> Int -> Int
modInt = a b => a % b

addF : Float -> Float -> Float
addF = a b => a + b

mulF : Float -> Float -> Float
mulF = a b => a * b

subF : Float -> Float -> Float
subF = a b => a - b

divF : Float -> Float -> Float
divF = a b => a / b

main : Effect Text Unit
main = do Effect {
  assertEq (addInt 3 4) 7
  assertEq (subInt 10 3) 7
  assertEq (mulInt 6 7) 42
  assertEq (3 + 4) 7
  assertEq (addF 1.5 2.5) 4.0
  assertEq (mulF 3.0 7.0) 21.0
  assertEq (subF 10.0 3.5) 6.5
  assertEq (subF 0.0 1.0) (-1.0)
  assertEq (divF 10.0 2.0) 5.0
  assertEq (divF 7.0 2.0) 3.5
  assertEq (modInt 17 5) 2
  assertEq (modInt 10 3) 1
  assertEq (modInt 20 4) 0
  assert (1.5 < 2.5)
  assert (3.0 > 2.0)
  assert (2.0 <= 2.0)
  assert (2.0 >= 2.0)
  assert (3.14 == 3.14)
  assert (3.14 != 2.71)
}
"#,
    );
}

// ─── Control flow ─────────────────────────────────────────────────────────────
// Covers: if expression, nested if/else, boolean conditionals.

#[test]
fn cranelift_jit_control_flow() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

clamp = x => if x > 10 then 10 else x

classify = n => if n < 0 then "neg" else if n == 0 then "zero" else "pos"

negate : Bool -> Bool
negate = b => if b then False else True

main : Effect Text Unit
main = do Effect {
  assertEq (clamp 5) 5
  assertEq (clamp 15) 10
  assertEq (classify (-5)) "neg"
  assertEq (classify 0) "zero"
  assertEq (classify 5) "pos"
  assertEq (negate True) False
  assertEq (negate False) True
}
"#,
    );
}

// ─── Functions, closures, polymorphism ────────────────────────────────────────
// Covers: typed Int, typed Float composition, typed/untyped mix, lambda closures,
//         multi-capture, deep capture, six-param, monomorphized polymorphic fn.

#[test]
fn cranelift_jit_functions_and_closures() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

add : Int -> Int -> Int
add = a b => a + b

double : Int -> Int
double = n => add n n

square : Int -> Int
square = n => n * n

addInt : Int -> Int -> Int
addInt = a b => a + b

addUntyped = a b => a + b

add6 : Int -> Int -> Int -> Int -> Int -> Int -> Int
add6 = a b c d e f => a + b + c + d + e + f

makeAdder = n => x => n + x

id : a -> a
id = x => x

main : Effect Text Unit
main = do Effect {
  result <- pure (add 3 4)
  assertEq result 7
  assertEq (double 21) 42
  assertEq (square 7) 49
  assertEq (add (double 3) (square 2)) 10
  assertEq (addInt 3 4) 7
  assertEq (addUntyped 3 4) 7
  assertEq (add6 1 2 3 4 5 6) 21
  addFive <- pure (makeAdder 5)
  assertEq (addFive 3) 8
  assertEq (addFive 10) 15
  ca = 10
  cb = 20
  f = x => ca + cb + x
  assertEq (f 12) 42
  c1 = 1
  c2 = 2
  c3 = 3
  g = x => y => z => c1 + c2 + c3 + x + y + z
  h = g 4
  k = h 5
  assertEq (k 6) 21
  assertEq (id 42) 42
  assertEq (id "hello") "hello"
}
"#,
    );
}

// ─── ADTs and pattern matching ────────────────────────────────────────────────
// Covers: Option, nested constructors, multi-arg (Tree), literal int/string
//         patterns, many constructors (Color), Result ADT, factorial.

#[test]
fn cranelift_jit_adts_and_matching() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

Option A = None | Some A

unwrapOr = default => opt => opt match
  | Some x => x
  | None   => default

safeHead = xs => xs match
  | []       => None
  | [x, ...] => Some x

Tree A = Leaf | Node (Tree A) A (Tree A)

treeSum : Tree Int -> Int
treeSum = t => t match
  | Leaf       => 0
  | Node l v r => treeSum l + v + treeSum r

describeNum = x => x match
  | 0 => "zero"
  | 1 => "one"
  | _ => "other"

greet = name => name match
  | "Alice" => "Hi Alice!"
  | "Bob"   => "Hey Bob!"
  | _       => "Hello!"

Color = Red | Green | Blue | Yellow | Cyan | Magenta

colorToInt : Color -> Int
colorToInt =
  | Red     => 1
  | Green   => 2
  | Blue    => 3
  | Yellow  => 4
  | Cyan    => 5
  | Magenta => 6

Result E A = Ok A | Err E

getOrDefault = d r => r match
  | Ok x  => x
  | Err _ => d

factorial : Int -> Int
factorial = n => if n <= 1 then 1 else n * factorial (n - 1)

main : Effect Text Unit
main = do Effect {
  assertEq (unwrapOr 0 (Some 42)) 42
  assertEq (unwrapOr 0 None) 0
  assertEq (safeHead [1, 2, 3]) (Some 1)
  assertEq (safeHead []) None
  nested = Some (Some 42)
  v = nested match
    | Some (Some n) => n
    | Some None     => 0
    | None          => -1
  assertEq v 42
  t = Node (Node Leaf 1 Leaf) 2 (Node Leaf 3 Leaf)
  assertEq (treeSum t) 6
  assertEq (describeNum 0) "zero"
  assertEq (describeNum 1) "one"
  assertEq (describeNum 99) "other"
  assertEq (greet "Alice") "Hi Alice!"
  assertEq (greet "Bob") "Hey Bob!"
  assertEq (greet "Charlie") "Hello!"
  assertEq (colorToInt Red) 1
  assertEq (colorToInt Green) 2
  assertEq (colorToInt Blue) 3
  assertEq (colorToInt Yellow) 4
  assertEq (colorToInt Cyan) 5
  assertEq (colorToInt Magenta) 6
  assertEq (getOrDefault 0 (Ok 42)) 42
  assertEq (getOrDefault 0 (Err "fail")) 0
  assertEq (factorial 0) 1
  assertEq (factorial 1) 1
  assertEq (factorial 5) 120
  assertEq (factorial 10) 3628800
}
"#,
    );
}

// ─── Records ─────────────────────────────────────────────────────────────────
// Covers: patching, computed update, many fields.

#[test]
fn cranelift_jit_records() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

updateAge = person => person <| { age: 99 }

main : Effect Text Unit
main = do Effect {
  result <- pure (updateAge { name: "Bob", age: 30 })
  assertEq result { name: "Bob", age: 99 }
  r = { count: 10 }
  r2 = r <| { count: _ + 5 }
  assertEq r2.count 15
  rf = { a: 1, b: 2, c: 3, d: 4, e: 5 }
  assertEq rf.a 1
  assertEq rf.b 2
  assertEq rf.c 3
  assertEq rf.d 4
  assertEq rf.e 5
}
"#,
    );
}

// ─── Lists, tuples, recursion, stdlib ────────────────────────────────────────
// Covers: tuple creation/destructuring/swap, list spread, head-tail recursion,
//         complex equality, string equality, recursive filter/foldl,
//         stdlib list.find.

#[test]
fn cranelift_jit_lists_tuples_and_recursion() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing
use aivi.list as List

swap = (a, b) => (b, a)

myLen = xs => xs match
  | []           => 0
  | [_, ...rest] => 1 + myLen rest

filterEvens : (A -> Bool) -> List A -> List A
filterEvens = pred xs => xs match
  | []           => []
  | [x, ...rest] => if pred x then [x, ...(filterEvens pred rest)] else filterEvens pred rest

myFoldl : (B -> A -> B) -> B -> List A -> B
myFoldl = f acc xs => xs match
  | []           => acc
  | [x, ...rest] => myFoldl f (f acc x) rest

main : Effect Text Unit
main = do Effect {
  t = (1, "hello", True)
  (ta, tb, tc) = t
  assertEq ta 1
  assertEq tb "hello"
  assertEq tc True
  assertEq (swap (1, 2)) (2, 1)
  xs = [1, 2, 3]
  ys = [0, ...xs, 4]
  assertEq ys [0, 1, 2, 3, 4]
  assertEq (myLen []) 0
  assertEq (myLen [1]) 1
  assertEq (myLen [1, 2, 3]) 3
  assert ([1, 2, 3] == [1, 2, 3])
  assert ([1, 2] != [1, 2, 3])
  r1 = { x: 1, y: 2 }
  r2 = { x: 1, y: 2 }
  assert (r1 == r2)
  assertEq "hello" "hello"
  assert ("hello" != "world")
  evens = filterEvens (x => x % 2 == 0) [1, 2, 3, 4, 5, 6]
  assertEq evens [2, 4, 6]
  foldResult = myFoldl (acc x => acc + x) 0 [1, 2, 3, 4, 5]
  assertEq foldResult 15
  found <- pure (List.find (x => x == 2) [1, 2, 3])
  found match
    | Some v => assertEq v 2
    | None   => fail "expected Some"
}
"#,
    );
}

// ─── Generators ───────────────────────────────────────────────────────────────
// Covers: generate block (Church fold), generate-bind, generator to list.

#[test]
fn cranelift_jit_generators() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing
use aivi.generator

gen = generate {
  yield 10
  yield 20
  yield 30
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

bigGen = generate {
  yield 10
  yield 20
  yield 30
  yield 40
  yield 50
}

main : Effect Text Unit
main = do Effect {
  foldResult <- pure (gen (a => b => a + b) 0)
  assertEq foldResult 60
  bindResult <- pure (pairSums (a => b => a + b) 0)
  assertEq bindResult 12
  assertEq (toList bigGen) [10, 20, 30, 40, 50]
}
"#,
    );
}

// ─── Effects ──────────────────────────────────────────────────────────────────
// Covers: attempt chain, do Option, resource block.

#[test]
fn cranelift_jit_effects() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

managedResource = name => resource {
  yield name
}

main : Effect Text Unit
main = do Effect {
  res1 <- attempt (pure 42)
  res2 <- attempt (fail "oops")
  v1 = res1 match
    | Ok v  => v
    | Err _ => 0
  v2 = res2 match
    | Ok _  => 0
    | Err e => 1
  assertEq v1 42
  assertEq v2 1
  optResult = do Option {
    a <- Some 1
    b <- Some 2
    c <- Some 3
    d <- Some 4
    Some (a + b + c + d)
  }
  assertEq optResult (Some 10)
  assertEq (1 + 1) 2
}
"#,
    );
}

// ─── Monomorph plan (internal inspection) ─────────────────────────────────────

#[test]
fn cranelift_jit_monomorph_plan_records_polymorphic_calls() {
    use aivi::desugar_target_with_cg_types;
    use native_fixture::write_aivi_source;
    use tempfile::tempdir;

    let dir = tempdir().expect("tempdir");
    let source = r#"@no_prelude
module app.main

use aivi

id : a -> a
id = x => x

main : Effect Text Unit
main = do Effect {
  result <- pure (id 42)
  print (id "hello")
}
"#;
    let source_path = write_aivi_source(dir.path(), "main.aivi", source);
    let (_program, _cg_types, monomorph_plan) =
        desugar_target_with_cg_types(&source_path).expect("desugar");

    let key = "app.main.id";
    assert!(
        monomorph_plan.contains_key(key),
        "monomorph_plan should contain {key}, got keys: {:?}",
        monomorph_plan.keys().collect::<Vec<_>>()
    );
    let instantiations = &monomorph_plan[key];
    assert!(
        instantiations.len() >= 2,
        "expected at least 2 instantiations for `id`, got {}: {:?}",
        instantiations.len(),
        instantiations
    );
}

// ─── AOT compile to object ────────────────────────────────────────────────────

#[test]
fn cranelift_aot_compile_to_object() {
    use aivi::{compile_to_object, desugar_target_with_cg_types_and_surface};
    use native_fixture::write_aivi_source;
    use tempfile::tempdir;

    let dir = tempdir().expect("tempdir");
    let source = r#"module app.main

main : Effect Text Unit
main = do Effect {
  print "Hello from AOT!"
}
"#;
    let source_path = write_aivi_source(dir.path(), "main.aivi", source);
    let (program, cg_types, monomorph_plan, surface_modules) =
        desugar_target_with_cg_types_and_surface(&source_path).expect("desugar");
    let object_bytes = compile_to_object(program, cg_types, monomorph_plan, &surface_modules)
        .expect("compile_to_object");

    assert!(
        object_bytes.len() > 64,
        "object file too small: {} bytes",
        object_bytes.len()
    );
    assert_eq!(&object_bytes[..4], b"\x7fELF", "expected ELF header");
}
