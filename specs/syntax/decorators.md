# Decorators

Decorators provide **compile-time metadata** attached to definitions.

## Policy

Decorators are intentionally narrow:

- Decorators MUST NOT be used to model domain semantics (e.g. database schemas/ORM, SQL, HTTP, validation rules).
- Integration behavior belongs in **typed values** (e.g. `Source` configurations) and **types** (decoders), not hidden in decorators.
- Unknown decorators are a compile error.
- `@native` is restricted to top-level definitions and requires an explicit type signature for type-safe bindings.

## Syntax

<<< ../snippets/from_md/syntax/decorators/syntax.aivi{aivi}

Decorators appear before the binding they annotate.

## Available Decorators

| Decorator     | Purpose                                              |
|:------------- |:---------------------------------------------------- |
| `@static`     | Compile-time evaluation and embedding                |
| `@native`     | Bind to runtime/native functions                     |
| `@deprecated` | Emit usage warnings with migration hints             |
| `@debug`      | Structured debug tracing                             |
| `@test`       | Mark definitions or modules as test-only             |
| `@no_prelude` | Skip implicit prelude import                         |

## Desugaring

| Surface                                    | Desugared                                                                   |
|:------------------------------------------ |:--------------------------------------------------------------------------- |
| `@static x = file.read ...`               | Compile-time evaluation                                                     |
| `@native "mod.fn"\nf : A -> B`            | Auto-generates `f __arg0 = mod.fn __arg0` (type signature required)        |
| `@native "crate::fn"\nf : A -> B`         | Auto-generates bridge fn; AOT-only                                          |

---

## `@static` — Compile-Time Evaluation

<!-- quick-info: {"kind":"decorator","name":"@static"} -->
`@static` evaluates deterministic source reads at compile time and embeds the value into the program as a constant. No runtime overhead.
<!-- /quick-info -->

### Syntax

```aivi
@static
binding = source.call "argument"
```

### Supported Sources (v0.1)

| Source call                    | Result type       | Description                                        |
|:------------------------------ |:----------------- |:-------------------------------------------------- |
| `file.read "path"`             | `Text`            | Embed file contents as text                        |
| `file.json "path"`             | inferred from use | Parse JSON, embed as typed value                   |
| `file.csv "path"`              | `List { ... }`    | Parse CSV, embed as list of records                |
| `env.get "KEY"`                | `Text`            | Embed environment variable value                   |
| `openapi.fromUrl url`          | typed module      | Generate typed API client from an OpenAPI spec URL |
| `openapi.fromFile "path"`      | typed module      | Generate typed API client from a local spec file   |

### Examples

<<< ../snippets/from_md/syntax/decorators/compile_time_embedding.aivi{aivi}

```aivi
@static
buildEnv = env.get "AIVI_BUILD_ENV"

@static
petStore = openapi.fromUrl ~url(https://petstore.swagger.io/v2/swagger.json)

@static
internalApi = openapi.fromFile "./specs/internal-api.yaml"
```

### Semantics

- Compilation **fails early** if a static source cannot be read, fetched, or decoded.
- The embedded value is a **constant** — no I/O happens at runtime.
- File paths are resolved relative to the source file first, then the workspace root.
- `@static` bindings must be **parameterless** (no function parameters).

### Compile-Time Errors

| Code  | Condition                                      |
|:----- |:---------------------------------------------- |
| E1514 | `@static` applied to a parameterised binding   |
| E1515 | File read failure                              |
| E1516 | JSON parse failure                             |
| E1517 | CSV parse failure                              |
| E1518 | OpenAPI spec fetch/read failure                |
| E1519 | OpenAPI spec parse failure (invalid schema)    |
| E1520 | Unsupported OpenAPI feature in type mapping    |

### OpenAPI Source

