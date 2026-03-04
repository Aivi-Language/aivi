# `@native` — Native Function Bindings

<!-- quick-info: {"kind":"decorator","name":"@native"} -->
`@native` binds an AIVI definition to a native function. Two forms exist: **runtime natives** (dot-path, resolved at runtime) and **crate natives** (double-colon path, auto-bridged at build time).
<!-- /quick-info -->

## Runtime Natives (dot-path)

Runtime natives bind to functions registered in the AIVI runtime's global environment.

### Syntax

```aivi
@native "module.functionName"
binding : TypeSig
```

No dummy body is required — the type signature is sufficient. If a def body is provided, it is replaced by the native binding.

### Example

<<< ../../snippets/from_md/syntax/decorators/native_basic.aivi{aivi}

### Rules

1. Must be **top-level**.
2. Must have an **explicit type signature**.
3. The target path must be a **valid dotted identifier** (e.g. `"gtk4.windowPresent"`).

The `@native` string is a **runtime record path**. The first segment selects a module record registered in the global environment (see `register_builtins` in `core.rs`); subsequent segments are field accesses into it. The module name is chosen by the runtime and may differ from the Cargo crate name (e.g. crate `rusqlite` is exposed as `database`).

### Adding a Runtime Native Binding

<<< ../../snippets/from_md/syntax/decorators/native_walkthrough.aivi{aivi}

Steps: create `crates/aivi/src/runtime/builtins/<mod>.rs` returning `Value::Record`, register via `env.set("mod", build_mod_record())` in `register_builtins`, add Cargo dependency.

---

## Crate Natives (double-colon path)

Crate natives bind directly to functions in Rust crates declared in the project's `Cargo.toml`. The compiler **auto-generates** a Rust bridge function that marshals AIVI `Value` types to and from Rust types based on the type signature.

### Syntax

```aivi
@native "crate_name::path::function"
binding : TypeSig
```

No dummy body is required — the type signature is sufficient. The function name must be `lowerCamelCase` as with any AIVI function binding.

### Example

<<< ../../snippets/from_md/syntax/decorators/native_crate_basic.aivi{aivi}

### Rules

1. Must be **top-level**.
2. Must have an **explicit type signature**.
3. The target path uses `::` (Rust path syntax): `"crate_name::module::function"`.
4. The referenced crate must be declared in `Cargo.toml` under `[dependencies]`.
5. **AOT-only** — crate natives require `aivi build`. Using them with `aivi run` (JIT) produces a compile error.

### Type Mapping

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

### Cargo.toml Dependency

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

### Bridge Generation

During `aivi build`, the compiler generates a Rust bridge module at `target/aivi-gen/src/native_bridge.rs`. Each crate-native binding produces a function that:

1. Extracts arguments from AIVI `CrateNativeValue` types
2. Converts to the corresponding Rust types
3. Calls the target crate function
4. Wraps the return value back into an AIVI `CrateNativeValue`

The generated bridge is compiled as part of the AOT binary and registered as builtins at startup via `register_crate_natives_on_ctx`.

### Serde Auto-Mapping for Records

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

### Compile-Time Errors

| Code  | Condition                                                          |
|:------|:-------------------------------------------------------------------|
| E1527 | Crate native used in JIT mode (`aivi run`)                         |
| E1528 | Referenced crate not found in `Cargo.toml` `[dependencies]`        |
| E1529 | Unsupported type in crate-native type signature                    |
