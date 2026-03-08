# Spec Doc Markers: Quick Info

This document explains how AIVI documentation can expose hover text and quick-reference material to tools such as the AIVI language server.

The main idea is simple: **write the documentation once, then mark the parts that tools should index**. The wrapped markdown stays the source of truth, so you do not have to maintain a separate copy just for hover cards.

## What a quick-info marker does

A quick-info marker wraps normal markdown and adds a small JSON description that tells tooling what the content refers to.

Use it when a paragraph, list item, table cell, or code-adjacent note is the best short explanation of a symbol or lookup topic such as:

- a module,
- a function,
- a type,
- a class,
- a domain,
- an operator,
- a class member,
- a decorator,
- a syntax form,
- a feature,
- or a topic page.

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
  "kind": "string",
  "name": "string",
  "module": "string (optional)",
  "signature": "string (optional)",
  "extractSignature": true
}
```

`kind` and `name` are required. If the JSON does not parse or either field is missing, current tooling skips that marker instead of indexing partial data.

### Field guide

- `kind` — what sort of thing the content describes. Common values in the current docs include `module`, `function`, `type`, `class`, `domain`, `operator`, `class-member`, `decorator`, `syntax`, `feature`, and `topic`. Tooling may map unrecognized values to `"unknown"` for presentation while still indexing the entry by name.
- `name` — the identifier or lookup label exactly as readers should find it, such as `isEmpty`, `<|`, `Option`, `@test`, or `state machines`.
- `module` — the parent module for non-module items, such as `aivi.text`. Include it whenever the same `name` could appear in more than one module, because module-aware lookup is how tooling disambiguates those entries.
- `signature` — an explicit signature when you want tooling to show one without inferring it.
- `extractSignature` — whether tooling may try to infer a signature from nearby markdown; the default is `true`.

When `extractSignature` is enabled, current tooling tries, in order, to:

1. copy the first fenced `aivi` code block verbatim, or
2. fall back to the first inline code span containing `:` or `->`, such as `` `Text -> Bool` ``.

That heuristic is intentionally simple. If the wrapped prose includes inline code like `` `value : Text` `` or `` `A -> B` `` that is not meant to become the hover signature, set `"extractSignature": false` or provide `"signature"` explicitly.

## Authoring guidelines

When you add quick-info markers, aim for the text a reader would want in a hover popup:

- Keep it short and explanatory.
- Prefer the clearest sentence or short paragraph, not a whole page section.
- Wrap existing prose instead of copying it.
- Add `module` for non-module items when there is any chance of name reuse.
- If a signature is obvious from the surrounding code block, let tooling extract it instead of repeating it manually.
- Nest markers only when the inner symbol genuinely deserves its own indexed explanation.

A good test is: *would this still read naturally if the marker comments disappeared?* If yes, you are using the feature as intended.

## Examples

The examples below escape the comment delimiters so this page can demonstrate the syntax without indexing its own samples. When authoring a real marker, replace `&lt;`/`&gt;` with literal `<`/`>` characters.

### Module documentation

Use this when a paragraph introduces a module at the top of its spec page.

```md
&lt;!-- quick-info: {"kind":"module","name":"aivi.text"} --&gt;
The `aivi.text` module provides core string and character utilities for `Text` and `Char`.
&lt;!-- /quick-info --&gt;
```

### Function documentation inside a table

Use this when a function is documented in a compact API table.

```md
| **isEmpty** text<br><code>Text -> Bool</code> | &lt;!-- quick-info: {"kind":"function","name":"isEmpty","module":"aivi.text"} --&gt;Returns `true` when `text` has zero length.&lt;!-- /quick-info --&gt; |
```

### Explicit signature when extraction would be unclear

```md
&lt;!-- quick-info: {"kind":"operator","name":"|>","signature":"A -> (A -> B) -> B"} --&gt;
Applies the value on the left to the function on the right.
&lt;!-- /quick-info --&gt;
```

### Nested markers

Use nesting sparingly when one explanation contains a smaller explanation that should also be indexed on its own.

```md
&lt;!-- quick-info: {"kind":"class","name":"Functor","module":"aivi.logic"} --&gt;
Functors support mapping over a wrapped value.
&lt;!-- quick-info: {"kind":"class-member","name":"map","module":"aivi.logic"} --&gt;
`map` applies a function inside the surrounding context.
&lt;!-- /quick-info --&gt;
&lt;!-- /quick-info --&gt;
```

The outer entry keeps the inner prose, but tooling strips the marker comments themselves from the indexed content.

## Verifying a marker

In this repository you can verify marker changes with existing tooling:

```bash
cargo run -p doc_index_gen -- --specs-dir specs --output /tmp/doc-index.json
cargo test -p aivi-lsp doc_index
```

The generator confirms that the metadata parses and emits an index entry. The targeted `aivi-lsp` tests exercise nested markers, table-cell markers, signature extraction, and ambiguous-name lookup behavior.
