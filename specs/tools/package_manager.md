# Package Manager & Packaging

AIVI packages build on top of Cargo, Rust's standard build and package manager. In practice, an AIVI package is a Cargo package (often called a crate in Rust) with extra AIVI metadata and an AIVI-to-Rust build step.

That means you get familiar Cargo behavior for fetching dependencies, packaging archives, and publishing crates, while `aivi` adds language-aware validation and scaffolding. See the [AIVI CLI spec](cli.md) for the full command reference; this page focuses on project layout and packaging-specific behavior.

## Project structure

A typical AIVI project looks like this:

```text
my-project/
├── aivi.toml        # AIVI-specific configuration
├── Cargo.toml       # Cargo metadata plus AIVI package metadata
├── src/
│   ├── main.aivi    # Entry point for applications
│   └── lib.aivi     # Entry point for libraries
├── .gitignore
└── target/          # Generated artefacts and build output
```

### `aivi.toml`

`aivi.toml` describes the AIVI-facing part of the project. The `[build]` table is optional; the values below are the defaults that `aivi init` writes into new projects.

```toml
[project]
kind = "bin"                # "bin" for apps or "lib" for libraries.
entry = "main.aivi"         # Entry source file inside src/.
language_version = "0.1"    # Optional, but recommended for reproducible packages.

[build]                     # Optional; these are the scaffold defaults.
gen_dir = "target/aivi-gen" # Where generated Rust is written.
rust_edition = "2024"       # Rust edition for generated code.
cargo_profile = "dev"       # Default Cargo profile.
native_ui_target = "portable" # Or "gnome-gtk4-libadwaita".
```

### `Cargo.toml`

`Cargo.toml` stays a normal Cargo manifest. AIVI adds metadata under `[package.metadata.aivi]` so the CLI can validate language version, package kind, and the published AIVI entry file.

## Discovering packages

`aivi search <query>` searches crates.io using the `aivi` keyword (currently via `cargo search "keyword:aivi <query>"`). In practice, packages are discoverable here when they advertise themselves with the `aivi` Cargo keyword; installation still validates `[package.metadata.aivi]`.

## Installing dependencies

`aivi install <spec>` edits the root `Cargo.toml`, asks Cargo to resolve the dependency, and rolls the manifest back if AIVI validation fails.

Installs are strict by default:

- the dependency must declare `[package.metadata.aivi]`
- it must include `language_version` and `kind`
- it must be `kind = "lib"` because applications are not installable as libraries
- if your project declares `[project].language_version` in `aivi.toml`, the dependency must match it exactly

`--no-fetch` updates `Cargo.toml` but skips `cargo fetch`, which is useful in offline or controlled environments.

### Supported dependency specs

- `name` — latest registry version
- `name@version` — specific registry version (also supports `name@latest`)
- `git+https://host/repo(.git)` or `git+https://host/repo(.git)#rev=<sha>` — Git dependency
- `path:../local-crate` or `path:../local-crate/Cargo.toml` — local path dependency

## AIVI package metadata

AIVI packages declare metadata under `[package.metadata.aivi]`. For `aivi package` and `aivi publish`, `language_version`, `kind`, and `entry` must all be present and match `aivi.toml`.

Library package:

```toml
[package.metadata.aivi]
language_version = "0.1"
kind = "lib"
entry = "src/lib.aivi"
```

Application package:

```toml
[package.metadata.aivi]
language_version = "0.1"
kind = "bin"
entry = "src/main.aivi"
```

## Packaging and publishing

AIVI delegates the actual archive creation and upload steps to Cargo, but adds preflight validation first.

- `aivi package` runs `cargo package` after checking that `aivi.toml` and `Cargo.toml` are present and that `[package.metadata.aivi]` matches `aivi.toml`.
- `aivi publish` runs `cargo publish` with the same checks.
- `--allow-dirty` and `--no-verify` are forwarded to both commands; `--dry-run` is forwarded by `aivi publish`.

## Guide: creating and publishing a library

### 1. Create the package

```sh
aivi init my-aivi-lib --lib  # Scaffold a library package.
cd my-aivi-lib
```

This creates a project with `aivi.toml`, `Cargo.toml`, and `src/lib.aivi`.

### 2. Write your library

Edit `src/lib.aivi`:

<<< ../snippets/from_md/runtime/package_manager/write_your_library.aivi{aivi}

### 3. Check package metadata

Make sure `Cargo.toml` includes the package metadata AIVI expects:

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
entry = "src/lib.aivi"
```

### 4. Test locally

```sh
aivi test src  # Run any @test definitions before packaging.
```

See the [AIVI CLI spec](cli.md) for command options and [Testing](../stdlib/core/testing.md) for `@test` authoring.

### 5. Package or publish

```sh
aivi publish --dry-run  # Verify everything first.
aivi publish            # Publish to crates.io.
```

## Guide: using a package in another project

### 1. Install the dependency

```sh
aivi install my-aivi-lib@0.1.0  # Add the dependency and fetch it.
```

### 2. Import it in your code

The import path comes from the library's `module` declaration, not from the Cargo package name. See [Modules](../syntax/modules.md) for import/export syntax details.

<<< ../snippets/from_md/runtime/package_manager/import_in_your_code.aivi{aivi}

### 3. Use a Git dependency when needed

```sh
aivi install "git+https://github.com/user/my-aivi-lib.git#rev=abc123"  # Pin a specific Git revision.
```

### 4. Use a local path during development

```sh
aivi install "path:../my-aivi-lib"  # Work against a local checkout.
```

Path dependencies are especially useful before a package is published.
