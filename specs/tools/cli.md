# AIVI CLI

The `aivi` command-line tool is the main entry point for creating projects, running programs, inspecting compiler output, formatting code, and starting editor-facing services.

If you are new to AIVI, most day-to-day work fits into `run`, `check`, `fmt`, and `test`; use `init` when you are starting a new project. Later sections cover packaging, editor services, and compiler-inspection commands that are more useful once you already have a project or are debugging tooling.

A good mental model is:

- `aivi` handles AIVI-specific tasks such as parsing, formatting, checking, and code generation.
- Cargo still handles package resolution, Rust compilation, and publishing under the hood.

## Start here

If you are brand new, you can ignore most of the command list on your first day.
The usual starter loop is:

1. `aivi init hello-world --bin`
2. `cd hello-world`
3. `aivi run`
4. `aivi check`
5. `aivi fmt --write src`

Add `aivi test` once the project actually contains `@test` definitions.

## Installation

The CLI is distributed as a binary named `aivi`. Once it is installed, `aivi --version` prints both the CLI version and the supported language version.

## A quick first workflow

```bash
aivi init hello-world --bin  # Create an application project.
cd hello-world
aivi run                     # Run the project entry through the development JIT pipeline.
```

For a library project, use `--lib` instead of `--bin`.

When the current directory contains `aivi.toml`, `aivi build`, `aivi run`, `aivi check`, and `aivi test` all prefer **project mode**. Passing a file path from inside that project does not switch those commands into direct mode; run them from outside the project root if you want true direct-mode behavior.

## Command map by task

If you are skimming, start from the workflow you want:

| I want to... | Command to reach for |
| --- | --- |
| create a new project | `aivi init` / `aivi new` |
| add a dependency | `aivi install` |
| run my current project | `aivi run` |
| build artefacts without running | `aivi build` |
| type-check quickly | `aivi check` |
| format code | `aivi fmt` |
| run AIVI tests | `aivi test` |
| explore and experiment interactively | `aivi repl` |
| inspect compiler stages | `aivi parse`, `aivi desugar`, `aivi kernel`, `aivi rust-ir` |
| start editor tooling | `aivi lsp` |
| expose tooling over MCP | `aivi mcp serve` |
| package or publish a library | `aivi package`, `aivi publish` |

Two patterns show up throughout this page:

- **project mode** reads `aivi.toml` in the current project directory
- **direct mode** works on an explicit file or directory path you pass on the command line

### Which mode should I use?

Use **project mode** when you have an `aivi.toml` and want the command to behave like a normal project tool.
Use **direct mode** when you want to point at one file or directory without scaffolding a full project first.

| Situation | Reach for |
| --- | --- |
| “I am inside a normal AIVI project.” | `aivi run`, `aivi build`, `aivi check`, `aivi test` |
| “I want to inspect or run one file/folder from a scratch location.” | `aivi run path/to/file.aivi`, `aivi check path/to/file.aivi`, `aivi build path/to/dir/...` |

## Common workflows

| Workflow | Commands | When to use it |
| --- | --- | --- |
| first-time setup | `aivi init` → `aivi run` → `aivi check` | starting a new project |
| daily editing | `aivi run --watch` → `aivi check` → `aivi test` | active development |
| before commit | `aivi fmt --write src` → `aivi check` → `aivi test` | local validation |
| release a library | `aivi package --dry-run` → `aivi publish` | packaging and publishing |

In the rows that include `aivi test`, assume the project already contains `@test` definitions.

## Commands

### `version`

Prints CLI and language version information.

```bash
aivi version
aivi --version
aivi -V
```

Running plain `aivi` also prints the version header before the command help.

### Project management

#### `init` / `new`

Creates a new AIVI project directory.

```bash
aivi init <name> [--bin|--lib] [--edition 2024] [--language-version 0.1] [--force]
```

- `<name>`: project directory and package name.
- `--bin`: create an application project (default).
- `--lib`: create a library project.
- `--edition`: set the Rust edition used for generated Cargo files (default: `2024`).
- `--language-version`: set the AIVI language version written into the scaffold (default: `0.1`).
- `--force`: allow generation into a non-empty directory.

`new` is an alias for `init`.

#### `install`

Adds a dependency to the current project.

```bash
aivi install <spec> [--no-fetch]
```

Supported dependency specs:

- `name`: latest registry version
- `name@version`: a specific registry version
- `git+https://github.com/user/repo`: a Git dependency
- `path:../local-crate`: a local path dependency

Install validation is strict by default:

- the dependency must declare `[package.metadata.aivi]`
- it must include `language_version`
- it must be `kind = "lib"`
- if your project declares `language_version` in `aivi.toml`, the dependency must match it exactly

`--no-fetch` updates `Cargo.toml` but skips `cargo fetch`.

#### `search`

Searches the package registry for AIVI packages.

```bash
aivi search <query>
```

