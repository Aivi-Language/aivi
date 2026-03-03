# Package Manager & Packaging

AIVI piggybacks on Rust's `cargo` ecosystem for packaging, dependency management,
and publishing. An AIVI project is essentially a Rust project with additional
metadata and build steps. AIVI sources live in `src/`, and compiled artefacts
are written to `target/`.

## Project Structure

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

### `aivi.toml`

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

### `Cargo.toml` Integration

AIVI projects are valid Cargo packages. The `Cargo.toml` file contains standard
Rust package metadata and dependencies.

## Package Discovery

- `aivi search <query>` searches crates.io with the `aivi` keyword and only
  presents AIVI packages.

## Installing Dependencies

- `aivi install <spec>` edits `[dependencies]` in the root `Cargo.toml`, then
  resolves the dependency with Cargo.
- Installs are **strict by default**:
  - the dependency must declare `[package.metadata.aivi]` with at least
    `language_version` and `kind`
  - dependencies must be `kind = "lib"` (applications are not installable)
  - if the project declares `[project].language_version` in `aivi.toml`, the
    dependency's `language_version` must match exactly
- `--no-fetch` edits `Cargo.toml` but skips `cargo fetch` (useful for offline
  workflows; validation happens via `cargo metadata`).

### Dependency Specs

Supported `<spec>` formats:

- `name` (registry; defaults to `*`)
- `name@version` (registry; also supports `name@latest`)
- `git+https://host/repo(.git)#rev=<sha>` (git)
- `path:../local-crate` (path; the package name is inferred from the dependency's
  `Cargo.toml` if available)

## AIVI Package Metadata

An AIVI package is a Rust crate that declares:

```toml
[package.metadata.aivi]
language_version = "0.1"
kind = "lib"
```

For publishable packages, the `entry` field is also required:

```toml
[package.metadata.aivi]
language_version = "0.1"
kind = "bin"
entry = "src/main.aivi"
```

## Packaging & Publishing

AIVI v0.1 intentionally delegates packaging and publishing to Cargo, but adds a
small amount of preflight validation.

- `aivi package` runs `cargo package` after verifying that:
  - `aivi.toml` exists and parses
  - `Cargo.toml` contains `[package.metadata.aivi]` consistent with `aivi.toml`
- `aivi publish` runs `cargo publish` with the same preflight validation.
  - `--dry-run` forwards to `cargo publish --dry-run`
  - `--allow-dirty` and `--no-verify` forward to Cargo

## Guide: Creating and Publishing an AIVI Package

### 1. Initialize a new library

```sh
mkdir my-aivi-lib && cd my-aivi-lib
aivi init --lib
```

This creates:

```
my-aivi-lib/
  aivi.toml          # project manifest
  Cargo.toml         # auto-generated, includes [package.metadata.aivi]
  src/
    lib.aivi          # library entry point
```

### 2. Write your library

Edit `src/lib.aivi`:

<<< ../snippets/from_md/runtime/package_manager/write_your_library.aivi{aivi}


### 3. Configure metadata

Ensure `Cargo.toml` has:

```toml
[package]
name = "my-aivi-lib"
version = "0.1.0"
edition = "2024"
keywords = ["aivi"]
description = "A small AIVI utility library."

[package.metadata.aivi]
language_version = "0.1"
kind = "lib"
```

### 4. Test locally

```sh
aivi test
```

### 5. Publish

```sh
aivi publish --dry-run    # verify everything first
aivi publish              # publish to crates.io
```

## Guide: Using an AIVI Package

### 1. Install a dependency

```sh
aivi install my-aivi-lib@0.1.0
```

This adds the dependency to `Cargo.toml` and fetches it.

### 2. Import in your code

<<< ../snippets/from_md/runtime/package_manager/import_in_your_code.aivi{aivi}


### 3. Use a Git dependency

```sh
aivi install "git+https://github.com/user/my-aivi-lib.git#rev=abc123"
```

### 4. Use a local path dependency

```sh
aivi install "path:../my-aivi-lib"
```

This is useful during development when the package is not yet published.
