# Aivi CLI

The Aivi Command Line Interface (CLI) is the primary tool for managing Aivi projects. It handles project creation, dependency management, compilation, and execution.

## Installation

The CLI is distributed as a single binary named `aivi`. It can be installed directly from source or via pre-built binaries (roadmap).

## Commands

### Project Management

#### `init` / `new`

Creates a new Aivi project in a directory.

```bash
aivi init <name> [--bin|--lib] [--edition 2024] [--language-version 0.1] [--force]
```

- `<name>`: The name of the project.
- `--bin`: Create a binary (application) project (default).
- `--lib`: Create a library project.
- `--edition`: Set the Rust edition (default: 2024).
- `--language-version`: Set the Aivi language version (default: 0.1).
- `--force`: Force creation even if the directory is not empty.

#### `install`

Installs a dependency into the current project.

```bash
aivi install <spec> [--require-aivi] [--no-fetch]
```

- `<spec>`: The dependency specification. clear
  - `name`: Installs the latest version from the registry.
  - `name@version`: Installs a specific version.
  - `git+https://github.com/user/repo`: Installs from a Git repository.
  - `path:../local-crate`: Installs from a local path.
- `--require-aivi`: Ensures the installed dependency is a valid Aivi package.
- `--no-fetch`: Updates `Cargo.toml` but skips running `cargo fetch`.

#### `search`

Searches for Aivi packages in the registry.

```bash
aivi search <query>
```

#### `clean`

Cleans build artifacts.

```bash
aivi clean [--all]
```

- `--all`: Cleans both Aivi-generated code (`target/aivi-gen`) and Cargo artifacts (`target/debug`, `target/release`).

### Build & Run

#### `build`

Compiles the current project.

```bash
aivi build [--release] [-- <cargo args...>]
```

- `--release`: Build in release mode (optimizations enabled).
- `<cargo args...>`: Additional arguments passed to `cargo build`.

#### `run`

Runs the current project (if it is a binary).

```bash
aivi run [--release] [-- <cargo args...>]
```

- `--release`: Run in release mode.
- `<cargo args...>`: Additional arguments passed to `cargo run`.

### Development Tools

#### `fmt`

Formats Aivi source code.

```bash
aivi fmt <path>
```

#### `check`

Checks the code for errors without generating code.

```bash
aivi check <path|dir/...>
```

Calculates diagnostics and performs type checking.

#### `parse`

Parses a file and outputs the concrete syntax tree (CST) and any syntax errors.

```bash
aivi parse <path|dir/...>
```

#### `desugar`

Shows the desugared high-level intermediate representation (HIR) of a module.

```bash
aivi desugar <path|dir/...>
```

#### `kernel`

Shows the Kernel (Core Calculus) representation of a module.

```bash
aivi kernel <path|dir/...>
```

#### `rust-ir`

Shows the Rust Intermediate Representation (Rust IR) of a module.

```bash
aivi rust-ir <path|dir/...>
```

### Services

#### `lsp`

Starts the Language Server Protocol (LSP) server. This is typically used by editor extensions, not directly by users.

```bash
aivi lsp
```

#### `mcp`

Starts the Model Context Protocol (MCP) server for a specific file or directory. This allows LLMs to context-aware interaction with the codebase.

```bash
aivi mcp serve <path|dir/...> [--allow-effects]
```

- `--allow-effects`: Allows the MCP server to execute tools that have side effects.
