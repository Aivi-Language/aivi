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
            let (program, cg_types, _monomorph_plan) =
                desugar_target_with_cg_types(&source_path_str).expect("desugar");
            run_cranelift_jit(program, cg_types, _monomorph_plan, &[]).expect("cranelift jit");
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
fn cranelift_jit_stdlib_list_find_calls_builtin() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing
use aivi.list as List

@test "stdlib list find"
main : Effect Text Unit
main = do Effect {
  result <- pure (List.find (x => x == 2) [1, 2, 3])
  result match
    | Some v => assertEq v 2
    | None   => fail "expected Some"
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
fn cranelift_jit_match_expression() {
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
fn cranelift_jit_generate_with_bind() {
    // Covers generate-bind semantics currently delegated through runtime helpers.
    // This test stays as parity coverage while bind lowering is migrated natively.
    run_jit(
        r#"@no_prelude
module app.main

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

@test "generate with bind"
main : Effect Text Unit
main = do Effect {
  result <- pure (pairSums (a => b => a + b) 0)
  assertEq result 12
}
"#,
    );
}

#[test]
fn cranelift_jit_resource_block() {
    // Resource blocks are preserved in the RustIR (not kernel-desugared).
    // The JIT constructs a Resource value via runtime helpers.
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

#[test]
fn cranelift_jit_typed_arithmetic() {
    // Verifies that typed Int arithmetic compiles to native iadd/isub/imul/sdiv
    // when both operands are known Int, avoiding rt_binary_op.
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

add : Int -> Int -> Int
add = a b => a + b

sub : Int -> Int -> Int
sub = a b => a - b

mul : Int -> Int -> Int
mul = a b => a * b

@test "native int arithmetic"
main : Effect Text Unit
main = do Effect {
  assertEq (add 3 4) 7
  assertEq (sub 10 3) 7
  assertEq (mul 6 7) 42
}
"#,
    );
}

#[test]
fn cranelift_jit_literal_arithmetic() {
    // Literal-to-literal arithmetic should use native iadd
    // since both operands are known Int from lower_lit_number.
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

result = 3 + 4

@test "literal arithmetic"
main : Effect Text Unit
main = do Effect {
  assertEq result 7
}
"#,
    );
}

#[test]
fn cranelift_jit_typed_float_arithmetic() {
    // Verifies typed Float params get unboxed at function entry and
    // native fadd/fmul are used.
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

addF : Float -> Float -> Float
addF = a b => a + b

mulF : Float -> Float -> Float
mulF = a b => a * b

@test "native float arithmetic"
main : Effect Text Unit
main = do Effect {
  assertEq (addF 1.5 2.5) 4.0
  assertEq (mulF 3.0 7.0) 21.0
}
"#,
    );
}

#[test]
fn cranelift_jit_typed_function_composition() {
    // Tests function composition: double calls add, both with typed Int signatures.
    // Verifies that ensure_boxed at function return correctly re-boxes for the caller.
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

@test "typed function composition"
main : Effect Text Unit
main = do Effect {
  assertEq (double 21) 42
  assertEq (square 7) 49
  assertEq (add (double 3) (square 2)) 10
}
"#,
    );
}

#[test]
fn cranelift_jit_monomorph_plan_records_polymorphic_calls() {
    // Verifies that calling a polymorphic function with concrete types
    // records the instantiation in the monomorph plan.
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

    // `id` is polymorphic (forall a. a -> a).
    // Called with Int (id 42) and Text (id "hello"), so the monomorph plan should
    // contain concrete instantiations for the qualified name `app.main.id`.
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

#[test]
fn cranelift_jit_monomorphized_polymorphic_function() {
    // End-to-end: a polymorphic `id` function is monomorphized for Int
    // and Text, and the JIT produces correct results.
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

id : a -> a
id = x => x

main : Effect Text Unit
main = do Effect {
  assertEq (id 42) 42
  assertEq (id "hello") "hello"
}
"#,
    );
}

