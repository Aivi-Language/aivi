# aivi-cli

`aivi-cli` builds the `aivi` binary and owns end-to-end command orchestration across syntax,
HIR, typed core, lambda IR, backend IR, Cranelift, the runtime, GTK, LSP, and MCP surfaces.

## Commands

| Command | Current behavior |
| --- | --- |
| `check` | Parse, resolve, and type-check a file, directory, or manifest entries |
| `compile` | Emit a native object file; do not link a runnable app |
| `build` | Package one runnable executable with a source-free embedded app bundle |
| `run` | Compile and launch a GTK app or load a compatible frozen run image |
| `execute` | Run an exported headless `main : Task ...` |
| `test` | Run every `@test` value or one exact test from a requested file |
| `lex` | Print the lossless token stream |
| `fmt` | Print canonical formatting, read stdin, or check files |
| `openapi-gen` | Generate AIVI declarations from OpenAPI 3 JSON/YAML |
| `init` | Scaffold a project |
| `lsp` | Start the stdio language server |
| `mcp` | Start the stdio live-introspection server |
| `manual-snippets` | Check and optionally rewrite manual AIVI blocks |

`aivi help <command>` is the executable command contract. The user-facing artifact distinctions
are documented in [Toolchain and Artifacts](../../manual/reference/toolchain-and-artifacts.md).

## Ownership boundaries

- `main_parts/dispatch.rs` parses commands and selects entrypoints.
- `main_parts/check_execute.rs` owns check and headless execution orchestration.
- `main_parts/run_*.rs` own live-session preparation, versioned run artifacts, hydration, and
  lifecycle.
- `main_parts/build_tools.rs` owns runnable packaging and help output.
- `manual_snippets.rs` owns repository documentation verification.
- `mcp.rs` owns live application introspection.
- `run_session.rs` owns the shared scheduler/GTK session boundary.

## Invariants

- GTK creation, mutation, and event dispatch stay on the GLib main thread.
- Worker results cross into scheduler/GTK ownership as messages.
- `compile` stops at object emission; only `build` owns the single-file runnable path.
- Frozen images and backend payloads use bounded versioned envelopes and fail closed with rebuild
  guidance.
- The CLI and LSP render the same native frontend diagnostics.
- Exit status is zero on success and non-zero on validation, runtime, or command errors.
- The crate forbids unsafe code.
