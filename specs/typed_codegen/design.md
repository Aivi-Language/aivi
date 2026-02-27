# Compiler & Backend Design

This document describes AIVI's compilation pipeline, value representation, memory
management, and the Cranelift-based code generation backend.

## Compilation Pipeline

```text
Source (.aivi)
  │  parse
  ▼
Surface AST
  │  desugar
  ▼
Kernel IR (HIR)
  │  type-check  →  CgType map
  ▼
RustIR (monomorphised intermediate form)
  │  inline small functions
  │  lower
  ▼
Cranelift IR  →  native machine code
```

- **`aivi run`** — Cranelift **JIT**: compiles to native code in-memory and
  executes immediately.
- **`aivi build`** — Cranelift **AOT**: compiles to a relocatable object file
  (`.o`), generates a thin Rust harness, and links via `cargo build` to produce
  a standalone executable.

## Value Representation

All runtime values share a single tagged enum (`Value`). Composite values are
reference-counted via `Arc`:

| Category | Representation |
| --- | --- |
| Scalars | `Value::Int(i64)`, `Value::Float(f64)`, `Value::Bool(bool)` — inlined, no heap |
| Text | `Value::Text(Arc<String>)` |
| Lists | `Value::List(Arc<Vec<Value>>)` |
| Records | `Value::Record(Arc<HashMap<InternedString, Value>>)` |
| Constructors | `Value::Constructor { name, args: Arc<Vec<Value>> }` |
| Closures | `Value::Closure { .. }`, `Value::Builtin(..)` |
| Effects | `Value::Effect(..)` — suspended computations |

### Boxed-pointer ABI

At the Cranelift boundary every value is passed as a `*mut Value` (a boxed
pointer). The runtime helpers `box_value()` and `unbox_value()` convert between
heap-allocated pointers and owned `Value` instances.

When `CgType` information is available, **scalar values stay in CPU registers**
without heap allocation:

- `Int` → `i64`
- `Float` → `f64`
- `Bool` → `i64` (0 / 1)

Boxing/unboxing helpers (`rt_box_int`, `rt_unbox_int`, etc.) bridge the two
representations at function boundaries.

### Runtime context

Every JIT/AOT function receives `*mut JitRuntimeCtx` as its first parameter.
This context holds a pointer to the full `Runtime` and is threaded through all
calls so that runtime helpers can access globals, builtins, and diagnostics.

## Memory Management

