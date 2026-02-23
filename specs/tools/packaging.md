# Packaging & Project Structure

AIVI piggybacks on Rust's `cargo` ecosystem for packaging and dependency management. An AIVI project is essentially a Rust project with additional metadata and build steps.

## File Structure

A typical AIVI project looks like this:

```text
my-project/
├── aivi.toml        # AIVI-specific configuration
├── Cargo.toml       # Rust/Cargo configuration
├── src/
│   ├── main.aivi    # Entry point (for binaries)
│   └── lib.aivi     # Entry point (for libraries)
├── .gitignore
└── target/          # Build artifacts
```

## `aivi.toml`

The `aivi.toml` file configures the AIVI compiler settings for the project.

```toml
[project]
kind = "bin"              # "bin" or "lib"
entry = "main.aivi"       # Entry source file
language_version = "0.1"  # Targeted AIVI version

[build]
gen_dir = "target/aivi-gen" # Where generated Rust code is placed
rust_edition = "2024"       # Rust edition for generated code
cargo_profile = "dev"       # Default cargo profile
native_ui_target = "portable" # "portable" (default) or "gnome-gtk4-libadwaita"
```

## `Cargo.toml` Integration

AIVI projects are valid Cargo packages. The `Cargo.toml` file contains standard Rust package metadata and dependencies.

### Metadata

AIVI stores its specific metadata under `[package.metadata.aivi]`:

```toml
[package.metadata.aivi]
language_version = "0.1"
kind = "bin"
entry = "src/main.aivi"
```

### Dependencies

Dependencies are managed via `Cargo.toml`'s `[dependencies]` section. You can use standard Rust crates or other AIVI packages.

```toml
[dependencies]
aivi_native_runtime = { path = "..." } # Runtime for generated Rust code (native backend; experimental)
my-aivi-lib = { path = "../my-aivi-lib" } # Another AIVI package
```

## Compilation Model

When you run `aivi build`:

1. **Aivi Compilation**: The `aivi` compiler reads `src/*.aivi` files, type-checks them, and compiles them into Rust code.
2. **Code Generation**: The generated Rust code is written to `target/aivi-gen/src`.
3. **Rust Compilation**: `cargo build` is invoked in the project root, compiling the generated sources referenced by your `Cargo.toml`.

This architecture allows Aivi to leverage the full power of the Rust ecosystem, including optimized compilation, linking, and native interoperability.