#[test]
fn cranelift_aot_compile_to_object() {
    // Verify that compile_to_object produces valid ELF/object bytes.
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

    // Basic sanity: ELF magic number (Linux) or Mach-O / COFF header
    assert!(
        object_bytes.len() > 64,
        "object file too small: {} bytes",
        object_bytes.len()
    );
    // ELF magic: 0x7f 'E' 'L' 'F'
    assert_eq!(&object_bytes[..4], b"\x7fELF", "expected ELF header");
}

// ─── Additional coverage: tuple operations ───

#[test]
fn cranelift_jit_tuple_creation_and_destructuring() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "tuple destructuring"
main : Effect Text Unit
main = do Effect {
  t = (1, "hello", True)
  (a, b, c) = t
  assertEq a 1
  assertEq b "hello"
  assertEq c True
}
"#,
    );
}

#[test]
fn cranelift_jit_tuple_in_function() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

swap = (a, b) => (b, a)

@test "tuple swap"
main : Effect Text Unit
main = do Effect {
  assertEq (swap (1, 2)) (2, 1)
}
"#,
    );
}

// ─── Additional coverage: list spread ───

#[test]
fn cranelift_jit_list_spread_in_construction() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "list spread"
main : Effect Text Unit
main = do Effect {
  xs = [1, 2, 3]
  ys = [0, ...xs, 4]
  assertEq ys [0, 1, 2, 3, 4]
}
"#,
    );
}

// ─── Additional coverage: string equality ───

#[test]
fn cranelift_jit_string_equality() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "string equality"
main : Effect Text Unit
main = do Effect {
  assertEq "hello" "hello"
  assert ("hello" != "world")
}
"#,
    );
}

// ─── Additional coverage: boolean conditionals ───

#[test]
fn cranelift_jit_boolean_conditionals() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

negate : Bool -> Bool
negate = b => if b then False else True

@test "boolean conditionals"
main : Effect Text Unit
main = do Effect {
  assertEq (negate True) False
  assertEq (negate False) True
}
"#,
    );
}

// ─── Additional coverage: int modulo ───

#[test]
fn cranelift_jit_int_modulo() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

mod : Int -> Int -> Int
mod = a b => a % b

@test "int modulo"
main : Effect Text Unit
main = do Effect {
  assertEq (mod 17 5) 2
  assertEq (mod 10 3) 1
  assertEq (mod 20 4) 0
}
"#,
    );
}

// ─── Additional coverage: float division ───

#[test]
fn cranelift_jit_float_division() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

divF : Float -> Float -> Float
divF = a b => a / b

@test "float division"
main : Effect Text Unit
main = do Effect {
  assertEq (divF 10.0 2.0) 5.0
  assertEq (divF 7.0 2.0) 3.5
}
"#,
    );
}

// ─── Additional coverage: nested constructor matching ───

#[test]
fn cranelift_jit_nested_constructor_match() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

Option A = None | Some A

@test "nested option match"
main : Effect Text Unit
main = do Effect {
  x = Some (Some 42)
  v = x match
    | Some (Some n) => n
    | Some None     => 0
    | None          => -1
  assertEq v 42
}
"#,
    );
}

// ─── Additional coverage: multi-arg constructors ───

#[test]
fn cranelift_jit_multi_arg_constructor() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

Tree A = Leaf | Node (Tree A) A (Tree A)

treeSum : Tree Int -> Int
treeSum = t => t match
  | Leaf       => 0
  | Node l v r => treeSum l + v + treeSum r

@test "tree traversal"
main : Effect Text Unit
main = do Effect {
  t = Node (Node Leaf 1 Leaf) 2 (Node Leaf 3 Leaf)
  assertEq (treeSum t) 6
}
"#,
    );
}

// ─── Additional coverage: literal int patterns ───

#[test]
fn cranelift_jit_literal_int_pattern() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

describe = x => x match
  | 0 => "zero"
  | 1 => "one"
  | _ => "other"

@test "literal int pattern"
main : Effect Text Unit
main = do Effect {
  assertEq (describe 0) "zero"
  assertEq (describe 1) "one"
  assertEq (describe 99) "other"
}
"#,
    );
}

// ─── Additional coverage: literal string patterns ───

#[test]
fn cranelift_jit_literal_string_pattern() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

