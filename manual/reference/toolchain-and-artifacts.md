# Toolchain and Artifacts

This page describes the toolchain and command outputs that the repository ships today. For
language semantics, use the other [reference pages](/reference/).

## Supported development toolchain

| Component | Repository contract |
| --- | --- |
| Rust | 1.98.0, pinned by `rust-toolchain.toml` |
| Rust edition | 2024 |
| Cargo resolver | 3 |
| Node.js | 24 for documentation and editor tooling |
| pnpm | 10.19.0 in CI |
| Desktop stack | GTK 4, libadwaita, GLib/GIO, and WebKitGTK 6 |

Rustup reads `rust-toolchain.toml` automatically. A Debian or Ubuntu development machine needs
the same native headers as CI:

```sh
sudo apt-get install libgtk-4-dev libadwaita-1-dev libwebkitgtk-6.0-dev \
  libssl-dev libdbus-1-dev libsecret-1-dev
```

Build the developer CLI from the repository root:

```sh
cargo build --bin aivi
target/debug/aivi --version
```

## CLI commands and outputs

| Command | Result |
| --- | --- |
| `aivi check [path]` | Parses, resolves, and type-checks a file, directory, or manifest entries. No executable artifact is produced. |
| `aivi check --runnable [path] [--view name]` | Checks the selected app through runnable lowering, the GTK catalog, and native bundle serialization in memory. Does not launch or activate sources. |
| `aivi compile <path> [-o object]` | Emits a native object file. It does not link a runnable GTK application. |
| `aivi build <path> -o <executable>` | Produces one directly runnable executable containing a source-free application bundle. |
| `aivi run [path]` | Compiles and launches a GTK application, or launches a compatible serialized run image. |
| `aivi execute <path> [-- args...]` | Runs an exported `main : Task ...` without GTK. |
| `aivi test <path> [name]` | Runs every `@test` value in the workspace, or one exact test value from the requested file. |
| `aivi fmt` | Prints canonical formatting or checks files with `--check`. |
| `aivi lsp` | Serves Language Server Protocol messages over stdio. |
| `aivi mcp` | Serves live application introspection tools over stdio. |
| `aivi manual-snippets` | Checks and optionally formats fenced AIVI examples in the manual. |

Run `aivi help <command>` for the complete argument contract.

For documentation, `aivi manual-snippets --preserve-format` checks the examples exactly as
written, including deliberate shorthand and expanded teaching forms. Without that option,
the command checks canonically formatted code and fails when formatting changes are needed;
`--write` applies those changes. `--write` and `--preserve-format` are mutually exclusive.
All modes write a diagnostic report and check syntax and types, not runtime results.

## Runnable artifacts

Use `aivi check --runnable demos/stockroom/main.aivi` before packaging an application. This
uses the same eager run preparation and native frozen bundle encoder as `build`, without
creating an executable or opening a display. `--view name` selects an entry when the module
contains multiple views. With no path, manifest apps are checked; recursive directories are
only supported by ordinary `check`, since they also contain library modules.

A successful runnable check verifies compilation and serialization for the current target.
It does not promise that external files exist, servers are reachable, or another machine has
the required desktop libraries. Ordinary `check` still checks syntax and types for libraries;
it does not claim native-backend coverage.

`aivi compile` and `aivi build` are intentionally different:

- `compile` stops after Cranelift object emission. Linking remains the caller's responsibility.
- `build` validates the runnable surface and appends a source-free bundle to the current AIVI
  runtime executable. The resulting file still depends on the target system's GTK stack.

Frozen run images and backend payloads use typed, versioned binary envelopes. Decoding is bounded
to 256 MiB, trailing bytes are rejected, and unknown formats or versions fail with an instruction
to rebuild. Serialized artifacts are therefore build outputs, not a long-term interchange format.
Do not edit them or commit compatibility assumptions about their internal version numbers.

## Caches

Live source runs use a versioned cache below the platform cache home, normally
`$XDG_CACHE_HOME/aivi` or `$HOME/.cache/aivi`. Backend program and JIT artifacts live below
`aivi/compiled`. Cache keys include semantic fingerprints plus compiler and target information.
A corrupt, stale, or incompatible entry is a cache miss and is rebuilt; it must never change
language behavior.

New projects also ignore a workspace-local `.aivi-cache` directory. Treat every cache as
disposable.

## Timing output

`aivi check --timings <path>` and `aivi run --timings <path>` print stage timing information to
stderr. These reports are useful for finding a phase to benchmark, but they are not substitutes
for repeatable Criterion measurements. See [Benchmarking](/reference/benchmarking).
