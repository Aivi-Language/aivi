# Decorators

Decorators provide **compile-time metadata** attached to definitions.

## Policy (Constraints)

Decorators are intentionally narrow:

- Decorators MUST NOT be used to model domain semantics (e.g. database schemas/ORM, SQL, HTTP, validation rules).
- Integration behavior belongs in **typed values** (e.g. `Source` configurations) and **types** (decoders), not hidden in decorators.
- Unknown decorators are a compile error.
- `@native` is restricted to top-level definitions and requires an explicit type signature for type-safe bindings.

## 14.1 Syntax

<<< ../snippets/from_md/syntax/decorators/syntax.aivi{aivi}

Decorators appear before the binding they annotate.

## 14.2 Standard Decorators

### Compile-Time

| Decorator     | Usage                                            | Meaning                                                               |
|:------------- |:------------------------------------------------ |:--------------------------------------------------------------------- |
| `@static`     | `@static x = file.read "..."`                    | Embed at compile time                                                 |
| `@native`     | `@native "gtk4.appRun"`                          | Bind definition to a runtime/native function path                     |
| `@deprecated` | `@deprecated msg`                                | Emit warning on use                                                   |
| `@debug`      | `@debug()` / `@debug(pipes, args, return, time)` | Emit structured debug trace events when compiled with `--debug-trace` |

### Tooling (MCP)

MCP decorators are not supported in v0.1.

### Testing

| Decorator | Usage                                               | Meaning                                                     |
|:--------- |:--------------------------------------------------- |:----------------------------------------------------------- |
| `@test`   | `@test "adds two numbers" add_is_commutative = ...` | Mark a definition as a test case (description is mandatory) |

### Pragmas (Module-level)

| Decorator     | Usage                  | Meaning                                                    |
|:------------- |:---------------------- |:---------------------------------------------------------- |
| `@no_prelude` | `@no_prelude module M` | Skip implicit prelude import                               |
| `@test`       | `@test module M`       | Mark module as test-only (excluded from production builds) |

## 14.3 Decorator Desugaring

Decorators desugar to compile-time metadata:

| Surface                        | Desugared                                                   |
|:------------------------------ |:----------------------------------------------------------- |
| `@static x = file.read ...`    | Compile-time evaluation                                     |
| `@native "mod.fn" f x y = ...` | Rewritten to `f x y = mod.fn x y` (type signature required) |
| (none)                         | (none)                                                      |

## 14.4 Usage Examples

### Compile-Time Embedding (`@static`)

<<< ../snippets/from_md/syntax/decorators/compile_time_embedding.aivi{aivi}

The compiler evaluates the right-hand side at compile time and embeds the result as a constant.

### Native Bindings (`@native`)

`@native` binds an AIVI definition to a host-runtime function identified by a dotted path. The body you write is a dummy — the compiler replaces it with a call forwarding all parameters to the native target.

<<< ../snippets/from_md/syntax/decorators/native_basic.aivi{aivi}

**Rules:**

1. Must be **top-level** (not inside `domain` or other blocks).
2. Must have an **explicit type signature**.
3. The target path must be a **valid dotted identifier** (e.g. `"gtk4.windowPresent"`, `"system.io.file.readAll"`).
4. Parameter names in the dummy body must be **simple identifiers** (no destructuring).

#### Path anatomy

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

#### How `@native` bridges to Rust Cargo packages — step by step

Suppose you want to expose the [uuid](https://crates.io/crates/uuid) crate to AIVI programs so users can generate UUIDs.

<<< ../snippets/from_md/syntax/decorators/native_walkthrough.aivi{aivi}

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

### Deprecation Warnings (`@deprecated`)

<<< ../snippets/from_md/syntax/decorators/deprecated_example.aivi{aivi}

The compiler emits a warning at every call site. Use a human-readable migration hint as the argument.

### Debug Tracing (`@debug`)

`@debug` is a tooling pragma for compiler-emitted trace logs. It has no semantic effect unless you compile with `--debug-trace`.

- `@debug()` (or `@debug`) defaults to function-level timing only.
- Parameters are order-insensitive; duplicates are ignored.
- Allowed parameters: `pipes`, `args`, `return`, `time`.

<<< ../snippets/from_md/syntax/decorators/debug_example.aivi{aivi}

When enabled, the compiler emits JSONL-friendly structured events:

- `fn.enter` / `fn.exit` per function call
- `pipe.in` / `pipe.out` per `|>` step (when `pipes` is enabled)

For multiple pipelines in a function body, step numbering restarts per pipeline chain and events include an additional `pipeId` field for disambiguation.

### Test Declarations (`@test`)

<<< ../snippets/from_md/syntax/decorators/test_example.aivi{aivi}

A description string is **mandatory**. Tests are collected by `aivi test` and excluded from production builds. When applied to a module (`@test module M`), the entire module is test-only.

### No-Prelude Pragma (`@no_prelude`)

<<< ../snippets/from_md/syntax/decorators/no_prelude_example.aivi{aivi}

Useful when writing low-level modules that intentionally avoid or redefine prelude symbols.
