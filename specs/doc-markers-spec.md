# Spec Doc Markers: Quick Info

This document explains how AIVI documentation can expose hover text and quick-reference material to tools such as the AIVI language server.

The main idea is simple: **write the documentation once, then mark the parts that tools should index**. The wrapped markdown stays the source of truth, so you do not have to maintain a separate copy just for hover cards.

## What a quick-info marker does

A quick-info marker wraps normal markdown and adds a small JSON description that tells tooling what the content refers to.

Use it when a paragraph, list item, table cell, or code-adjacent note is the best short explanation of a symbol such as:

- a module,
- a function,
- a type,
- a class,
- a domain,
- an operator,
- or a class member.

## Marker syntax

- Opening marker: `<!-- quick-info: {JSON} -->`
- Closing marker: `<!-- /quick-info -->`
- Wrapped content: any valid markdown you already want readers to see

Tooling should ignore the HTML comments when rendering markdown, but it should keep and index the wrapped content.

Markers may be nested when that genuinely helps, although most pages only need a simple one-layer marker around the best explanatory text.

## JSON metadata

The JSON object in the opening marker gives tooling the lookup information it needs.

```json
{
  "kind": "module | function | type | class | domain | operator | class-member",
  "name": "string",
  "module": "string (optional)",
  "signature": "string (optional)",
  "extractSignature": true
}
```

### Field guide

- `kind` — what sort of thing the content describes. Unknown values may be treated as `"unknown"`.
- `name` — the identifier exactly as it appears in AIVI code, such as `isEmpty`, `<|`, or `Option`.
- `module` — the parent module for non-module items, such as `aivi.text`.
- `signature` — an explicit signature when you want tooling to show one without inferring it.
- `extractSignature` — whether tooling may try to infer a signature from nearby markdown; the default is `true`.

When `extractSignature` is enabled, tooling may infer a signature from:

- a fenced AIVI code block, or
- an inline code span that looks like a type, such as `` `Text -> Bool` ``.

## Authoring guidelines

When you add quick-info markers, aim for the text a reader would want in a hover popup:

- Keep it short and explanatory.
- Prefer the clearest sentence or short paragraph, not a whole page section.
- Wrap existing prose instead of copying it.
- If a signature is obvious from the surrounding code block, let tooling extract it instead of repeating it manually.

A good test is: *would this still read naturally if the marker comments disappeared?* If yes, you are using the feature as intended.

## Examples

### Module documentation

Use this when a paragraph introduces a module at the top of its spec page.

```md
<!-- quick-info: {"kind":"module","name":"aivi.text"} -->
The `aivi.text` module provides core string and character utilities for `Text` and `Char`.
<!-- /quick-info -->
```

### Function documentation inside a table

Use this when a function is documented in a compact API table.

```md
| **isEmpty** text<br><code>Text -> Bool</code> | <!-- quick-info: {"kind":"function","name":"isEmpty","module":"aivi.text"} -->Returns `true` when `text` has zero length.<!-- /quick-info --> |
```

### Explicit signature when extraction would be unclear

```md
<!-- quick-info: {"kind":"operator","name":"<|","signature":"A -> Patch A -> A"} -->
Applies a structural patch and returns a new value instead of mutating the old one.
<!-- /quick-info -->
```
