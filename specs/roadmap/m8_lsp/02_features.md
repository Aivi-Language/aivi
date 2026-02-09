# M8 LSP — Features

## L1: parse diagnostics

- [x] `textDocument/didOpen` and `didChange` parse and publish syntax diagnostics.
- [x] Document symbols for module exports and top-level bindings.

## L2: definition + completion (names)

- [x] `textDocument/definition` within the current file (modules, exports, defs).
- [ ] `textDocument/definition` works across modules and imports.
- [x] `textDocument/completion` includes keywords and current-module symbols.
- [ ] `textDocument/completion` includes imported module exports.

## L3: hover + signature help

- [ ] `textDocument/hover` shows inferred type + docstring (if available).
- [ ] `textDocument/signatureHelp` supports curried functions and piped calls.

## L4: references + rename

- [ ] `textDocument/references` finds all uses in workspace.
- [ ] `textDocument/rename` updates references and module exports safely.

## L5: formatting + code actions

- [ ] `textDocument/formatting` uses CST-driven formatter.
- [ ] `textDocument/codeAction` includes:
  - import missing name
  - add type annotation
  - add match cases for ADTs

## L6: semantic tokens + inlay hints

- [ ] Tokens for value/type/module/constructor ids.
- [ ] Optional inlay hints for inferred types and effect requirements.

## Quality constraints

- Never crash on partial code.
- Stable ordering and deterministic diagnostics.
- Latency budget: keep most requests under 200ms for small files.
