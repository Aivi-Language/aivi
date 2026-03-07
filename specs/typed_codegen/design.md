# Compiler & Backend Design

This guide explains how AIVI turns typed source code into native machine code. It covers the compilation pipeline, runtime value layout, memory management, typed code generation, and the bridge between generated code and the runtime.

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
RustIR (monomorphised intermediate form)
  │  inline small functions
  │  lower
  ▼
Cranelift IR  →  native machine code
```

If you are new to compiler terminology, the stages mean roughly this:

- **Surface AST**: a structured form close to what the programmer wrote
- **HIR**: a cleaned-up representation used for name resolution and typechecking
- **Block-free HIR**: the same program after block sugar such as `do` and `resource` has been lowered away
- **RustIR**: a backend-oriented, monomorphised representation where generic code has been specialized
- **Cranelift IR**: low-level SSA-based code that can be turned into machine instructions

The CLI uses that same pipeline in two different output modes:

- **`aivi run`** uses Cranelift JIT compilation: code is compiled to native instructions in memory and executed immediately.
- **`aivi build`** uses Cranelift AOT compilation: code is compiled to an object file, wrapped in a thin Rust harness, and linked into a standalone executable.

## Incremental frontend checkpoints

The pipeline above shows the conceptual full flow. In practice, repeated checks and editor requests may reuse frontend artefacts at module, export-surface, definition-group, and schema-artefact granularity before later lowering continues.

The ownership, invalidation, and LSP snapshot rules for that reuse are specified in [Incremental Compilation & Workspace Checking](/tools/incremental_compilation).

## Runtime value representation

At runtime, AIVI uses a single tagged `Value` enum for dynamically represented values. Composite values are shared through `Arc`, which keeps immutable data cheap to clone.

| Category | Representation |
| --- | --- |
| Scalars | `Value::Int(i64)`, `Value::Float(f64)`, `Value::Bool(bool)` — stored inline |
| Text | `Value::Text(Arc<String>)` |
| Lists | `Value::List(Arc<Vec<Value>>)` |
| Records | `Value::Record(Arc<HashMap<InternedString, Value>>)` |
| Constructors | `Value::Constructor { name, args: Arc<Vec<Value>> }` |
| Closures | `Value::Closure { .. }`, `Value::Builtin(..)` |
| Effects | `Value::Effect(..)` — suspended computations |

### Boxed-pointer ABI

At the Cranelift boundary, values cross as `*mut Value` pointers. Runtime helpers such as `box_value()` and `unbox_value()` bridge between heap-allocated `Value` boxes and owned values.

When `CgType` information is known, scalar values can stay unboxed in registers:

- `Int` → `i64`
- `Float` → `f64`
- `Bool` → `i64` (`0` or `1`)

Boxing helpers such as `rt_box_int` and `rt_unbox_int` connect the scalar fast path to the generic boxed representation at boundaries where that is required.

### Runtime context

Every JIT or AOT function receives `*mut JitRuntimeCtx` as its first parameter. That context points at the full runtime and gives helper calls access to globals, builtins, and diagnostic state.

## Memory management

AIVI uses reference counting via Rust's `Arc`. There is no tracing garbage collector.

- **Immutable values** such as lists, records, text, and big integers are shared by incrementing reference counts instead of deep-copying.
- **Deterministic cleanup** means resources such as file handles and sockets can be released as soon as the last reference goes away, usually tied to `resource { ... }` scope boundaries.
- **No user-visible lifetimes** are exposed in the language surface.

### Perceus-style RC reuse analysis

The Cranelift backend performs a Perceus-inspired reuse analysis so it can recycle heap allocations instead of always freeing and allocating new boxes.
This is especially valuable for functional code that pattern matches on a value and then builds a closely related replacement.

The core observation is that every `Value` variant occupies the same boxed heap footprint, so a reusable `Box<Value>` allocation can be repurposed for any new `Value`. No constructor-specific shape matching is needed at the box level.

The optimization works in four pieces:

1. **Variable use analysis** (`use_analysis.rs`) walks monomorphised RustIR and marks references that are definitely the last use of a local variable. The analysis is conservative around branches.
2. **Reuse token extraction** happens when `lower_match` sees that the scrutinee is a last-use local. It calls `rt_try_reuse(scrutinee)`, which drops inner data but keeps the outer `Box<Value>` allocation alive as a reuse token.
3. **Reuse-aware allocation** lets the first allocation in a match arm consume that token through helpers such as `rt_reuse_constructor`, `rt_reuse_list`, `rt_reuse_tuple`, or `rt_reuse_record`.
4. **In-place record patching** uses `rt_patch_record_inplace` when record update targets a last-use local and the underlying `Arc<HashMap<...>>` is uniquely owned.

### Layout optimizations

Several layout-level choices help generated code stay efficient:

- **Record shapes** use interned layouts so repeated field lookups can resolve to stable offsets after one shape lookup.
- **Closed record types** give the backend enough information to lower known layouts more directly.
- **Tagged scalar encoding** reduces transient allocation pressure for scalar runtime values.

### Cycle handling

Strict immutability prevents ordinary data-structure cycles. The main remaining cycle source is recursive closures, and the runtime handles those using weak back-references or scope-based cycle breaking.

## `CgType`: typed code generation

`CgType` is the backend's compact notion of runtime layout. It tells code generation when a value can stay in a specialized representation and when it must fall back to generic boxed `Value` handling.

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

The typechecker lowers its richer internal `Type` representation into `CgType` after substitution. Monomorphic definitions can often get concrete `CgType` values; polymorphic definitions fall back to `CgType::Dynamic`.

### How `CgType` improves code generation

1. **Parameter binding**: function types are decomposed into per-parameter `CgType` information so scalar arguments can be unboxed on entry.
2. **Call dispatch**: direct JIT-to-JIT calls can pass unboxed scalars natively, while runtime helper calls keep using boxed pointers where necessary.
3. **Return values**: known scalar results can stay unboxed until they cross a boundary that requires boxing.
4. **Fallback**: unknown or open-ended layouts still work through generic boxed `Value` handling.

## JIT compilation (`aivi run`)

The JIT path proceeds like this:

1. lower Surface → HIR → block-free HIR → RustIR, including monomorphization
2. attach `CgType` information from type inference
3. inline eligible small functions
4. declare all Cranelift function signatures so bodies can emit direct calls
5. lower each `RustIrExpr` to Cranelift IR using `TypedValue` to track both SSA value and `CgType`
6. finalize native function pointers, wrap them as runtime callables, and register them in globals
7. look up `main` and run its effect through the runtime

Multi-clause domain operators are merged into `Value::MultiClause` so clause ordering is preserved.

## AOT compilation (`aivi build`)

The AOT path shares the same frontend and lowering work, then adds packaging steps:

1. emit a relocatable object file through `cranelift-object`
2. generate `__aivi_main()` to register compiled functions and captured-variable metadata
3. embed string constants as relocatable data sections
4. compile and link a thin Rust harness via `cargo build`

The final executable still enters through the runtime so effects, globals, and helper bridges behave the same way as they do in JIT mode.

## Runtime helper bridge

Generated Cranelift code calls into the runtime through a large `extern "C"` helper surface.

| Category | Examples |
| --- | --- |
| Boxing / unboxing | `rt_box_int`, `rt_unbox_float`, `rt_clone_value`, `rt_drop_value` |
| Allocation | `rt_alloc_unit`, `rt_alloc_string`, `rt_alloc_list`, `rt_alloc_record`, `rt_alloc_constructor` |
| Reuse | `rt_try_reuse`, `rt_reuse_constructor`, `rt_reuse_record`, `rt_reuse_list`, `rt_reuse_tuple`, `rt_patch_record_inplace` |
| Access | `rt_record_field`, `rt_list_index`, `rt_tuple_item`, `rt_constructor_arg` |
| Pattern matching | `rt_constructor_name_eq`, `rt_value_equals`, `rt_list_tail`, `rt_list_len` |
| Control | `rt_apply`, `rt_force_thunk`, `rt_run_effect`, `rt_bind_effect`, `rt_check_call_depth` |
| Generators | `rt_gen_vec_new`, `rt_gen_vec_push`, `rt_gen_vec_into_generator` |
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

- **Monomorphization** happens first, so inlined bodies are already ground-typed.
- **Perceus-style reuse analysis** runs after inlining, which means larger visible bodies can expose more reuse opportunities.
- **`CgType` propagation** also benefits, because an unboxed scalar can sometimes stay unboxed all the way through the inlined body.
