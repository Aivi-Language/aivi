# aivi-lsp

`aivi-lsp` serves AIVI analysis over Language Server Protocol stdio. It uses `aivi-query` for
revision-aware compiler results and `tower-lsp` for transport.

## Capabilities

- versioned incremental document synchronization
- diagnostics with stable codes and related information
- formatting, completion, hover, and signature help
- definition, implementation, references, prepare rename, and rename
- document and workspace symbols
- document highlights and folding ranges
- inlay hints, code actions, and `@test` code lenses
- full, range, and delta semantic tokens

Handlers live in capability-named modules. `server::Backend` is the protocol boundary and
`aivi_lsp::run()` starts the stdio server.

## State and concurrency

- `documents` applies LSP edit batches transactionally to versioned `ropey::Rope` values. UTF-16
  offsets, surrogate boundaries, range lengths, and monotonically increasing versions validate
  before the query input changes.
- `analysis_pool` is a bounded dedicated worker pool. Blocking compiler work does not run on a
  Tokio executor thread.
- Superseded diagnostics are cancelled. Publication is serialized with a final revision check, so
  a rapid edit cannot publish an older result.
- `workspace_index` publishes one immutable navigation index per query-database workspace
  revision.
- `semantic_tokens` keeps bounded revision history for delta responses.
- Protocol traffic uses stdout and tracing uses stderr.

The database is owned directly by `ServerState`; access that may overlap analysis is coordinated
explicitly. DashMap protects document and request registries, but callers clone immutable snapshots
before analysis rather than holding map guards across awaits.

## Diagnostics

Frontend diagnostics originate in `aivi-syntax` and `aivi-hir` through `aivi-query`. LSP
conversion preserves severity, `domain::name` code, UTF-16 primary range, and cross-file
secondary labels. Type-annotation and unused-symbol hints are editor-owned diagnostics. The final
wire list is deterministically sorted.

See [Diagnostics and Editor Tooling](../../manual/reference/editor-tooling.md) and
[AIVI_RFC.md §27](../../AIVI_RFC.md#27-language-server-lsp).
