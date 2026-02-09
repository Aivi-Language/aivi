# LSP Plan (Rust `tower-lsp` + shared compiler engine)

Goal: make AIVI a “daily driver” language in editors while the compiler is still evolving.

## Architecture

- A single analysis engine shared by CLI + LSP:
  - parse (lossless) → resolve → typecheck
  - all of it is incremental via `salsa` (or equivalent)
- LSP maintains:
  - open document text
  - file-to-module mapping
  - an analysis “snapshot” per version

Recommended layering:
- `aivi_parser` produces CST + syntax diagnostics.
- `aivi_resolve` produces module graph + name diagnostics.
- `aivi_types` produces type diagnostics + per-node type info.
- `aivi_fmt` formats CST.
- `aivi_lsp` is just protocol glue + conversions to/from `lsp-types`.

## Features by milestone

### Milestone L1: parse diagnostics

- `textDocument/didOpen`, `didChange`:
  - parse file
  - publish syntax diagnostics
- Basic document symbols:
  - show module exports + top-level bindings

### Milestone L2: go-to-definition + completion (names)

- `textDocument/definition`:
  - find symbol under cursor
  - navigate within file and across imports
- `textDocument/completion`:
  - keywords and visible symbols
  - exported module members

### Milestone L3: types in hover + signature help

- `textDocument/hover`:
  - show inferred type and docstring (if available)
- `textDocument/signatureHelp`:
  - for function calls (including curried functions)

### Milestone L4: references + rename

- `textDocument/references`
- `textDocument/rename`

### Milestone L5: formatting + code actions

- `textDocument/formatting`:
  - CST-driven formatter
- `textDocument/codeAction`:
  - quick fixes: “import missing name”, “add type annotation”, “add match cases”

### Milestone L6: semantic tokens + inlay hints (quality)

- Semantic tokens driven by:
  - resolved IDs (value/type/constructor/module)
  - typed info (effects, domains)
- Inlay hints:
  - inferred types for `let` bindings (optional)
  - effect requirements (optional)

## Key implementation details

### Spans and mapping

Everything depends on robust span mapping:
- `SourceMap` stores file text and line offsets.
- Diagnostics include `(FileId, Span, message, severity, code)`.
- LSP conversion maps spans to `Range`.

### Incrementality model

Use `salsa` queries like:
- `parse(FileId) -> ParsedFile`
- `module_graph(WorkspaceId) -> ModuleGraph`
- `resolve(FileId) -> ResolvedFile`
- `typecheck(FileId) -> TypedFile`

Avoid global “rebuild everything” on each keystroke.

### “Never crash on partial code”

Mandatory:
- parser recovery (synchronize on safe tokens)
- name resolver tolerates missing nodes
- typechecker uses “error types” and keeps going

## VS Code strategy

Short-term:
- Keep current TextMate grammar for highlighting.
- Use `vscode-languageclient` to run `aivi lsp`.

Long-term:
- Add tree-sitter grammar (optional) for better highlighting and folding.

