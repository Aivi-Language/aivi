# `@native` — Runtime Function Bindings

<!-- quick-info: {"kind":"decorator","name":"@native"} -->
`@native` binds an AIVI definition to a host-runtime function identified by a dotted path. The body you write is a dummy — the compiler replaces it with a call forwarding all parameters to the native target.
<!-- /quick-info -->

## Syntax

```aivi
@native "module.functionName"
binding : TypeSig
binding = param1 param2 => dummyBody
```

## Example

<<< ../../snippets/from_md/syntax/decorators/native_basic.aivi{aivi}

## Rules

1. Must be **top-level** (not inside `domain` or other blocks).
2. Must have an **explicit type signature**.
3. The target path must be a **valid dotted identifier** (e.g. `"gtk4.windowPresent"`, `"system.io.file.readAll"`).
4. Parameter names in the dummy body must be **simple identifiers** (no destructuring).

## Path Anatomy

The `@native` string is a **runtime record path**, not a Cargo crate name.

```
@native "gtk4.windowPresent"
         ───┬ ──────┬──────
            │       └─ field name inside that record
            └── runtime module name (registered in the global environment)
```

At startup the AIVI runtime registers a set of **module records** into the global environment (see `register_builtins` in `core.rs`). Each module is a `Value::Record` — a `HashMap<String, Value>` whose keys are function names. The first segment of the `@native` path selects the module record; subsequent segments are field accesses into it.

For example, `"gtk4.windowPresent"`:

1. **`gtk4`** — resolves to the record built by `build_gtk4_record()`, which is backed by the Rust `gtk4` / `libadwaita` crates.
2. **`windowPresent`** — looks up the `"windowPresent"` key in that record, which points to a `Value::Builtin` closure wrapping the real C call `gtk_window_present()`.

The module name is chosen by the AIVI runtime, **not** by the Cargo crate. A single runtime module may wrap multiple Cargo crates (e.g. `crypto` wraps parts of `ring`, `sha2`, and `hmac`), and the module name may differ from the crate name entirely (e.g. the Cargo crate `rusqlite` is exposed as the module `database`).

## Walkthrough: Bridging a Rust Crate

Suppose you want to expose the [uuid](https://crates.io/crates/uuid) crate to AIVI programs so users can generate UUIDs.

<<< ../../snippets/from_md/syntax/decorators/native_walkthrough.aivi{aivi}

Under the hood the compiler carries out these steps:

| Step | What happens | Where |
|:---- |:------------ |:----- |
| **1. Parse** | `@native "uuid.v4"` rewrites the body to `v4 = uuid.v4` (a `FieldAccess` AST node). For `v5` with two params it becomes `v5 = namespace name => uuid.v5 namespace name`. | `crates/aivi/src/surface/parser/` |
| **2. Type-check** | The explicit signature `Effect Text Text` is verified against the native target's expected type. | `crates/aivi/src/ty/` |
| **3. Rust builtin module** | You create `crates/aivi/src/runtime/builtins/uuid.rs` containing a `build_uuid_record()` function. It returns a `Value::Record` with fields `"v4"` and `"v5"` — each a `Value::Builtin` closure that calls the real `uuid` crate. | `crates/aivi/src/runtime/builtins/` |
| **4. Cargo dependency** | Add `uuid = { version = "1", features = ["v4", "v5"] }` to `Cargo.toml`. The builtin closures call `Uuid::new_v4().to_string()` etc., then wrap the result in `Value::Text`. | `Cargo.toml` |
| **5. Register** | In `register_builtins(env)`, add `env.set("uuid", build_uuid_record())` so the name `uuid` resolves globally. | `crates/aivi/src/runtime/builtins/core.rs` |
| **6. JIT / AOT** | At runtime, Cranelift emits a call to `rt_record_field(uuid_record, "v4")` which does a `HashMap` lookup and invokes the Rust closure. | `crates/aivi/src/cranelift_backend/lower.rs` |

The AIVI user never sees the Rust layer — they call `uuid.v4` like any other function.
