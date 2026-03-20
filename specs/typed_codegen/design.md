# Compiler & Backend Design

This guide explains AIVI's current compiler and backend architecture for turning typed source code into native machine code. It covers the compilation pipeline, runtime value layout, memory management, typed code generation, and the bridge between generated code and the runtime.

It is mainly contributor-facing, but the opening sections aim to give a readable high-level picture before the page gets into backend-specific detail. Where the implementation is intentionally conservative today, this page calls that out instead of describing a more advanced future design as if it already existed.

## Compilation pipeline at a glance

```text
Source (.aivi)
  │  parse
  ▼
Surface AST
  │  desugar
  ▼
HIR (High-level IR)
  │  desugar blocks
  ▼
HIR (Block-free)
  │  type-check  →  CgType map
  ▼
RustIR (backend-oriented intermediate form)
  │  inject source schemas
  │  monomorphise
  │  inline small functions
  │  lower
  ▼
Cranelift IR  →  native machine code
```

If you are new to compiler terminology, the stages mean roughly this:

- **Surface AST** (**abstract syntax tree**): a structured form close to what the programmer wrote
- **HIR** (**high-level intermediate representation**): a cleaned-up form used for name resolution and typechecking
- **Block-free HIR**: the same program after block sugar such as `do` and `resource` has been lowered away
- **RustIR**: a backend-oriented representation that is later specialized (monomorphised) to concrete types
- **Cranelift IR**: a low-level **static single assignment (SSA)** representation that can be turned into machine instructions

The CLI uses that same pipeline in two different output modes:

- **`aivi run`** uses Cranelift **JIT** (**just-in-time**) compilation: code is compiled to native instructions in memory and executed immediately.
- **`aivi build`** uses Cranelift **AOT** (**ahead-of-time**) compilation: code is compiled to an object file, wrapped in a thin Rust harness, and linked into a standalone executable.

## Incremental frontend checkpoints

The pipeline above shows the conceptual full flow. In practice, repeated checks and editor requests may reuse frontend artefacts at module, export-surface, definition-group, and schema-artefact granularity before later lowering continues.

The ownership, invalidation, and LSP snapshot rules for that reuse are specified in [Incremental Compilation & Workspace Checking](../tools/incremental_compilation.md).

## Runtime value representation

