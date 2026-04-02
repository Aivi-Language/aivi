# aivi-query

## Purpose

Incremental query database for AIVI tooling — workspace discovery, file parsing and HIR queries,
and LSP backing store. `aivi-query` memoises parse/HIR results per file revision so that
editor-facing features (diagnostics, symbols, completions, formatting) avoid re-parsing unchanged
files. It is the single source of truth for file-to-module mapping and import resolution in the
tooling layer.

## Entry points

```rust
// Central incremental database
RootDatabase::new() -> RootDatabase
RootDatabase::set_file_text(file: SourceFile, text: Arc<str>)
RootDatabase::invalidate(file: SourceFile)

// File / module queries
parsed_file(db: &RootDatabase, file: SourceFile) -> ParsedFileResult
hir_module(db: &RootDatabase, file: SourceFile) -> HirModuleResult
resolve_module_file(db: &RootDatabase, path: &Path) -> Option<SourceFile>
exported_names(db: &RootDatabase, file: SourceFile) -> ExportedNames

// Diagnostics and symbols
all_diagnostics(db: &RootDatabase, file: SourceFile) -> Vec<Diagnostic>
symbol_index(db: &RootDatabase, file: SourceFile) -> Vec<LspSymbol>
format_file(db: &RootDatabase, file: SourceFile) -> Option<String>

// Workspace discovery
discover_workspace_root(path: &Path) -> Option<PathBuf>
discover_workspace_root_from_directory(dir: &Path) -> Option<PathBuf>

// Entrypoint resolution
resolve_v1_entrypoint(db: &RootDatabase, path: &Path) -> Result<ResolvedEntrypoint, EntrypointResolutionError>
```

## Invariants

- `RootDatabase` is `Send + Sync`; it uses `parking_lot` read-write locks for internal caches.
- Queries are memoised by file content hash; `set_file_text` invalidates all cached results for that file and any file that transitively imports it.
- `parsed_file` and `hir_module` never panic; errors are carried inside `ParsedFileResult` / `HirModuleResult`.
- File-to-module mapping is deterministic: the same path always resolves to the same `SourceFile` within a database lifetime.
- `all_diagnostics` aggregates parse and HIR diagnostics; it does not run backend or runtime passes.
- Typed queries (type-checking beyond HIR) are deferred to a future milestone and are not yet memoised.

## Diagnostic codes

This crate emits no `DiagnosticCode` values of its own. It surfaces diagnostics produced by
`aivi-syntax` and `aivi-hir`.

## RFC reference

See [`../../AIVI_RFC.md`](../../AIVI_RFC.md) §26 (incremental query database and tooling).
