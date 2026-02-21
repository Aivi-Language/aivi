# Spec Doc Markers: Quick Info

This document defines **documentation markers** that allow tooling (notably the AIVI LSP) to
extract hover/quick-info content directly from existing spec markdown text.

The markers **wrap** existing markdown. The wrapped markdown remains the single source of truth:
no documentation should be duplicated just for hover.

## Marker Syntax

- Opening marker:
  - `<!-- quick-info: {JSON} -->`
- Closing marker:
  - `<!-- /quick-info -->`
- Content:
  - Any existing markdown (paragraphs, headings, table cells, code blocks, etc.)

Markers may be nested. Tooling should ignore the marker comments themselves when rendering
markdown, but keep the wrapped content.

## JSON Metadata

The JSON object in the opening marker provides lookup keys for tooling:

```json
{
  "kind": "module | function | type | class | domain | operator | class-member",
  "name": "string",
  "module": "string (optional)",
  "signature": "string (optional)",
  "extractSignature": true
}
```

Notes:

- `kind` is used for display and indexing. Tooling may treat unknown kinds as `"unknown"`.
- `name` is the identifier as it appears in AIVI code (e.g. `isEmpty`, `<|`, `Option`).
- `module` is the parent module for non-module items (e.g. `aivi.text`).
- `signature` can be provided explicitly when extraction is ambiguous.
- `extractSignature` defaults to `true`; when enabled, tooling may infer a signature from:
  - A fenced AIVI code block (\\`\\`\\`aivi ... \\`\\`\\`)
  - Inline code spans that look like a type (e.g. `` `Text -> Bool` ``)

## Examples

Module documentation:

```md
<!-- quick-info: {"kind":"module","name":"aivi.text"} -->
The `aivi.text` module provides core string and character utilities for `Text` and `Char`.
<!-- /quick-info -->
```

Table cell documentation:

```md
| **isEmpty** text<br><pre><code>`Text -> Bool`</code></pre> | <!-- quick-info: {"kind":"function","name":"isEmpty","module":"aivi.text"} -->Returns `true` when `text` has zero length.<!-- /quick-info --> |
```
