# `@native` — Native Function Bindings

<!-- quick-info: {"kind":"decorator","name":"@native"} -->
`@native` binds an AIVI definition to a native function. Two forms exist: **runtime natives** (dot-path, resolved at runtime) and **crate natives** (double-colon path, auto-bridged at build time).
<!-- /quick-info -->

Use `@native` when an AIVI binding should call functionality implemented outside ordinary AIVI source.
In day-to-day application code, prefer a standard-library wrapper when one already exists, such as [`aivi.system`](../../stdlib/system/system.md) or [`aivi.ui.gtk4`](../../stdlib/ui/gtk4.md).
Reach for raw `@native` mainly when you are:

- extending the AIVI runtime itself, or
- binding to a Rust crate during an AOT build

There are two main forms:

- **runtime natives** for functions already registered in the AIVI runtime
- **crate natives** for Rust functions pulled in through the project's `Cargo.toml`

In both forms, the AIVI type signature is the contract that keeps the binding type-safe.

## Start here: which form should you use?

| Form | Best when | Resolved when |
|:---- |:--------- |:------------- |
| **runtime native** | the function is already exposed by the AIVI runtime as a builtin | when the program runs |
| **crate native** | the function lives in a Rust crate listed in your project dependencies | when `aivi build` generates the native bridge |

An **AOT** (ahead-of-time) build means the program is compiled to a native binary before it runs. Crate natives use that build step, so they work with `aivi build` rather than `aivi run`.

## Runtime natives (dot-path)

Runtime natives bind to functions registered in the runtime's global environment.
This is the right choice when the function is already exposed by the runtime as a builtin module record.

### Syntax

<<< ../../snippets/from_md/syntax/decorators/native/block_01.aivi{aivi}


Placing `@native` on the type signature is enough: the compiler will synthesize the forwarding definition for you.
No dummy body is required.
If a matching body is present, the compiler replaces its expression with the native forwarding call.

### Example

<<< ../../snippets/from_md/syntax/decorators/native/block_02.aivi{aivi}


Call this binding as `rawArgs Unit`.
The friendlier `aivi.system.args` wrapper is implemented the same way, but hides the raw `Unit` argument from normal application code.

### How resolution works

The string in `@native` is a runtime record path.
In plain language, that means a dotted path such as `"system.args"`, `"system.env.get"`, or `"gtk4.windowPresent"`:

- the first segment selects a builtin module record in the global environment,
- later segments walk through fields on that record.

The runtime-facing module name is chosen by AIVI itself and may differ from the Cargo crate name behind it.

### Rules

1. The binding must be **top-level**.
2. The binding must have an **explicit type signature**.
3. The target string must be a valid dotted identifier such as `"gtk4.windowPresent"`.
4. If you keep a placeholder body, its parameters must be plain identifiers rather than destructuring patterns.
5. A zero-parameter signature forwards to a bare field access; a function signature forwards all parameters in order.

### Adding a new runtime native

If you are extending AIVI itself rather than consuming it, adding a new runtime native usually means:

1. implement or extend a builtin record in `crates/aivi/src/runtime/builtins/`
2. register the root record in `register_builtins`
3. add any required Cargo dependency
4. optionally expose a friendlier stdlib wrapper in `crates/aivi/src/stdlib/` if application code should call it directly

---

## Crate natives (double-colon path)

Crate natives call Rust functions from crates listed in the project's `Cargo.toml`.
During `aivi build`, the compiler generates a Rust bridge that converts between AIVI values and Rust values.

### Syntax

<<< ../../snippets/from_md/syntax/decorators/native/block_03.aivi{aivi}


No dummy body is required here either.
Placing `@native` on the type signature alone is enough for the compiler to generate the forwarding definition.
If you do keep a body, the compiler still rewrites it to call the crate-native target.
The AIVI function name still follows normal `lowerCamelCase` conventions.

### Example

