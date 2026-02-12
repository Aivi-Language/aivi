# Packaging & Project Structure

Aivi piggybacks on Rust's `cargo` ecosystem for packaging and dependency management. An Aivi project is essentially a Rust project with additional metadata and build steps.

## File Structure

A typical Aivi project looks like this:

```text
my-project/
├── aivi.toml        # Aivi-specific configuration
├── Cargo.toml       # Rust/Cargo configuration
├── src/
│   └── main.aivi    # Entry point (for binaries)
│   └── lib.aivi     # Entry point (for libraries)
├── .gitignore
└── target/          # Build artifacts
```

## `aivi.toml`

The `aivi.toml` file configures the Aivi compiler settings for the project.

```toml
[project]
kind = "bin"              # "bin" or "lib"
entry = "main.aivi"       # Entry source file
language_version = "0.1"  # Targeted Aivi version

[build]
gen_dir = "target/aivi-gen" # Where generated Rust code is placed
rust_edition = "2024"       # Rust edition for generated code
cargo_profile = "dev"       # Default cargo profile
codegen = "native"          # "native" (standalone Rust) or "embed" (HIR+interpreter)
```

## `Cargo.toml` Integration

Aivi projects are valid Cargo packages. The `Cargo.toml` file contains standard Rust package metadata and dependencies.

### Metadata

Aivi stores its specific metadata under `[package.metadata.aivi]`:

```toml
[package.metadata.aivi]
language_version = "0.1"
kind = "bin"
entry = "src/main.aivi"
```

### Dependencies

Dependencies are managed via `Cargo.toml`'s `[dependencies]` section. You can use standard Rust crates or other Aivi packages.

```toml
[dependencies]
aivi = { path = "..." } # Embed backend runtime (HIR + interpreter)
aivi_native_runtime = { path = "..." } # Native backend runtime (standalone Rust; experimental)
serde_json = "1.0"      # Standard Rust crate
my-aivi-lib = { path = "../my-aivi-lib" } # Another Aivi package
```

## Compilation Model

When you run `aivi build`:

1.  **Aivi Compilation**: The `aivi` compiler reads `src/*.aivi` files, type-checks them, and compiles them into Rust code.
2.  **Code Generation**: The generated Rust code is written to `target/aivi-gen/src`.
3.  **Rust Compilation**: `cargo build` is invoked on the generated Rust project in `target/aivi-gen`.

This architecture allows Aivi to leverage the full power of the Rust ecosystem, including optimized compilation, linking, and native interoperability.

## Codegen Backends (v0.1)

AIVI v0.1 supports two Rust codegen modes for projects:

- `codegen = "embed"`: Generates Rust that embeds the program (HIR) and executes it with the interpreter runtime.
  This is the most complete backend today.
- `codegen = "native"`: Generates standalone Rust logic (experimental). Generated code depends on `aivi_native_runtime`
  instead of the interpreter.

Select the backend in `aivi.toml` under `[build]`.