At runtime, AIVI centers on a tagged `Value` enum. The real enum includes many host-integration cases; the table below groups the cases most relevant to code generation and runtime layout. Shared immutable aggregates commonly use `Arc` (Rust's atomically reference-counted shared pointer), which keeps cloning cheap without exposing mutation in the language.

| Category | Representation |
| --- | --- |
| Scalars | `Value::Unit`, `Value::Int(i64)`, `Value::Float(f64)`, `Value::Bool(bool)` — stored inline in the enum |
| Text / time | `Value::Text(String)`, `Value::DateTime(String)` |
| Collections | `Value::List(Arc<Vec<Value>>)`, `Value::Tuple(Vec<Value>)`, `Value::Record(Arc<HashMap<String, Value>>)` |
| Algebraic data | `Value::Constructor { name, args: Vec<Value> }`, `Value::MultiClause(Vec<Value>)` |
| Callables / laziness | `Value::Builtin(..)` for callable values and partial applications, `Value::Thunk(..)` for lazy values |
| Effects / resources / hosts | `Value::Effect(..)`, `Value::Resource(..)`, plus host-facing variants for sources, channels, files, sockets, servers, and other runtime objects |

### Boxed-pointer calling convention (ABI)

At the current Cranelift function ABI, compiled functions still receive `*mut JitRuntimeCtx` as their first parameter and use boxed `*mut Value` pointers for ordinary arguments and return values. Runtime helpers such as `box_value()` and `unbox_value()` bridge between heap-allocated `Value` boxes and owned Rust values.

Inside a compiled function body, `CgType` information lets the lowering pass keep some scalar values unboxed in SSA registers:

- `Int` → `i64`
- `Float` → `f64`
- `Bool` → `i64` (`0` or `1`)

Boxing helpers such as `rt_box_int` and `rt_unbox_int` connect that scalar fast path to the generic boxed representation at helper boundaries and function entry / exit points. In other words, the current implementation gets most of its performance benefit from type-directed optimizations *within* lowered function bodies rather than through a fully specialized public ABI between all compiled functions.

### Runtime context

Every JIT or AOT function receives `*mut JitRuntimeCtx` as its first parameter. That context points at the full runtime and gives helper calls access to globals, builtins, and diagnostic state.

## Memory management

AIVI uses reference counting via Rust's `Arc`. There is no tracing garbage collector.

- **Immutable values** such as lists, records, large numeric values, and many host-facing runtime handles are shared by incrementing reference counts instead of deep-copying.
- **Resource cleanup** is coordinated by flow scopes, `@cleanup` registrations, and the runtime's cleanup stack; dropping the last reference is only part of that story.
- **No user-visible lifetimes** are exposed in the language surface.

### Perceus-style RC reuse analysis

The Cranelift backend performs a Perceus-inspired reuse analysis so it can recycle heap allocations instead of always freeing and allocating new boxes. Perceus is a reference-counting optimization approach from functional-language implementation research; the important practical point here is that the compiler tries to reuse boxes when it can prove an old value is at its last use.
This is especially valuable for functional code that pattern matches on a value and then builds a closely related replacement.

The core observation is that every `Value` variant occupies the same boxed heap footprint, so a reusable `Box<Value>` allocation can be repurposed for any new `Value`. No constructor-specific shape matching is needed at the box level.

The optimization works in four pieces:

1. **Variable use analysis** (`use_analysis.rs`) walks monomorphised RustIR and marks references that are definitely the last use of a local variable. The analysis is conservative around branches.
2. **Reuse token extraction** happens when `lower_match` sees that the scrutinee is a last-use local. It calls `rt_try_reuse(scrutinee)`, which drops inner data but keeps the outer `Box<Value>` allocation alive as a reuse token.
3. **Reuse-aware allocation** lets the first allocation in a match arm consume that token through helpers such as `rt_reuse_constructor`, `rt_reuse_list`, `rt_reuse_tuple`, or `rt_reuse_record`.
4. **In-place record patching** uses `rt_patch_record_inplace` when record update targets a last-use local and the underlying `Arc<HashMap<...>>` is uniquely owned.

### Current layout-oriented optimizations

Several current implementation choices help generated code stay efficient:

- **Closed `CgType` information** lets lowering decide when `Int`, `Float`, and `Bool` can remain as unboxed SSA values instead of boxed `Value`s.
- **Record patch fast path** uses `rt_patch_record_inplace` when both the outer `Box<Value>` and the inner `Arc<HashMap<...>>` are uniquely owned.
- **Tagged builtin arguments** use a compact `TaggedValue` cache for some scalar builtin-call paths without changing the main `Value` representation.

### Cycle handling

Most values produced by ordinary AIVI code are immutable and acyclic. Because the runtime relies on reference counting rather than a tracing collector, any intentionally cyclic host structure must be managed explicitly by runtime code rather than assumed to be collected automatically.

## `CgType`: typed code generation

`CgType` is the backend's compact notion of runtime layout. In plain language, it tells code generation what is known about a value's shape at compile time, including whether it can stay in a specialized form such as an unboxed integer in a CPU register.

In the current implementation, only `Int`, `Float`, and `Bool` are fully unboxed during expression lowering. Other `CgType` variants still matter—they describe closed shapes, guide helper selection, and support source-schema injection—but they usually continue to travel through the boxed `Value` ABI today.

In Rust-ish notation, the current enum is:

```rust
pub enum CgType {
    Dynamic,
    Int, Float, Bool,
    Text, Unit, DateTime,
    Func(Box<CgType>, Box<CgType>),
    ListOf(Box<CgType>),
    Tuple(Vec<CgType>),
    Record(BTreeMap<String, CgType>),
    Adt { name: String, constructors: Vec<(String, Vec<CgType>)> },
}
```

The `BTreeMap` here just means record fields are stored in a deterministic name-sorted map. The typechecker lowers its richer internal `Type` representation into `CgType` after substitution. Monomorphic definitions can often get concrete `CgType` values; polymorphic definitions fall back to `CgType::Dynamic`.

### How `CgType` improves code generation

1. **Parameter binding**: function types are decomposed into per-parameter `CgType` information so scalar arguments can be unboxed immediately after entry.
2. **Expression lowering**: `TypedValue` tracks both the SSA value and its `CgType`, allowing arithmetic and comparisons on known scalars to stay unboxed through larger parts of a function body.
3. **Boundary control**: known scalar results can stay unboxed until they cross the current boxed ABI or a helper boundary that requires boxing.
4. **Fallback**: unknown or open-ended layouts still work through generic boxed `Value` handling, so typed lowering is an optimization rather than a separate execution model.

## JIT compilation (`aivi run`)

The JIT path proceeds like this:

1. lower Surface → HIR → block-free HIR → RustIR
2. attach `CgType` information from type inference
3. inject source-boundary schemas where the expected type is statically known
4. monomorphize generic RustIR definitions into concrete specializations
5. inline eligible small functions
6. declare all Cranelift function signatures so bodies can emit direct calls
7. lower each `RustIrExpr` to Cranelift IR using `TypedValue` to track both SSA value and `CgType`
8. finalize native function pointers, wrap them as runtime callables, and register them in globals
9. look up `main` and run its effect through the runtime

Multi-clause domain operators are merged into `Value::MultiClause` so clause ordering is preserved.

## AOT compilation (`aivi build`)

The AOT path shares the same frontend and lowering work, then adds packaging steps:

1. emit a relocatable object file through `cranelift-object`
2. generate `__aivi_main()` to register compiled top-level functions and compiled lambdas
3. embed string constants as relocatable data sections
4. compile and link a thin Rust harness via `cargo build`

The final executable still enters through the runtime so effects, globals, and helper bridges behave the same way as they do in JIT mode.

## Runtime helper bridge

Generated Cranelift code calls into the runtime through a large `extern "C"` helper surface. Representative helpers include:

| Category | Examples |
| --- | --- |
| Boxing / unboxing | `rt_box_int`, `rt_unbox_float`, `rt_clone_value`, `rt_drop_value` |
| Allocation | `rt_alloc_unit`, `rt_alloc_string`, `rt_alloc_list`, `rt_alloc_record`, `rt_alloc_constructor` |
| Reuse | `rt_try_reuse`, `rt_reuse_constructor`, `rt_reuse_record`, `rt_reuse_list`, `rt_reuse_tuple`, `rt_patch_record_inplace` |
| Access | `rt_record_field`, `rt_list_index`, `rt_tuple_item`, `rt_constructor_arg` |
| Pattern matching | `rt_constructor_name_eq`, `rt_value_equals`, `rt_list_tail`, `rt_list_len` |
| Control | `rt_apply`, `rt_force_thunk`, `rt_run_effect`, `rt_bind_effect`, `rt_wrap_effect`, `rt_check_call_depth` |
| Generators | `rt_gen_vec_new`, `rt_gen_vec_push`, `rt_gen_vec_extend_generator`, `rt_gen_vec_into_generator` |
| Sigils | `rt_eval_sigil` |
| AOT registration | `rt_register_jit_fn` |

All helpers receive `JitRuntimeCtx*` as their first parameter. Errors are recorded in `runtime.jit_pending_error` with first-error-wins semantics so the root cause is preserved.

## Function inlining

A RustIR-to-RustIR inlining pass runs after monomorphization and before Cranelift lowering. Its job is to remove the overhead of small calls and expose more code to downstream optimizations.

### Eligibility

A function is an inline candidate when:

- its body has an AST-node cost of at most `12` after peeling lambda wrappers, and
- it is not self-recursive

### Mechanics

1. collect candidate functions by both qualified and short name
2. rewrite call sites bottom-up so child expressions are processed first
3. substitute parameters with arguments in cloned function bodies
4. respect variable shadowing in lambdas, match arms, and block bindings
5. freshen all expression ids so later passes such as reuse analysis stay correct

### Interaction with later passes

- **Monomorphization** happens first, so inlined bodies are already specialized to concrete types.
- **Perceus-style reuse analysis** runs after inlining, which means larger visible bodies can expose more reuse opportunities.
- **`CgType` propagation** also benefits, because an unboxed scalar can sometimes stay unboxed all the way through the inlined body.