<<< ../../snippets/from_md/syntax/decorators/native/block_04.aivi{aivi}


This example needs `serde_json` in the project's `Cargo.toml`.
Because it returns a record, it is also a practical case for adding `serde = { version = "1.0", features = ["derive"] }`.

### When to use crate natives

Use this form when you want direct access to Rust ecosystem functionality without hand-writing glue code.
Examples include parsers, serializers, and small host-side utility functions.

### Rules

1. The binding must be **top-level**.
2. The binding must have an **explicit type signature**.
3. The target string must use Rust-style `::` path syntax.
4. The referenced crate must appear in `Cargo.toml` under `[dependencies]`.
5. Crate natives are **AOT-only**: they work with `aivi build`, not `aivi run`.

### Currently verified bridge mapping

The current bridge generator has a smaller verified surface than "all AIVI types".
Today, the documented auto-conversions are:

**Arguments**

| AIVI Type | Rust Type |
|:--------- |:--------- |
| `Text` | `&str` |
| `Int` | `i64` |
| `Float` | `f64` |
| `Bool` | `bool` |

**Returns**

| AIVI Type | Rust Type |
|:--------- |:--------- |
| `Text` | `String` |
| `Int` | `i64` |
| `Float` | `f64` |
| `Bool` | `bool` |
| `Unit` | `()` |
| `Option T` | `Option<T>` |
| `List T` | `Vec<T>` |
| `Result E A` | `Result<A, E>` |
| `{ a: T, b: U }` | generated Rust struct |

For `Result` returns, `Ok(v)` becomes AIVI `Ok v`.
`Err(e)` becomes AIVI `Err ...` with the Rust error formatted to `Text` via Rust's `format!("{e}")`.
For `Option` returns, `Some(v)` maps to `Some v` and `None` maps to `None`.

Treat other signature shapes as implementation details for now and verify the generated bridge if you rely on them.

### Cargo dependencies

Referenced crates are declared in the project's `Cargo.toml`:

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
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

When a crate-native signature uses record shapes that the bridge can materialize, the compiler can generate Rust structs with `Serialize` and `Deserialize` derives for those shapes.
This is especially useful for serde-driven return values such as `serde_json::from_str`.

How it works:

1. the compiler scans crate-native signatures for record shapes, including nested ones
2. each unique record shape becomes a generated Rust struct
3. AIVI `camelCase` field names are converted to Rust `snake_case` with per-field serde rename attributes when needed
4. the bridge adds the type annotations Rust needs for generic calls such as `serde_json::from_str::<...>`
5. when serde support is required, the bridge imports `serde::{Deserialize, Serialize}`

Example:

<<< ../../snippets/from_md/syntax/decorators/native/block_05.aivi{aivi}


Generates bridge code along these lines:

```rust
#[derive(Deserialize, Serialize)]
struct __NativeStruct0 {
    #[serde(rename = "imapHost")]
    imap_host: String,
    #[serde(rename = "imapPort")]
    imap_port: i64,
    #[serde(rename = "useSsl")]
    use_ssl: bool,
}

fn __crate_native__serde_json__from_str(mut args: Vec<CrateNativeValue>) -> Result<CrateNativeValue, String> {
    let a0 = /* extract Text from args */;
    let result = serde_json::from_str::<__NativeStruct0>(&a0);
    /* wrap result back to CrateNativeValue */
}
```

If you use record mapping, remember to include the crates the bridge relies on:

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### Compile-time errors

| Code | Condition |
|:---- |:--------- |
| E1511 | `@native` is missing its required string argument |
| E1526 | `@native` is used outside a top-level definition, is missing a type signature, uses an invalid target path, or keeps non-identifier parameters |
| E1527 | Crate native used in JIT mode (`aivi run`) |
| E1528 | Referenced crate not found in `Cargo.toml` `[dependencies]` |

Rust compilation can still fail after these checks if the referenced Rust function's real signature does not line up with the bridge generated from your AIVI type signature.
