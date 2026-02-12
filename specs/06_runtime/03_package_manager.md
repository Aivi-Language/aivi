# Package Manager (Cargo-backed)

The AIVI CLI uses Cargo as the dependency resolver and build tool. AIVI sources
live in `src/`, and generated Rust is written to `target/aivi-gen/`.

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