<!-- quick-info: {"kind":"topic","name":"openapi compile-time source"} -->
`openapi.fromUrl` and `openapi.fromFile` parse an [OpenAPI 3.x](https://spec.openapis.org/oas/v3.1.1.html) spec at compile time and generate a typed, callable API client.
<!-- /quick-info -->

The generated value is a **factory function** that takes a configuration record and returns a record of callable endpoint functions:

```aivi
@static
petStoreApi = openapi.fromFile "./petstore.json"

// Create a client with config
client = petStoreApi { bearerToken: Some "sk-...", baseUrl: None, headers: None, timeoutMs: None, retryCount: None, strictStatus: None }

// Call an endpoint — returns Source RestApi (Result Error Response)
pets <- client.listPets { limit: Some 10 }

// Destructuring also works
{ listPets, createPets } = petStoreApi { bearerToken: None, baseUrl: None, headers: None, timeoutMs: None, retryCount: None, strictStatus: None }
result <- listPets {}
```

**Config Record Fields:**

| Field          | Type            | Description                                          |
|:-------------- |:--------------- |:---------------------------------------------------- |
| `bearerToken`  | `Option Text`   | Bearer token for `Authorization` header              |
| `headers`      | `Option (List (Text, Text))` | Additional HTTP headers (key-value pairs) |
| `timeoutMs`    | `Option Int`    | Request timeout in milliseconds                      |
| `retryCount`   | `Option Int`    | Number of retries on failure                         |
| `strictStatus` | `Option Bool`   | Treat non-2xx responses as errors                    |
| `baseUrl`      | `Option Text`   | Override the base URL from the spec                  |

**Endpoint Parameters:**

Each endpoint function takes a record of parameters. Parameters from the OpenAPI spec (path, query, header) are mapped by name. For `POST`/`PUT`/`PATCH` endpoints, any extra fields become the JSON request body. Required parameters are direct fields; optional parameters are `Option T`.

**Type Mapping:**

| OpenAPI Type                      | AIVI Type     |
|:--------------------------------- |:------------- |
| `string`                          | `Text`        |
| `integer` / `int32` / `int64`     | `Int`         |
| `number` / `float` / `double`     | `Float`       |
| `boolean`                         | `Bool`        |
| `array` of `T`                    | `List T`      |
| `object` (with properties)        | closed record |
| `$ref`                            | named type    |
| nullable / not required           | `Option T`    |
| `oneOf` / `anyOf`                 | sum type (ADT)|
| `enum` (strings)                  | sum type      |
| `string` with `format: date`      | `Date`        |
| `string` with `format: date-time` | `DateTime`    |

Endpoint functions are named from `operationId` (lowerCamelCase); if absent, derived from method + path. Cached in `.aivi-cache/openapi/`; pass `--refresh-static` to force re-fetch. Accepts `.json`, `.yaml`, `.yml`, and Swagger 2.0 (auto-converted).

---

## `@native` — Native Function Bindings

<!-- quick-info: {"kind":"decorator","name":"@native"} -->
`@native` binds an AIVI definition to a native function. Two forms exist: **runtime natives** (dot-path, resolved at runtime) and **crate natives** (double-colon path, auto-bridged at build time).
<!-- /quick-info -->

### Runtime Natives (dot-path)

Runtime natives bind to functions registered in the AIVI runtime's global environment.

#### Syntax

```aivi
@native "module.functionName"
binding : TypeSig
```

No dummy body is required — the type signature is sufficient. If a def body is provided, it is replaced by the native binding.

#### Example

<<< ../snippets/from_md/syntax/decorators/native_basic.aivi{aivi}

#### Rules

1. Must be **top-level**.
2. Must have an **explicit type signature**.
3. The target path must be a **valid dotted identifier** (e.g. `"gtk4.windowPresent"`).

The `@native` string is a **runtime record path**. The first segment selects a module record registered in the global environment (see `register_builtins` in `core.rs`); subsequent segments are field accesses into it. The module name is chosen by the runtime and may differ from the Cargo crate name (e.g. crate `rusqlite` is exposed as `database`).

#### Adding a Runtime Native Binding

<<< ../snippets/from_md/syntax/decorators/native_walkthrough.aivi{aivi}

Steps: create `crates/aivi/src/runtime/builtins/<mod>.rs` returning `Value::Record`, register via `env.set("mod", build_mod_record())` in `register_builtins`, add Cargo dependency.

### Crate Natives (double-colon path)

Crate natives bind directly to functions in Rust crates declared in the project's `Cargo.toml`. The compiler **auto-generates** a Rust bridge function that marshals AIVI `Value` types to and from Rust types based on the type signature.

#### Syntax

```aivi
@native "crate_name::path::function"
binding : TypeSig
```

No dummy body is required — the type signature is sufficient. The function name must be `lowerCamelCase` as with any AIVI function binding.

#### Example

<<< ../snippets/from_md/syntax/decorators/native_crate_basic.aivi{aivi}

#### Rules

1. Must be **top-level**.
2. Must have an **explicit type signature**.
3. The target path uses `::` (Rust path syntax): `"crate_name::module::function"`.
4. The referenced crate must be declared in `Cargo.toml` under `[dependencies]`.
5. **AOT-only** — crate natives require `aivi build`. Using them with `aivi run` (JIT) produces a compile error.

#### Type Mapping

The compiler maps AIVI types to Rust types for the generated bridge:

| AIVI Type             | Rust Type (argument) | Rust Type (return) |
|:----------------------|:---------------------|:-------------------|
| `Text`                | `&str`               | `String`           |
| `Int`                 | `i64`                | `i64`              |
| `Float`               | `f64`                | `f64`              |
| `Bool`                | `bool`               | `bool`             |
| `List T`              | `Vec<T>`             | `Vec<T>`           |
| `Option T`            | `Option<T>`          | `Option<T>`        |
| `Result E A`          | —                    | `Result<A, E>`     |
| `{ a: T, b: U }`     | generated struct     | generated struct   |

For `Result` return types, `Ok(v)` maps to `Ok v` and `Err(e)` maps to `Err (Text.show e)`.

For `Option` return types, `Some(v)` maps to `Some v` and `None` maps to `None`.

#### Cargo.toml Dependency

Crate dependencies are declared directly in the project's `Cargo.toml`:

```toml
[dependencies]
quick-xml = { version = "0.31", features = ["serialize"] }
serde = { version = "1.0", features = ["derive"] }
```

The compiler validates during `aivi build` that every crate referenced by `@native "crate::..."` has a matching entry in `Cargo.toml`. A missing dependency produces:

```
error[E1528]: crate `quick-xml` referenced by @native binding but not found in Cargo.toml [dependencies]
```

#### Bridge Generation

During `aivi build`, the compiler generates a Rust bridge module at `target/aivi-gen/src/native_bridge.rs`. Each crate-native binding produces a function that:

1. Extracts arguments from AIVI `CrateNativeValue` types
2. Converts to the corresponding Rust types
3. Calls the target crate function
4. Wraps the return value back into an AIVI `CrateNativeValue`

The generated bridge is compiled as part of the AOT binary and registered as builtins at startup via `register_crate_natives_on_ctx`.

#### Serde Auto-Mapping for Records

When a crate-native binding's type signature contains **record types** (e.g., `{ name: Text, age: Int }`), the compiler automatically generates `#[derive(Deserialize, Serialize)]` Rust structs in the bridge module. This enables direct binding to serde-based crate APIs like `quick_xml::de::from_str` or `serde_json::from_str` without writing wrapper code.

**How it works:**

1. The compiler scans all crate-native type signatures for record types (including nested records inside `Result`, `Option`, `List`).
2. For each unique record shape (deduplicated by field names and types), it generates a Rust struct with `#[derive(Deserialize, Serialize)]` and `#[serde(rename_all = "camelCase")]`.
3. AIVI `camelCase` field names are converted to Rust `snake_case` in the struct definition.
4. When a binding's return type contains a generated struct, the compiler adds a turbofish type annotation (e.g., `from_str::<MyStruct>(...)`) so Rust can infer the generic parameter.
5. When `serde` is needed, the bridge module adds `use serde::{Deserialize, Serialize};` — you must include `serde` in your `Cargo.toml`.

**Example:**

```aivi
@native "serde_json::from_str"
parseJson : Text -> Result Text { name: Text, age: Int }
```

Generates:

```rust
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiviRecord0 {
    name: String,
    age: i64,
}

fn __crate_native__serde_json__from_str(mut args: Vec<CrateNativeValue>) -> Result<CrateNativeValue, String> {
    let a0 = /* extract Text from args */;
    let result = serde_json::from_str::<AiviRecord0>(&a0);
    /* wrap result back to CrateNativeValue */
}
```

**Required `Cargo.toml` entry when using records:**

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
```

#### Compile-Time Errors

| Code  | Condition                                                          |
|:------|:-------------------------------------------------------------------|
| E1527 | Crate native used in JIT mode (`aivi run`)                         |
| E1528 | Referenced crate not found in `Cargo.toml` `[dependencies]`        |
| E1529 | Unsupported type in crate-native type signature                    |

---

## `@deprecated` — Deprecation Warnings

<!-- quick-info: {"kind":"decorator","name":"@deprecated"} -->
`@deprecated` marks a binding as deprecated. The compiler emits a warning at every call site.
<!-- /quick-info -->

### Syntax

```aivi
@deprecated "migration hint message"
binding = ...
```

### Example

<<< ../snippets/from_md/syntax/decorators/deprecated_example.aivi{aivi}

---

## `@debug` — Structured Debug Tracing

<!-- quick-info: {"kind":"decorator","name":"@debug"} -->
`@debug` is a tooling pragma for compiler-emitted trace logs. No semantic effect unless compiled with `--debug-trace`.
<!-- /quick-info -->

### Syntax

```aivi
@debug
@debug()
@debug(pipes, args, return, time)
```

`@debug` / `@debug()` defaults to function-level timing only. Parameters are order-insensitive; duplicates ignored. Allowed: `pipes`, `args`, `return`, `time`.

### Example

<<< ../snippets/from_md/syntax/decorators/debug_example.aivi{aivi}

When enabled, emits JSONL-friendly structured events: `fn.enter` / `fn.exit` per call, `pipe.in` / `pipe.out` per `|>` step (when `pipes` enabled).

---

## `@no_prelude` — Skip Prelude Import

<!-- quick-info: {"kind":"decorator","name":"@no_prelude"} -->
`@no_prelude` opts a module out of the implicit prelude import.
<!-- /quick-info -->

### Syntax

```aivi
@no_prelude module ModuleName
```

### Example

<<< ../snippets/from_md/syntax/decorators/no_prelude_example.aivi{aivi}

---

## `@test` — Test Declarations

<!-- quick-info: {"kind":"decorator","name":"@test"} -->
`@test` marks a definition as a test case or a module as test-only. Tests are collected by `aivi test` and excluded from production builds.
<!-- /quick-info -->

### Syntax

```aivi
// Test case (description is mandatory)
@test "description of what is tested"
testName = ...

// Test-only module
@test module ModuleName
```

### Example

<<< ../snippets/from_md/syntax/decorators/test_example.aivi{aivi}

### Rules

- A description string is **mandatory** for test cases.
- When applied to a module, the entire module is test-only.
- Tests are discovered and executed by `aivi test`.
- Test-only modules are stripped from production builds.

---

## Mock Expressions

<!-- quick-info: {"kind":"syntax","name":"mock expression"} -->
Mock expressions provide **scoped binding substitution** for testing and isolation. A `mock ... in` expression temporarily replaces a module-level binding within a lexical scope.
<!-- /quick-info -->

### Syntax

```
mock <qualified.path> = <expr>
( mock <qualified.path> = <expr> )*
in <body>
```

- `<qualified.path>` — a dotted identifier referencing an imported binding (e.g. `rest.get`).
- `<expr>` — replacement expression; must type-check against the original binding's type.
- Multiple `mock` lines may precede a single `in`.

### Basic Example

<<< ../snippets/from_md/syntax/mock_expression/basic.aivi{aivi}

### Multiple Mocks

<<< ../snippets/from_md/syntax/mock_expression/multiple.aivi{aivi}

### Scoping Rules

| Rule | Behaviour |
|:-----|:----------|
| **Lexical** | Active only inside the `in <body>` expression |
| **Deep** | Transitive calls see the mock (runtime environment override) |
| **Nestable** | Inner `mock` blocks can re-shadow an outer mock |
| **Restore** | Original binding is restored after `body` completes (even on error) |
| **Qualified only** | Only qualified imported names can be mocked |

### Snapshot Mocks

The `mock snapshot` variant **records real responses** on first run and **replays from snapshot files** on subsequent runs:

<<< ../snippets/from_md/syntax/mock_expression/snapshot.aivi{aivi}

| Mode | What happens |
|:-----|:-------------|
| **First run** (or `aivi test --update-snapshots`) | Calls real function, serializes to `__snapshots__/<test>/<binding>.snap` |
| **Subsequent runs** | Deserializes from `.snap` — no real call, deterministic |
| **Snapshot missing** | Fails: "run with `--update-snapshots`" |

### `assertSnapshot`

```aivi
assertSnapshot : Text -> A -> Effect Text Unit
```

Compares a serialized value against a stored snapshot. Pass `--update-snapshots` to re-record.

```aivi
@test "user formatting"
testFormat = do Effect {
  formatted <- pure (formatUserTable [{ id: 1, name: "Ada" }])
  assertSnapshot "user_table" formatted
}
```

### Compile-Time Errors

| Code | Condition |
|:-----|:----------|
| E1540 | `mock` target is not a qualified path |
| E1541 | `mock` target does not resolve to a known binding |
| E1542 | Mock expression type does not match original type |
| E1543 | `mock snapshot` used with `= expr` (mutually exclusive) |
| E1544 | Expected `in` keyword after mock binding(s) |

---

## Related

- [Testing Module](/stdlib/core/testing) — assertions, test runner, snapshot assertions
