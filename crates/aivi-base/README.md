# aivi-base

## Purpose

Foundational source-tracking and diagnostic infrastructure shared by every AIVI compiler layer.
This crate sits at the bottom of the dependency graph — no other AIVI crate it depends on.
It provides byte-indexed source locations, file identity, span types, a typed diagnostic model,
and a simple arena allocator used across the pipeline.

## Entry points

```rust
// Source tracking
SourceDatabase::new() -> SourceDatabase
SourceDatabase::add_file(name, text) -> FileId
SourceDatabase::file(id: FileId) -> &SourceFile
SourceFile::source() -> &str
SourceSpan::new(file: FileId, start: ByteIndex, end: ByteIndex) -> SourceSpan

// Diagnostics
Diagnostic::new(code: DiagnosticCode, severity: Severity, message: String) -> Diagnostic
Diagnostic::with_label(span: SourceSpan, style: LabelStyle, message: String) -> Diagnostic
ErrorCollection::new() -> ErrorCollection
ErrorCollection::push(diag: Diagnostic)

// Arena
Arena::<T>::new() -> Arena<T>
Arena::alloc(value: T) -> ArenaId<T>
```

## Invariants

- `FileId` values are stable for the lifetime of a `SourceDatabase`; they are never reused.
- `ByteIndex` offsets are in UTF-8 bytes; callers are responsible for alignment to char boundaries.
- `Span` carries a `FileId` — cross-file spans are not supported.
- `DiagnosticCode` is a `(namespace, slug)` pair; both parts are `&'static str`.
- `Arena` allocations are append-only; deallocation of individual items is not supported.
- `ErrorCollection` accumulates without de-duplicating; callers decide when to flush.

## Diagnostic codes

This crate emits no diagnostics — it only provides the infrastructure for other crates to emit them.

## RFC reference

See [`../../AIVI_RFC.md`](../../AIVI_RFC.md) §3.6 (source model) and §4.1 (pipeline diagnostics).