Internally this wraps `cargo search keyword:aivi <query> --limit 20`, so results come from crates.io.

#### `package`

Runs `cargo package` for the current project after AIVI-specific preflight checks.

```bash
aivi package [--allow-dirty] [--no-verify] [-- <cargo args...>]
```

- requires `aivi.toml`
- requires `[package.metadata.aivi]` in `Cargo.toml`
- forwards extra Cargo arguments after `--`

#### `publish`

Runs `cargo publish` with the same preflight checks used by `package`.

```bash
aivi publish [--dry-run] [--allow-dirty] [--no-verify] [-- <cargo args...>]
```

- `--dry-run`: verify publishing without uploading
- `--allow-dirty`: forward to Cargo
- `--no-verify`: forward to Cargo

#### `clean`

Removes generated build artefacts.

```bash
aivi clean [--all]
```

- without `--all`: cleans AIVI-generated code in `target/aivi-gen`
- with `--all`: also cleans Cargo artefacts such as `target/debug` and `target/release`

See also [Package Manager & Packaging](package_manager.md) for dependency syntax, manifest structure, and publishing workflow.

### Build and run

#### `build`

`build` has two modes.

**Project mode** uses the project manifest (`aivi.toml`):

```bash
aivi build [--release]
```

- compiles the source tree derived from the project entry in `aivi.toml`
- writes an object file to `target/aivi-gen/aivi_program.o`
- writes generated harness/support code under `target/aivi-gen/src/`
- invokes `cargo build` to link the final binary
- `--release` switches Cargo to the release profile

**Direct mode** compiles an explicit file or directory target:

```bash
aivi build <path|dir/...> [--debug-trace] [--out <dir|path>]
```

- emits a native object file and prints its path
- `--out`: output directory for generated artefacts (default: `target/aivi-gen`)
- `--debug-trace`: enable verbose compiler tracing via `AIVI_DEBUG_TRACE=1`

Direct mode is the easiest way to experiment with one file or folder before you have a full project layout.

#### `run`

`run` also has project mode and direct mode.

**Project mode**:

```bash
aivi run [--watch|-w]
```

- runs the current project described by `aivi.toml`
- uses the Cranelift JIT pipeline for a fast edit/run loop
- `--watch` re-runs when source files change
- `--release` and extra Cargo arguments are not supported in project mode; use `aivi build --release` when you need an optimized binary

**Direct mode**:

```bash
aivi run <path|dir/...> [--debug-trace] [--target native] [--watch|-w]
```

- uses the Cranelift JIT runtime, meaning it compiles to native code in memory and runs it immediately instead of producing a standalone binary first
- `--target native` is the supported direct-run target
- `--debug-trace` enables verbose compiler tracing
- `--watch` recompiles and re-runs when `.aivi` files change

Direct `run` is great for quick experiments and examples. For distributable applications and repeatable builds, prefer `aivi build`.

### Development tools

#### `fmt`

Formats AIVI source code.

```bash
aivi fmt [--write] <path|dir/...>
```

- without `--write`: prints formatted output to stdout
- with `--write`: updates files in place

#### `check`

Parses, resolves, and type-checks code without generating runnable output.

```bash
aivi check [--debug-trace] [--check-stdlib] [<path|dir/...>]
```

- inside a project, omitting the target checks the source tree derived from `aivi.toml`
- outside a project, pass an explicit file or directory target
- `--debug-trace`: enable verbose compiler tracing
- `--check-stdlib`: include diagnostics from embedded stdlib modules

#### `test`

Runs top-level definitions decorated with `@test`.

```bash
aivi test [<path|dir/...>] [--check-stdlib] [--only <name>...] [--update-snapshots]
```

- inside a project, omitting the target uses the source tree derived from `aivi.toml`
- outside a project, pass an explicit file or directory target
- discovers `@test` definitions under the target
- formats discovered test files in place before execution
- executes each test as an `Effect`
- fails if no `@test` definitions are found under the target
- reports failures using qualified names such as `Module.testName`
- writes file lists to `target/aivi-test-passed-files.txt` and `target/aivi-test-failed-files.txt`
- `--only` may be repeated and accepts either qualified or unqualified test names
- `--update-snapshots` records or refreshes snapshot files used by snapshot assertions

See also [Testing Module](../stdlib/core/testing.md) for assertion helpers and snapshot semantics, and [`@test`](../syntax/decorators/test.md) for decorator syntax and `mock ... in` rules.

#### `parse`

Prints the concrete syntax tree and any syntax errors.

```bash
aivi parse <path|dir/...>
```

#### `desugar`

Shows the desugared high-level intermediate representation (HIR).

```bash
aivi desugar [--debug-trace] <path|dir/...>
```

#### `kernel`

Shows the kernel representation of a module.

```bash
aivi kernel [--debug-trace] <path|dir/...>
```

#### `rust-ir`

Shows the Rust intermediate representation used before backend lowering.