AIVI uses **reference counting** (via Rust's `Arc`) as its sole memory
management strategy. There is no tracing garbage collector.

- **Immutable values**: `List`, `Record`, `Text`, `BigInt`, etc. are immutable
  and shared via `Arc`. Cloning a value increments the reference count — no deep
  copy.
- **Deterministic cleanup**: resources (file handles, sockets) are freed as soon
  as the last reference is dropped, tied to the `resource { .. }` scope.
- **No user-visible lifetimes**: AIVI does not expose Rust-like lifetime
  annotations in source code.

### Perceus-style RC reuse analysis

The Cranelift backend implements a **Perceus-inspired reuse analysis** that
recycles heap allocations instead of freeing and re-allocating them. This is
especially effective for functional match-and-reconstruct patterns that dominate
AIVI code.

**Key insight.** All `Value` enum variants occupy the same heap size (Rust enum =
max variant size). Any reuse token from any dropped `Value` can be recycled for
any new `Value` — no shape matching is needed at the box level. This is simpler
than Koka's Perceus (which requires per-constructor sizing).

The optimisation has four components:

1. **Variable use analysis** (`use_analysis.rs`). A pre-lowering pass walks the
   monomorphised RustIR body and computes, for each local variable reference,
   whether it is the *last* use of that variable. The analysis is conservative
   for branches: a variable used in multiple match arms or both if-branches is
   not marked as last-use inside those branches.

2. **Reuse token extraction.** When `lower_match` detects that the scrutinee is
   a last-use local, it calls `rt_try_reuse(scrutinee)` after pattern
   destructuring. This drops the inner `Value` data (decrements `Arc`s, frees
   strings) but preserves the `Box<Value>` allocation, returning it as a
   *reuse token*.

3. **Reuse-aware allocation.** The first allocation in the match arm body
   (constructor, list, tuple, or record) consumes the reuse token via
   `rt_reuse_*` helpers. If a token is available, the new `Value` is written
   directly into the existing allocation via `ptr::write`; otherwise a fresh
   `Box::new` is used. Tokens are block-local and cleared after each arm to
   prevent cross-branch leaks.

4. **In-place record patching.** When `{ rec | field = newVal }` targets a
   last-use local, the lowering calls `rt_patch_record_inplace` instead of the
   regular `rt_patch_record`. If the inner `Arc<HashMap>` has a strong count of
   1 the HashMap is mutated in place and the box is reused; otherwise it falls
   back to clone-and-patch.

### Layout optimisations

- **Record shapes**: record values use interned field layouts so that repeated
  field lookups resolve to stable offsets after one shape resolution.
- **Closed record types**: type-level records are closed by default, enabling
  backends to lower known shapes to fixed-layout representations.
- **Tagged scalar encoding**: scalar runtime values expose a compact tagged
  representation to reduce transient allocation pressure.

### Cycle handling

Strict immutability prevents data-structure cycles. The only source of cycles is
**recursive closures** (a function referring to itself). The runtime breaks these
via weak back-references or scope-based cycle breaking — transparent to the
programmer.

## CgType — Typed Code Generation

`CgType` is a compile-time type annotation that tells the backend whether a
definition's runtime layout is fully known ("closed") or requires the generic
`Value` fallback ("open / dynamic").

```rust
pub enum CgType {
    Dynamic,              // needs Value boxing
    Int, Float, Bool,     // scalar — stays in registers
    Text, Unit, DateTime,
    Func(Box<CgType>, Box<CgType>),
    ListOf(Box<CgType>),
    Tuple(Vec<CgType>),
    Record(Vec<(String, CgType)>),
    Adt { name: String, constructors: Vec<(String, Vec<CgType>)> },
    Value,                // explicitly boxed for interop
}
```

The type checker's internal `Type` is lowered to `CgType` via substitution.
Monomorphic definitions get concrete `CgType`s; polymorphic definitions get
`CgType::Dynamic`.

### How CgType drives optimisation

1. **Parameter binding**: `decompose_func_type()` breaks a nested
   `Func(A, Func(B, ...))` into per-parameter `Option<CgType>`. Parameters with
   a known scalar type are **unboxed on entry** — no heap allocation.
2. **Call dispatch**: direct JIT-to-JIT calls pass unboxed scalars natively;
   runtime helpers receive boxed pointers.
3. **Return path**: if the return type is a known scalar, the value stays
   unboxed in Cranelift IR until a boundary where it must be boxed.
4. **Fallback**: unknown types or compound values default to boxed `*mut Value`.

## JIT Compilation (`aivi run`)

1. Lower HIR → Kernel → RustIR with monomorphisation.
2. Annotate each definition with its `CgType` from type inference.
3. **Inline**: rewrite call sites of small functions
   with the callee's body (see [Function Inlining](#function-inlining) below).
4. **Pass 1 — declare**: register all function signatures in Cranelift so that
   bodies can emit direct `call` instructions to other JIT functions.
5. **Pass 2 — compile**: lower each `RustIrExpr` to Cranelift IR using the
   `TypedValue` wrapper (tracks both the SSA value and its `CgType`).
6. **Finalise**: extract native function pointers, wrap as `Value::Builtin`,
   register into `runtime.ctx.globals`.
7. **Execute**: look up `"main"`, run its effect via the runtime.

Multi-clause domain operators are merged into `Value::MultiClause` for ordered
clause traversal.

## AOT Compilation (`aivi build`)

1. Same front-end pipeline as JIT (steps 1–5).
2. Emit a relocatable object file via `cranelift-object`.
3. Generate `__aivi_main()` which:
   - registers all compiled function pointers (short + qualified names) via
     `rt_register_jit_fn`
   - registers captured-variable info for inner lambdas
   - looks up `"main"` and runs it as an effect
4. String constants are embedded as relocatable data sections (unlike JIT which
   uses process-memory addresses).
5. A thin Rust harness calls `__aivi_main()` and is linked via `cargo build`.

## Runtime Helper Bridge

50+ `extern "C"` functions are registered as JIT symbols so that Cranelift code
can call into the runtime:

| Category | Examples |
| --- | --- |
| Boxing / unboxing | `rt_box_int`, `rt_unbox_float`, `rt_clone_value`, `rt_drop_value` |
| Allocation | `rt_alloc_unit`, `rt_alloc_string`, `rt_alloc_list`, `rt_alloc_record`, `rt_alloc_constructor` |
| Reuse (Perceus) | `rt_try_reuse`, `rt_reuse_constructor`, `rt_reuse_record`, `rt_reuse_list`, `rt_reuse_tuple`, `rt_patch_record_inplace` |
| Access | `rt_record_field`, `rt_list_index`, `rt_tuple_item`, `rt_constructor_arg` |
| Pattern matching | `rt_constructor_name_eq`, `rt_value_equals`, `rt_list_tail`, `rt_list_len` |
| Control | `rt_apply`, `rt_force_thunk`, `rt_run_effect`, `rt_bind_effect`, `rt_check_call_depth` |
| Generators | `rt_gen_vec_new`, `rt_gen_vec_push`, `rt_gen_vec_into_generator` |
| Sigils | `rt_eval_sigil` |
| AOT | `rt_register_jit_fn` |

All helpers receive `JitRuntimeCtx*` as first parameter. Errors are stored in
`runtime.jit_pending_error` (first-error-wins semantics to preserve root cause).

## Function Inlining

A RustIR → RustIR rewrite pass (`inline.rs`) runs after monomorphization and
before Cranelift lowering. It replaces call sites of small, non-recursive
functions with the callee's body, eliminating call overhead and exposing more
code to Cranelift's register allocator and peephole optimisations.

### Eligibility

A function is an inline candidate if its body (after peeling lambda wrappers)
has an AST-node cost ≤ 12 **and** it is not self-recursive (no `Global`
reference to its own name in the body).

### Mechanics

1. **Candidate collection.** Walk all `RustIrDef`s and index eligible functions
   by both qualified (`Module.name`) and short name.
2. **Bottom-up rewriting.** For each function body, recursively process children
   first, then check if the current node is an `App(Global(name), arg)`,
   `Call(Global(name), args)`, or `Pipe(Global(name), arg)` targeting a
   candidate. If so, clone the candidate body, substitute parameters with
   arguments, freshen all expression ids, and replace the call node.
3. **Depth limiting.** Inlining is capped at depth 4 to prevent runaway
   expansion from chains or mutual recursion.
4. **Shadowing.** The substitution pass correctly handles variable shadowing in
   lambda parameters, match arm bindings, and block `let` bindings.
5. **Id freshening.** Every inlined sub-tree gets new unique expression ids so
   that the downstream Perceus use-analysis (which keys on `(expr_id, var_name)`
   tuples) produces correct last-use information.

### Interaction with other passes

- **Monomorphization** runs first, so inlined bodies are already ground-typed.
- **Perceus reuse analysis** runs after inlining, on the expanded bodies. More
  code is visible per function → more reuse opportunities.
- **CgType propagation** benefits from inlining: if a caller already has an
  unboxed scalar and the inlined body expects it, the box/unbox round-trip is
  eliminated entirely.

## Maybe later

- Stable binary ABI across compiler versions
- Zero-copy projections for all aggregate types
- Advanced optimisation passes (LICM, loop transforms)
