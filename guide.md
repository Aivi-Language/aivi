# Guide

## Prerequisites

- Rust toolchain (stable). Install via `rustup` if needed.

## Build

```bash
cargo build
```

## Run

```bash
# Show help
cargo run -p aivi -- --help

# Parse a single file
cargo run -p aivi -- parse examples/hello.aivi

# Parse all .aivi files under a directory (recursive)
cargo run -p aivi -- parse examples/...

# Check module name resolution
cargo run -p aivi -- check examples/...

# Desugar to a kernel-friendly HIR (JSON)
cargo run -p aivi -- desugar examples/...

# Format a single file
cargo run -p aivi -- fmt examples/hello.aivi

# Start the language server (requires aivi-lsp in PATH)
cargo run -p aivi -- lsp

# Build a WASM artifact (expects a main definition)
cargo run -p aivi -- build path/to/module.aivi --target wasm32-wasi --out target/aivi.wasm

# Run the WASM artifact under Wasmtime
cargo run -p aivi -- run path/to/module.aivi --target wasm32-wasi

# Example WASM run
cargo run -p aivi -- run examples/10_wasm.aivi --target wasm32-wasi

# Run using the native effect runtime (M6)
cargo run -p aivi -- run examples/11_concurrency.aivi --target native

# Run JSX + domains + patching demo (M7)
cargo run -p aivi -- run examples/12_m7.aivi --target native
```

## Test

```bash
cargo test
```

## Notes

- The WASM backend is intentionally minimal: it supports basic literals, simple arithmetic,
- The WASM backend is intentionally minimal: it supports basic literals, simple arithmetic,
  `if`, blocks with simple bindings, and `print` for `Text`. It currently expects a single
  module and a `main` definition. More features will land in later phases.
- The native runtime is the current home for effects, resources, and concurrency. It is
  intentionally minimal and does not yet cover the full surface language.
