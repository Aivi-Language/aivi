# Bootstrapping an AIVI project

This guide covers the current Cargo-backed workflow: `aivi init` scaffolds a project, and `aivi build` / `aivi run` generate Rust into `target/aivi-gen/` and then delegate to Cargo.

## Prerequisites

- Rust toolchain (stable). Install via `rustup` if needed.

## Install the CLI

From this repo:

```bash
cargo install --path crates/aivi
```

Optional (for `aivi lsp`):

```bash
cargo install --path crates/aivi_lsp
```

## Create a new project

```bash
# Binary project (writes `src/main.aivi`)
aivi init my-app --bin

# Library project (writes `src/lib.aivi`)
aivi init my-lib --lib
```

Project layout:

- `aivi.toml`: AIVI project config (entrypoint, generated Rust dir, Rust edition, Cargo profile).
- `Cargo.toml`: Standard Cargo manifest wired to the generated Rust (`target/aivi-gen/src/...`).
- `src/*.aivi`: Your AIVI sources (entry file is `main.aivi` or `lib.aivi` by default).

## Build and run

```bash
cd my-app

# Generates Rust into `target/aivi-gen/` and then runs `cargo build`
aivi build

# Generates Rust into `target/aivi-gen/` and then runs `cargo run`
aivi run

# Use `--release` (or set `build.cargo_profile = "release"` in `aivi.toml`)
aivi build --release
aivi run --release

# Forward args to cargo (note: cargo uses its own `--` separator)
aivi run -- -- --help
```

Clean generated Rust:

```bash
# Removes `build.gen_dir` (default: `target/aivi-gen`)
aivi clean

# Also runs `cargo clean`
aivi clean --all
```

## Add AIVI dependencies

`aivi install` edits `[dependencies]` in `Cargo.toml`, validates the dependency has
`[package.metadata.aivi]` (and `kind = "lib"`), and (by default) runs `cargo fetch`.
If your project declares `language_version` in `aivi.toml`, installed AIVI dependencies
must match it.

```bash
cd my-app

aivi install aivi-foo@^0.1
aivi install aivi-bar@latest
aivi install git+https://github.com/owner/aivi-baz.git#rev=<sha>
aivi install path:../local/aivi-qux

# Offline / no network
aivi install aivi-bar@latest --no-fetch

# Install embedded stdlib modules into src/
aivi install aivi.number

# Search crates.io for AIVI packages
aivi search prelude
```

## Package & publish (Cargo-backed)

`aivi package` and `aivi publish` are thin wrappers around Cargo with AIVI-specific
preflight validation (checks `aivi.toml` and `[package.metadata.aivi]` consistency):

```bash
# Build the `.crate` archive locally
aivi package

# Dry-run a publish (no upload)
aivi publish --dry-run
```

## Editor support (LSP)

`aivi lsp` spawns an `aivi-lsp` binary from your `PATH`:

```bash
aivi lsp
```

If you’re developing from this repo, `cargo install --path crates/aivi_lsp` is the simplest way to make `aivi-lsp` available.

## Compiler/introspection commands

These operate on a file or on a directory target like `integration-tests/...` (recursive):

```bash
aivi parse integration-tests/legacy/hello.aivi
aivi check integration-tests/...
aivi fmt integration-tests/legacy/hello.aivi
aivi desugar integration-tests/...
aivi kernel integration-tests/legacy/hello.aivi
aivi rust-ir integration-tests/legacy/hello.aivi
```

## Notes / current limitations

- `aivi build` / `aivi run` currently expect a **single module** in the program (keep projects to one `module ...` for now).
- The runtime entrypoint is a definition named `main` and it must evaluate to an `Effect` value.
