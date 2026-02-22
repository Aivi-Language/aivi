# Typed Codegen Design

## Problem Statement

The current native Rust backend wraps **every** value in a `Value` enum at runtime:
- Integers: `Value::Int(i64)` — requires tagging + heap for closures
- Records: `Value::Record(Arc<HashMap<String, Value>>)` — string key lookup every field access
- ADTs: `Value::Constructor { name: String, args: Vec<Value> }` — string tag dispatch, boxed args
- Function calls: `rt.apply(Value, Value)` — dynamic dispatch on `Value::Closure`/`Value::Builtin`/`Value::MultiClause`

This means even a simple `a + b` where the type checker **knows** both are `Int` generates a runtime match on `(Value::Int, ...)` branches.

## Solution: Typed Codegen Path

Add a parallel codegen path for **closed types** — types whose runtime representation is fully known at compile time. Fall back to the existing `Value`-boxed path only when the program actually needs open-world dynamism.

### What is a "closed type"?

A type is **closed** (a.k.a. *monomorphic* / *ground*) when it contains no unresolved type variables, no open record rows, and its runtime layout can be statically determined:

| Category | Examples | Rust emission |
|----------|----------|---------------|
| Primitives | `Int`, `Float`, `Bool`, `Text`, `Unit` | `i64`, `f64`, `bool`, `String`, `()` |
| Closed records | `{ x: Int, y: Float }` | Named struct with typed fields |
| ADTs (known constructors) | `Option Int`, `Result Text Int` | Rust `enum` with typed payloads |
| Concrete functions | `Int -> Bool` | `fn(i64) -> bool` (or `Fn(i64) -> bool`) |
| Tuples | `(Int, Text)` | `(i64, String)` |
| Lists of known elem type | `List Int` | `Vec<i64>` |

### What requires `Value` fallback?

| Situation | Reason |
|-----------|--------|
| Polymorphic function body | Type variable `'a` — concrete type unknown |
| Open record | `{ x: Int | r }` — unknown extra fields |
| Higher-kinded types | `f a` where `f` is a type variable |
| Dynamic dispatch (builtins) | Builtins operate on `Value` |
| Interop boundaries | Calling between typed and untyped code |

## Architecture

### Phase 1: `DefType` — lightweight type annotation for definitions

Add a `DefType` enum that describes the *resolved* type of each definition in a compact, codegen-friendly form. This is NOT the full `Type` enum from the type checker — it's a stripped-down representation.

```rust
/// Codegen-friendly type representation.
/// Closed = fully known at compile time. Open = needs Value boxing.
pub enum CgType {
    /// Unresolved / polymorphic — needs Value boxing
    Dynamic,
    /// Primitive types
    Int,
    Float,
    Bool,
    Text,
    Unit,
    DateTime,
    /// Function from A to B
    Func(Box<CgType>, Box<CgType>),
    /// Homogeneous list
    ListOf(Box<CgType>),
    /// Tuple with known element types
    Tuple(Vec<CgType>),
    /// Closed record with known field names and types, sorted by name
    Record(Vec<(String, CgType)>),
    /// ADT with known constructors and their payload types
    Adt {
        name: String,
        constructors: Vec<(String, Vec<CgType>)>,
    },
    /// Boxed Value — explicitly marked for interop
    Value,
}
```

### Phase 2: Type map from inference to codegen

Since the type checker operates on Surface AST and codegen operates on Rust IR, we need a bridge. The simplest approach: produce a **definition-level** type map.

```
infer_value_types() currently returns:
  HashMap<String, HashMap<String, String>>  // module → def → type STRING

New: also return structured types:
  HashMap<String, HashMap<String, CgType>>  // module → def → CgType
```

This doesn't require per-expression type annotation — we propagate from def types. Within a definition body, we use **local type propagation**: if `f : Int -> Bool` and we see `f x`, we know the result is `Bool` without a per-node type map.

### Phase 3: Typed expression emission

The codegen gets a `CgType` for each definition and propagates types through expressions:

| Expression | Typed emission (when possible) |
|------------|-------------------------------|
| `LitNumber "42"` with CgType::Int | `42_i64` |
| `LitNumber "3.14"` with CgType::Float | `3.f64` |
| `Binary "+" (Int, Int)` | `a + b` (direct i64 add) |
| `Lambda { param, body }` with Func(A, B) | `\|param: A\| -> B { body }` |
| `Record { x: 1, y: 2.0 }` with Record | struct init |
| `FieldAccess { base, "x" }` with Record | `base.x` (direct field) |
| `Match` with known ADT | `match scrut { Ctor1(a, b) => ..., Ctor2 => ... }` |
| `App(f, arg)` with Func(A, B) | `f(arg)` (direct call, no rt.apply) |