greet = name => name match
  | "Alice" => "Hi Alice!"
  | "Bob"   => "Hey Bob!"
  | _       => "Hello!"

@test "literal string pattern"
main : Effect Text Unit
main = do Effect {
  assertEq (greet "Alice") "Hi Alice!"
  assertEq (greet "Bob") "Hey Bob!"
  assertEq (greet "Charlie") "Hello!"
}
"#,
    );
}

// ─── Additional coverage: record computed update ───

#[test]
fn cranelift_jit_record_computed_update() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "record computed update"
main : Effect Text Unit
main = do Effect {
  r = { count: 10 }
  r2 = r <| { count: _ + 5 }
  assertEq r2.count 15
}
"#,
    );
}

// ─── Additional coverage: many-param functions (arity 6+) ───

#[test]
fn cranelift_jit_six_param_function() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

add6 : Int -> Int -> Int -> Int -> Int -> Int -> Int
add6 = a b c d e f => a + b + c + d + e + f

@test "six param function"
main : Effect Text Unit
main = do Effect {
  assertEq (add6 1 2 3 4 5 6) 21
}
"#,
    );
}

// ─── Additional coverage: deep closure capture ───

#[test]
fn cranelift_jit_deep_closure_capture() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "deep closure capture"
main : Effect Text Unit
main = do Effect {
  a = 1
  b = 2
  c = 3
  f = x => y => z => a + b + c + x + y + z
  g = f 4
  h = g 5
  assertEq (h 6) 21
}
"#,
    );
}

// ─── Additional coverage: float subtraction ───

#[test]
fn cranelift_jit_float_subtraction() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

subF : Float -> Float -> Float
subF = a b => a - b

@test "float subtraction"
main : Effect Text Unit
main = do Effect {
  assertEq (subF 10.0 3.5) 6.5
  assertEq (subF 0.0 1.0) (-1.0)
}
"#,
    );
}

// ─── Additional coverage: float comparisons ───

#[test]
fn cranelift_jit_float_comparisons() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "float comparisons"
main : Effect Text Unit
main = do Effect {
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

// ─── Additional coverage: complex equality ───

#[test]
fn cranelift_jit_complex_equality() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "complex equality"
main : Effect Text Unit
main = do Effect {
  assert ([1, 2, 3] == [1, 2, 3])
  assert ([1, 2] != [1, 2, 3])
  r1 = { x: 1, y: 2 }
  r2 = { x: 1, y: 2 }
  assert (r1 == r2)
}
"#,
    );
}

// ─── Additional coverage: generator to list ───

#[test]
fn cranelift_jit_generator_to_list_extended() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing
use aivi.generator

@test "generator to list"
main : Effect Text Unit
main = do Effect {
  g = generate {
    yield 10
    yield 20
    yield 30
    yield 40
    yield 50
  }
  assertEq (toList g) [10, 20, 30, 40, 50]
}
"#,
    );
}

// ─── Additional coverage: effect attempt chain ───

#[test]
fn cranelift_jit_effect_attempt_chain() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "effect attempt chain"
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
}
"#,
    );
}

// ─── Additional coverage: do Option chain ───

#[test]
fn cranelift_jit_do_option_chain() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "do option chain"
main : Effect Text Unit
main = do Effect {
  result = do Option {
    a <- Some 1
    b <- Some 2
    c <- Some 3
    d <- Some 4
    Some (a + b + c + d)
  }
  assertEq result (Some 10)
}
"#,
    );
}

// ─── Additional coverage: typed and untyped mix ───

#[test]
fn cranelift_jit_typed_and_untyped_mix() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

addInt : Int -> Int -> Int
addInt = a b => a + b

addUntyped = a b => a + b

@test "typed and untyped mix"
main : Effect Text Unit
main = do Effect {
  assertEq (addInt 3 4) 7
  assertEq (addUntyped 3 4) 7
}
"#,
    );
}

// ─── Additional coverage: recursive list operations ───

#[test]
fn cranelift_jit_recursive_filter() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

filter : (A -> Bool) -> List A -> List A
filter = pred xs => xs match
  | []           => []
  | [x, ...rest] => if pred x then [x, ...(filter pred rest)] else filter pred rest

