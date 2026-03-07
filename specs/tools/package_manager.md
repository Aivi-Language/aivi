# Package Manager & Packaging

AIVI packages build on top of Rust's Cargo ecosystem. In practice, an AIVI package is a Cargo package with extra AIVI metadata and an AIVI-to-Rust build step.

That means you get familiar Cargo behavior for fetching dependencies, packaging archives, and publishing crates, while `aivi` adds language-aware validation and scaffolding.

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

`aivi.toml` describes the AIVI-facing part of the project.

```toml
[project]
kind = "bin"                # "bin" for apps or "lib" for libraries.
entry = "main.aivi"         # Entry source file inside src/.
language_version = "0.1"    # Targeted AIVI language version.

[build]
gen_dir = "target/aivi-gen" # Where generated Rust is written.
rust_edition = "2024"       # Rust edition for generated code.
cargo_profile = "dev"       # Default Cargo profile.
native_ui_target = "portable" # Or "gnome-gtk4-libadwaita".
```

### `Cargo.toml`

`Cargo.toml` stays a normal Cargo manifest. AIVI adds metadata under `[package.metadata.aivi]` so the CLI can validate language version and package kind.

## Discovering packages

`aivi search <query>` searches crates.io and filters the results to packages that declare themselves as AIVI packages.

## Installing dependencies

`aivi install <spec>` edits the root `Cargo.toml` and then asks Cargo to resolve the dependency.

Installs are strict by default:

- the dependency must declare `[package.metadata.aivi]`
- it must include `language_version` and `kind`
- it must be `kind = "lib"` because applications are not installable as libraries
- if your project declares `[project].language_version` in `aivi.toml`, the dependency must match it exactly

`--no-fetch` updates `Cargo.toml` but skips `cargo fetch`, which is useful in offline or controlled environments.

### Supported dependency specs

- `name` — latest registry version
- `name@version` — specific registry version (also supports `name@latest`)
- `git+https://host/repo(.git)#rev=<sha>` — Git dependency
- `path:../local-crate` — local path dependency

## AIVI package metadata

Every AIVI package is a Rust crate that declares metadata like this:

```toml
[package.metadata.aivi]
language_version = "0.1"
kind = "lib"
```

Publishable packages also need an entry file:

```toml
[package.metadata.aivi]
language_version = "0.1"
kind = "bin"
entry = "src/main.aivi"
```

## Packaging and publishing

AIVI delegates the actual archive creation and upload steps to Cargo, but adds preflight validation first.

- `aivi package` runs `cargo package` after checking that `aivi.toml` exists and that `[package.metadata.aivi]` matches it.
- `aivi publish` runs `cargo publish` with the same checks.
- `--dry-run`, `--allow-dirty`, and `--no-verify` are forwarded to Cargo where applicable.

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
