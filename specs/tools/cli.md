# AIVI CLI

The AIVI Command Line Interface (CLI) is the primary tool for managing AIVI projects. It handles project creation, dependency management, compilation, and execution.

## Installation

The CLI is distributed as a single binary named `aivi`. It can be installed directly from source.

## Commands

### Version

#### `version`

Prints CLI and language version information.

```bash
aivi version
aivi --version
aivi -V
```

Running plain `aivi` also shows this version header before the command help.

### Project Management

#### `init` / `new`

Creates a new AIVI project in a directory.

```bash
aivi init <name> [--bin|--lib] [--edition 2024] [--language-version 0.1] [--force]
```

- `<name>`: The name of the project.
- `--bin`: Create a binary (application) project (default).
- `--lib`: Create a library project.
- `--edition`: Set the Rust edition (default: 2024).
- `--language-version`: Set the AIVI language version (default: 0.1).
- `--force`: Force creation even if the directory is not empty.

#### `install`

Installs a dependency into the current project.

```bash
aivi install <spec> [--no-fetch]
```

- `<spec>`: The dependency specification.
  - `name`: Installs the latest version from the registry.
  - `name@version`: Installs a specific version.
  - `git+https://github.com/user/repo`: Installs from a Git repository.
  - `path:../local-crate`: Installs from a local path.
- Validation: installs are strict by default. The dependency must declare
  `[package.metadata.aivi]` with `language_version` and `kind = "lib"`. If the
  project declares `language_version` in `aivi.toml`, it must match.
- `--no-fetch`: Updates `Cargo.toml` but skips running `cargo fetch`.

#### `search`

Searches for AIVI packages in the registry.

```bash
aivi search <query>
```

#### `package`

Packages the current project using Cargo (creates a `.crate` archive under `target/package`).

```bash
aivi package [--allow-dirty] [--no-verify] [-- <cargo args...>]
```

- Preflight validation: requires `aivi.toml` and `[package.metadata.aivi]` in `Cargo.toml`.
- `--allow-dirty`: Forwards to `cargo package --allow-dirty`.
- `--no-verify`: Forwards to `cargo package --no-verify`.
- `<cargo args...>`: Additional arguments forwarded to `cargo package` after `--`.

#### `publish`

Publishes the current project to crates.io via Cargo.

```bash
aivi publish [--dry-run] [--allow-dirty] [--no-verify] [-- <cargo args...>]
```

- Preflight validation: requires `aivi.toml` and `[package.metadata.aivi]` in `Cargo.toml`.
- `--dry-run`: Forwards to `cargo publish --dry-run`.
- `--allow-dirty`: Forwards to `cargo publish --allow-dirty`.
- `--no-verify`: Forwards to `cargo publish --no-verify`.
- `<cargo args...>`: Additional arguments forwarded to `cargo publish` after `--`.

#### `clean`

Cleans build artifacts.

```bash
aivi clean [--all]
```

- `--all`: Cleans both Aivi-generated code (`target/aivi-gen`) and Cargo artifacts (`target/debug`, `target/release`).

### Build & Run

#### `build`

Compiles AIVI code. Has two modes:

**Project mode** (no positional path argument   uses `aivi.toml`):

```bash
aivi build [--release] [-- <cargo args...>]
```

- `--release`: Build in release mode (optimizations enabled).
- `<cargo args...>`: Additional arguments passed to `cargo build`.

Reads `aivi.toml`, compiles all `.aivi` sources to Rust, writes the output to `target/aivi-gen/src/`, and invokes `cargo build`.
When `aivi.toml` sets `[build].native_ui_target = "gnome-gtk4-libadwaita"`, project mode forwards `--features runtime-gnome` to Cargo.

**Direct mode** (a path/glob is given as the first argument):

```bash
aivi build <path|dir/...> [--debug-trace] [--target rust|rust-native] [--out <dir|path>]
```

- `--target`: Backend to use.
  - `rust` (default): Generates a Rust library project in `--out`.
  - `rust-native`: Generates a Rust binary project in `--out`.
- `--out`: Output directory (default: `target/aivi-gen`).
- `--debug-trace`: Enables verbose compiler tracing via `AIVI_DEBUG_TRACE=1`.

#### `run`

Runs AIVI code. Has two modes:

**Project mode** (no positional path argument   uses `aivi.toml`):

```bash
aivi run [--release] [-- <cargo args...>]
```

