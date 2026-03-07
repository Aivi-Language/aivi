# `@native` — Native Function Bindings

<!-- quick-info: {"kind":"decorator","name":"@native"} -->
`@native` binds an AIVI definition to a native function. Two forms exist: **runtime natives** (dot-path, resolved at runtime) and **crate natives** (double-colon path, auto-bridged at build time).
<!-- /quick-info -->

Use `@native` when an AIVI binding should call functionality implemented outside AIVI.
There are two main cases:

- **runtime natives** for functions already registered in the AIVI runtime
- **crate natives** for Rust functions pulled in through the project's `Cargo.toml`

In both forms, the AIVI type signature is the contract that keeps the binding type-safe.

## Runtime natives (dot-path)

Runtime natives bind to functions registered in the runtime's global environment.
This is the right choice when the function is already exposed by the runtime as a builtin module record.

### Syntax

```aivi
@native "module.functionName"
binding : TypeSig
```

No dummy body is required.
If a body is present, the native binding replaces it.

### Example

<<< ../../snippets/from_md/syntax/decorators/native_basic.aivi{aivi}

```aivi
@native "time.now"
now : Unit -> Instant   // the type signature is required
```

### How resolution works

The string in `@native` is a runtime record path.
The first segment selects a module record in the global environment, and later segments walk through fields on that record.
The runtime-facing module name is chosen by AIVI itself and may differ from the Cargo crate name behind it.

### Rules

1. The binding must be **top-level**.
2. The binding must have an **explicit type signature**.
3. The target string must be a valid dotted identifier such as `"gtk4.windowPresent"`.

### Adding a new runtime native

<<< ../../snippets/from_md/syntax/decorators/native_walkthrough.aivi{aivi}

In practice, adding one means:

1. create a builtin module record in `crates/aivi/src/runtime/builtins/<mod>.rs`
2. register it in `register_builtins`
3. add any required Cargo dependency

---

## Crate natives (double-colon path)

Crate natives call Rust functions from crates listed in the project's `Cargo.toml`.
During `aivi build`, the compiler generates a Rust bridge that converts between AIVI values and Rust values.

### Syntax

```aivi
@native "crate_name::path::function"
binding : TypeSig
```

No dummy body is required.
The AIVI function name still follows normal `lowerCamelCase` conventions.

### Example

<<< ../../snippets/from_md/syntax/decorators/native_crate_basic.aivi{aivi}

```aivi
@native "serde_json::from_str"
parseUser : Text -> Result Text { name: Text, age: Int }   // AOT build generates the bridge
```

### When to use crate natives

Use this form when you want direct access to Rust ecosystem functionality without hand-writing glue code.
Examples include parsers, serializers, and small host-side utility functions.

### Rules

1. The binding must be **top-level**.
2. The binding must have an **explicit type signature**.
3. The target string must use Rust-style `::` path syntax.
4. The referenced crate must appear in `Cargo.toml` under `[dependencies]`.
5. Crate natives are **AOT-only**: they work with `aivi build`, not `aivi run`.

### Type mapping

The compiler maps AIVI types to Rust types in the generated bridge:

| AIVI Type | Rust Type (argument) | Rust Type (return) |
|:--------- |:-------------------- |:------------------ |
| `Text` | `&str` | `String` |
| `Int` | `i64` | `i64` |
| `Float` | `f64` | `f64` |
| `Bool` | `bool` | `bool` |
| `List T` | `Vec<T>` | `Vec<T>` |
| `Option T` | `Option<T>` | `Option<T>` |
| `Result E A` | — | `Result<A, E>` |
| `{ a: T, b: U }` | generated struct | generated struct |

For `Result` returns, `Ok(v)` maps to `Ok v` and `Err(e)` maps to `Err (Text.show e)`.
For `Option` returns, `Some(v)` maps to `Some v` and `None` maps to `None`.

### Cargo dependencies

Referenced crates are declared in the project's `Cargo.toml`:

```toml
[dependencies]
quick-xml = { version = "0.31", features = ["serialize"] }
serde = { version = "1.0", features = ["derive"] }
```

During `aivi build`, the compiler checks that each `@native "crate::..."` reference has a matching dependency entry.
If it does not, compilation fails with an error.

### Bridge generation

For each crate-native binding, `aivi build` generates bridge code in `target/aivi-gen/src/native_bridge.rs` that:

1. extracts arguments from AIVI values
2. converts them into Rust types
3. calls the target Rust function
4. wraps the result back into an AIVI value

The generated bridge is compiled into the final AOT binary and registered during startup.

### Serde auto-mapping for records

If a crate-native signature contains record types, the compiler can generate Rust structs with `Serialize` and `Deserialize` derives for those shapes.
This makes serde-based crates practical to use without writing custom wrappers.

How it works:

1. the compiler scans crate-native signatures for record types, including nested ones
2. each unique record shape becomes a generated Rust struct
3. AIVI `camelCase` field names are converted to Rust `snake_case`
4. the bridge adds the type annotations Rust needs for generic calls
5. when serde support is required, the bridge imports `serde::{Deserialize, Serialize}`

Example:

```aivi
@native "serde_json::from_str"
parseJson : Text -> Result Text { name: Text, age: Int }
```

Generates bridge code along these lines:

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

If you use record mapping, remember to include `serde` in `Cargo.toml`:

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
```

### Compile-time errors

| Code | Condition |
|:---- |:--------- |
| E1527 | Crate native used in JIT mode (`aivi run`) |
| E1528 | Referenced crate not found in `Cargo.toml` `[dependencies]` |
| E1529 | Unsupported type in a crate-native type signature |