@test "recursive filter"
main : Effect Text Unit
main = do Effect {
  evens = filter (x => x % 2 == 0) [1, 2, 3, 4, 5, 6]
  assertEq evens [2, 4, 6]
}
"#,
    );
}

// ─── Additional coverage: recursive foldl ───

#[test]
fn cranelift_jit_recursive_foldl() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

foldl : (B -> A -> B) -> B -> List A -> B
foldl = f acc xs => xs match
  | []           => acc
  | [x, ...rest] => foldl f (f acc x) rest

@test "recursive foldl"
main : Effect Text Unit
main = do Effect {
  result = foldl (acc x => acc + x) 0 [1, 2, 3, 4, 5]
  assertEq result 15
}
"#,
    );
}

// ─── Additional coverage: nested if/else ───

#[test]
fn cranelift_jit_nested_if_else() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

classify = n => if n < 0 then "neg" else if n == 0 then "zero" else "pos"

@test "nested if else"
main : Effect Text Unit
main = do Effect {
  assertEq (classify (-5)) "neg"
  assertEq (classify 0) "zero"
  assertEq (classify 5) "pos"
}
"#,
    );
}

// ─── Additional coverage: record with many fields ───

#[test]
fn cranelift_jit_record_many_fields() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "record many fields"
main : Effect Text Unit
main = do Effect {
  r = { a: 1, b: 2, c: 3, d: 4, e: 5 }
  assertEq r.a 1
  assertEq r.b 2
  assertEq r.c 3
  assertEq r.d 4
  assertEq r.e 5
}
"#,
    );
}

// ─── Additional coverage: closure captures multiple values ───

#[test]
fn cranelift_jit_closure_multi_capture() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

@test "closure multi capture"
main : Effect Text Unit
main = do Effect {
  a = 10
  b = 20
  f = x => a + b + x
  assertEq (f 12) 42
}
"#,
    );
}

// ─── Additional coverage: result ADT ───

#[test]
fn cranelift_jit_result_adt() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

Result E A = Ok A | Err E

getOrDefault = d r => r match
  | Ok x  => x
  | Err _ => d

@test "result adt"
main : Effect Text Unit
main = do Effect {
  assertEq (getOrDefault 0 (Ok 42)) 42
  assertEq (getOrDefault 0 (Err "fail")) 0
}
"#,
    );
}

// ─── Additional coverage: factorial with Int types ───

#[test]
fn cranelift_jit_factorial() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

factorial : Int -> Int
factorial = n => if n <= 1 then 1 else n * factorial (n - 1)

@test "factorial"
main : Effect Text Unit
main = do Effect {
  assertEq (factorial 0) 1
  assertEq (factorial 1) 1
  assertEq (factorial 5) 120
  assertEq (factorial 10) 3628800
}
"#,
    );
}

// ─── Additional coverage: list head-tail destructuring ───

#[test]
fn cranelift_jit_list_head_tail() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

myLen = xs => xs match
  | []           => 0
  | [_, ...rest] => 1 + myLen rest

@test "list length recursive"
main : Effect Text Unit
main = do Effect {
  assertEq (myLen []) 0
  assertEq (myLen [1]) 1
  assertEq (myLen [1, 2, 3]) 3
}
"#,
    );
}

// ─── Additional coverage: ADT with many constructors ───

#[test]
fn cranelift_jit_many_constructors() {
    run_jit(
        r#"@no_prelude
module app.main

use aivi
use aivi.testing

Color = Red | Green | Blue | Yellow | Cyan | Magenta

colorToInt : Color -> Int
colorToInt =
  | Red     => 1
  | Green   => 2
  | Blue    => 3
  | Yellow  => 4
  | Cyan    => 5
  | Magenta => 6

@test "many constructors"
main : Effect Text Unit
main = do Effect {
  assertEq (colorToInt Red) 1
  assertEq (colorToInt Green) 2
  assertEq (colorToInt Blue) 3
  assertEq (colorToInt Yellow) 4
  assertEq (colorToInt Cyan) 5
  assertEq (colorToInt Magenta) 6
}
"#,
    );
}