```bash
aivi rust-ir [--debug-trace] <path|dir/...>
```

These inspection commands (`parse`, `desugar`, `kernel`, `rust-ir`) are mainly for debugging compiler behavior. Most application authors can ignore them until they need to understand how a program is being lowered.

#### `repl`

Starts an interactive Read-Eval-Print Loop for the AIVI language.

```bash
aivi repl [--color] [--no-color] [--plain]
```

- `--color`: force ANSI color output even when stdout is not a terminal.
- `--no-color`: disable ANSI color output.
- `--plain`: plain read-eval-print mode — no TUI, pipe-friendly. Automatically selected when stdin is not a terminal.

With no flags the REPL opens a full-screen TUI. The prelude is pre-loaded and all stdlib symbols are immediately in scope. Successful expression inputs are compiled and evaluated, so the transcript shows `value :: Type` instead of only a type summary.

**Keyboard shortcuts (TUI mode)**

| Key | Action |
| --- | --- |
| Enter | Submit input |
| Shift+Enter | Insert newline (multi-line input) |
| ↑ / ↓ | Navigate history or slash-command suggestions |
| Ctrl+L | Clear transcript |
| Ctrl+C | Cancel current input |
| Ctrl+D | Exit (on empty input) |
| Tab | Accept the current slash-command suggestion, or toggle the symbol pane when no suggestion is shown |
| Esc | Close symbol pane |

**Slash commands**

| Command | Description |
| --- | --- |
| `/help` | Print command reference |
| `/use <module.path>` | Add an import to the session (e.g. `/use aivi.text`) |
| `/types [filter]` | List types in scope (stdlib + session); optional substring filter |
| `/values [filter]` | List session-defined values with their inferred types |
| `/functions [filter]` | List functions in scope (stdlib + session) |
| `/modules` | Show loaded modules in the session |
| `/clear` | Clear the transcript while keeping all session state |
| `/reset` | Clear the transcript and reset all session state |
| `/history [n]` | Show the last `n` inputs (default: 20) |
| `/load <path>` | Load a `.aivi` file into the session |
| `/openapi file <path> [as <name>]` | Inject an OpenAPI spec file as a `@static` module |
| `/openapi url <url> [as <name>]` | Inject an OpenAPI spec from a URL as a `@static` module |

The `/openapi` commands inject typed API bindings into the session under the given module name (or a name derived from the spec's `info.title`). They use the same `@static` OpenAPI mechanism as the language's compile-time external source support.

When the input starts with `/`, the TUI shows matching slash commands inline and filters them as you type. Use `↑` / `↓` to move through the suggestions and `Tab` to accept the highlighted command.

### Services

#### `lsp`

Starts the Language Server Protocol server used by editors. If you have never launched an LSP by hand, think of this as the background service your editor talks to for hovers, type hints, completions, and diagnostics.

```bash
aivi lsp
```

Typical flow:

1. your editor launches `aivi lsp`
2. the editor sends the current file contents, including unsaved edits
3. the server answers with diagnostics, completions, hover text, and navigation locations

See also [LSP Server](lsp_server.md) for the feature-level behavior and incremental-checking rules.

#### `mcp serve`

Starts the Model Context Protocol server for language tooling and optional GTK inspection / driver tools.

```bash
aivi mcp serve [<path|dir/...>] [--allow-effects] [--ui]
```

- `--allow-effects`: enables effectful tools such as formatting files in place, launching UI sessions, or driving GTK widgets
- `--ui`: also exposes GTK inspector and driver tools under underscore-safe names such as `aivi_gtk_launch` (legacy dotted spellings such as `aivi.gtk.launch` are still accepted on input)

`aivi mcp serve` exposes bundled specs as resources and tooling operations such as:

- `aivi_parse`
- `aivi_check`
- `aivi_fmt`
- `aivi_fmt_write` (requires `--allow-effects`)

With `--ui`, it also exposes non-effectful GTK tools such as discovery, widget inspection, and tree inspection, plus effectful tools such as `aivi_gtk_launch`, `aivi_gtk_click`, `aivi_gtk_type`, `aivi_gtk_select`, and `aivi_gtk_keyPress`.

The optional `<path|dir/...>` argument is accepted for compatibility and is currently ignored by the server; if omitted it defaults to `./...`, and individual tool calls still pass their own explicit `target` argument.

See also [MCP Server](mcp.md) for the GTK session lifecycle, inspection payloads, and interaction semantics.

#### `i18n gen`

Generates an AIVI module from a Java-style `.properties` catalog.

```bash
aivi i18n gen <catalog.properties> --locale <tag> --module <name> --out <file>
```

- `--locale`: BCP-47 locale tag such as `en` or `de-AT`
- `--module`: generated AIVI module name
- `--out`: output file path

## Diagnostics output

CLI diagnostics use ANSI colors when stderr is attached to a terminal. When output is piped or redirected, colors are disabled automatically so the text stays readable in logs and scripts.
