# aivi-lsp

## Purpose

Language Server Protocol server for the AIVI language. `aivi-lsp` exposes editor features —
hover documentation, completions, go-to-definition, diagnostics, formatting, semantic tokens,
symbol search, code lens, and unused-symbol hints — over the standard LSP protocol (stdio
transport). It is backed by `aivi-query` for incremental analysis and `tower-lsp` for protocol
handling.

## Entry points

```rust
// Start the LSP server on stdio (called from aivi-cli's `lsp` subcommand)
aivi_lsp::run() -> anyhow::Result<()>

// Core server type (tower-lsp LanguageServer impl)
server::Backend::new(client: tower_lsp::Client) -> Backend
```

Feature handlers live in dedicated modules:

| Module | LSP capability |
|---|---|
| `hover` | `textDocument/hover` — type and doc info |
| `completion` | `textDocument/completion` — name completions |
| `definition` | `textDocument/definition` — go-to-definition |
| `diagnostics` | `textDocument/publishDiagnostics` — parse + HIR errors |
| `formatting` | `textDocument/formatting` — canonical formatter |
| `semantic_tokens` | `textDocument/semanticTokens` — syntax highlighting |
| `symbols` | `textDocument/documentSymbol` and `workspace/symbol` |
| `code_lens` | `textDocument/codeLens` — inline action hints |
| `unused` | Unused import / binding hints |
| `implementation` | `textDocument/implementation` |

## Invariants

- LSP protocol traffic uses stdout; tracing is written to stderr to avoid contamination.
- All `tower-lsp` handler methods are `async`; blocking operations must not run on the Tokio executor thread.
- Document state is managed by `documents` using `dashmap` for concurrent access; snapshot reads are lock-free.
- `Backend` holds an `Arc<RootDatabase>` shared across all handlers; database writes are serialised by the database's own locks.
- The server does not cache LSP client state across reconnections; each `initialize` request starts fresh.
- Feature handlers return `None` / empty results rather than errors for unsupported positions; the protocol is never broken by a missing analysis result.

## Diagnostic codes

This crate emits no `DiagnosticCode` values of its own. It translates diagnostics from
`aivi-syntax` and `aivi-hir` (via `aivi-query`) into LSP `Diagnostic` objects.

## RFC reference

See [`../../AIVI_RFC.md`](../../AIVI_RFC.md) §27 (Language Server Protocol).
