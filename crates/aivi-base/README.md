# aivi-base

`aivi-base` owns source identity, byte spans, UTF-16 position conversion, diagnostics, rendering,
and small typed arenas shared by the compiler and tools. It has no dependency on another AIVI
crate.

## Main API

```rust
let mut sources = aivi_base::SourceDatabase::new();
let file = sources.add_file("main.aivi", "value answer = 42\n");
let source = sources.file(file).expect("new file remains addressable");

let diagnostic = aivi_base::Diagnostic::error("example")
    .with_code(aivi_base::DiagnosticCode::new("example", "failure"))
    .with_primary_label(source.full_span(), "the primary location");
let rendered = aivi_base::DiagnosticRenderer::plain().render(&diagnostic, &sources);
```

The public surface also includes `ByteIndex`, `Span`, `SourceSpan`, `LspPosition`,
`LspRange`, `Arena<T>`, and `ArenaId<T>`.

## Invariants

- `FileId` is stable within one `SourceDatabase` and is carried by every `SourceSpan`.
- `Span` offsets are UTF-8 byte offsets; `SourceFile` owns conversion to and from UTF-16 LSP
  positions.
- `DiagnosticCode` is a typed `domain::name` pair of static strings.
- Diagnostics preserve primary and secondary labels, notes, and help separately.
- Rendering is deterministic; color is an explicit `ColorMode` choice.
- Arenas are append-only and return typed IDs. Capacity failure is reported as `ArenaOverflow`.
- The crate forbids unsafe code.

This crate defines diagnostic infrastructure but does not define compiler-layer diagnostic codes.
See the [diagnostic and editor reference](../../manual/reference/editor-tooling.md).