- `--release`: Run in release mode.
- `<cargo args...>`: Additional arguments passed to `cargo run`.
  Project mode also forwards the `runtime-gnome` Cargo feature when `[build].native_ui_target` is set to `gnome-gtk4-libadwaita`.

**Direct mode** (a path/glob is given as the first argument):

```bash
aivi run <path|dir/...> [--debug-trace] [--target native] [--watch|-w]
```

- `--target native` (default): Executes the program via the Cranelift JIT compiler.
- `--debug-trace`: Enables verbose compiler tracing.
- `--watch` / `-w`: Enables **watch mode**. The program is compiled and executed, then automatically re-compiled and re-run whenever a `.aivi` file in the source directory changes. File changes are debounced (300 ms). Press Ctrl-C to exit the watch loop.

### Development Tools

#### `fmt`

Formats AIVI source code.

```bash
aivi fmt [--write] <path|dir/...>
```

- Without `--write`: prints the formatted output to stdout.
- `--write`: formats files **in-place**, overwriting if the content changed.

#### `check`

Checks the code for errors without generating code.

```bash
aivi check [--debug-trace] [--check-stdlib] <path|dir/...>
```

- Loads, parses, resolves, and type-checks all modules under the target.
- `--debug-trace`: Enables verbose compiler tracing.
- `--check-stdlib`: Include diagnostics from embedded stdlib modules (default filters them out).

#### `test`

Runs `@test`-decorated top-level definitions as integration tests.

```bash
aivi test <path|dir/**> [--check-stdlib] [--only <name>...]
```

- Test discovery: only files under the target that contain `@test` are formatted, parsed, and typechecked.
- Execution: each `@test` definition is executed as an `Effect`; failures are reported with the qualified name (`Module.testName`).
- Reports: writes file lists to `target/aivi-test-passed-files.txt` and `target/aivi-test-failed-files.txt`.
- `--check-stdlib`: Include diagnostics from embedded stdlib modules (default filters them out).
- `--only <name>`: Run only the specified test(s). Can be repeated (`--only foo --only bar`). Accepts fully qualified names (`Module.testName`) or unqualified suffixes (`testName`).

#### `parse`

Parses a file and outputs the concrete syntax tree (CST) and any syntax errors.

```bash
aivi parse <path|dir/...>
```

#### `desugar`

Shows the desugared high-level intermediate representation (HIR) of a module.

```bash
aivi desugar [--debug-trace] <path|dir/...>
```

#### `kernel`

Shows the Kernel (Core Calculus) representation of a module.

```bash
aivi kernel [--debug-trace] <path|dir/...>
```

#### `rust-ir`

Shows the Rust Intermediate Representation (Rust IR) of a module.

```bash
aivi rust-ir [--debug-trace] <path|dir/...>
```

### Diagnostics Output

CLI diagnostics (errors and warnings) are rendered with **ANSI colors** when stderr is connected to a terminal (TTY). When piped or redirected, colors are automatically disabled for clean machine-readable output.

### Services

#### `lsp`

Starts the Language Server Protocol (LSP) server. This is typically used by editor extensions, not directly by users.

```bash
aivi lsp
```

#### `mcp`

Starts the Model Context Protocol (MCP) server.

```bash
aivi mcp serve <path|dir/...> [--allow-effects]
```

- `--allow-effects`: Allows the MCP server to execute tools that have side effects.

In v0.1, `aivi mcp serve` exposes:

- bundled language specifications (`specs/`) as MCP resources, and
- language tooling MCP tools:
  - `aivi.parse` (`target`)
  - `aivi.check` (`target`, optional `checkStdlib`)
  - `aivi.fmt` (`target`, returns formatted text)
  - `aivi.fmt.write` (`target`, formats files in place; requires `--allow-effects`)

The `<path|dir/...>` argument is accepted for compatibility and currently ignored by the MCP server;
tool calls pass their own explicit `target` argument.

#### `i18n`

Generates an AIVI module from a `.properties` message catalog (Java-style key=value pairs).

```bash
aivi i18n gen <catalog.properties> --locale <tag> --module <name> --out <file>
```

- `<catalog.properties>`: Path to the Java-style `.properties` file.
- `--locale <tag>`: BCP-47 locale tag (e.g., `en`, `de-AT`).
- `--module <name>`: The AIVI module name for the generated file (e.g., `Messages`).
- `--out <file>`: Output `.aivi` file path.