When a typed expression needs to cross into `Value` territory (e.g., passed to a builtin), we generate a **boxing** conversion: `Value::Int(x)`. When receiving from `Value` territory, we generate **unboxing**: `match v { Value::Int(x) => x, ... }`.

### Phase 4: Gradual adoption

The typed path is **opt-in per definition**. If a definition's type resolves to `CgType::Dynamic`, the existing `emit_expr` path is used unchanged. The two can coexist in the same generated file.

### Phase 5: Typed MIR pre-pass

Before emitting Rust source for closed scalar definitions, the backend now lowers eligible expressions into a compact typed MIR (basic blocks + explicit branch terminators). This creates a stable optimization boundary for future passes (constant folding, CSE, and branch simplification) while preserving the current fallback strategy.

Current MIR coverage is intentionally narrow and safe:
- Scalars (`Int`, `Float`, `Bool`) and their literals/locals/globals
- Scalar binary expressions
- Single-level `if` branching via CFG blocks
- Automatic fallback to direct typed emission when MIR lowering does not apply

### Boundary protocol

```
typed_world ──box──> Value_world
Value_world ──unbox──> typed_world
```

Boxing/unboxing functions for each CgType:
- `fn cg_box_int(v: i64) -> Value { Value::Int(v) }`
- `fn cg_unbox_int(v: Value) -> Result<i64, RuntimeError> { ... }`
- etc.

## Implementation Status

### Completed

1. **`CgType` enum** — `crates/aivi_core/src/cg_type.rs`
   - Variants: `Dynamic`, `Int`, `Float`, `Bool`, `Text`, `Unit`, `DateTime`, `Func`, `ListOf`, `Tuple`, `Record`, `Adt`
   - Methods: `is_closed()`, `rust_type()` (Rust type string), `emit_box()` (typed→Value), `emit_unbox()` (Value→typed)

2. **Type → CgType lowering** — `crates/aivi/src/typecheck/checker/cg_type_lowering.rs`
   - Converts the type checker's internal `Type` to `CgType` via substitution
   - Monomorphic defs get concrete CgTypes; polymorphic defs get `CgType::Dynamic`

3. **InferResult with CgTypes** — `crates/aivi/src/typecheck/infer.rs`
   - `infer_value_types_full()` returns `InferResult { diagnostics, type_strings, cg_types }`
   - CgType map: `HashMap<String, HashMap<String, CgType>>` (module → def → type)

4. **Typed expression emitter** — `crates/aivi/src/native_rust_backend/typed_expr.rs`
   - `emit_typed_expr()` produces unboxed Rust code for closed types
   - Supports: literals, variables, binary ops, if/else, lambda, app, tuple, record, field access, match, block, list
   - Falls back to `None` (Value path) for unsupported expressions

5. **Pipeline wiring** — `crates/aivi/src/rust_codegen.rs`
   - `compile_rust_native_typed()` injects CgType into RustIrDef before emission
   - `emit_module()` emits both Value-returning and `_typed` variants for closed-type defs

6. **Driver integration** — `crates/aivi_driver/src/lib.rs`
   - `desugar_target_with_cg_types()` returns `(HirProgram, CgType map)` for the build pipeline
   - CLI `compile` and `build` commands use the typed pipeline

7. **Tests** — `crates/aivi/tests/typed_codegen.rs`
    - CgType collection verification
    - `_typed` function emission for closed types
    - No `_typed` for polymorphic definitions
    - End-to-end compilation and execution
    - MIR pre-pass marker coverage for scalar closed definitions

### Not Yet Implemented

- **ADT typed emission** — ADT constructors in CgType exist but type-to-cg-type lowering returns empty constructor args
- **Optimization passes over MIR** — CSE/LICM/inlining are not implemented yet
- **Direct Cranelift/LLVM backend** — codegen still emits Rust source and relies on `rustc`

### Recently Implemented

- **Typed call chain emission** — multi-arg calls to known typed globals now chain: `fn_typed(rt)?(arg1)?(arg2)?`
- **Boxing/unboxing at boundaries** — typed subexpressions that can't be emitted in typed mode fall back to `emit_expr` + `emit_unbox()`, allowing typed code to cross into Value territory
- **Typed `main` rewrite** — `main` gets a `_typed` variant when its CgType is closed; the entry point calls the typed version and boxes the result

## Performance Impact

For a tight loop like `fib 40`:
- Current: ~40M `rt.apply()` calls, each matching `Value::Closure`, boxing/unboxing i64
- Typed: ~40M direct `fn(i64) -> i64` calls, zero heap allocation for ints

Expected speedup for numeric/algorithmic code: **5-20x**.
