# M8 LSP — Workplan

## Milestones

### L1: diagnostics + symbols

- [x] Wire `didOpen`/`didChange` to parse.
- [x] Publish syntax diagnostics.
- [x] Provide document symbols for module exports and top-level bindings.

### L2: navigation + completion

- [ ] Resolve definitions across modules.
- [x] Provide basic completions (keywords, in-file names, exports).

### L3: types in UX

- [ ] Hover for inferred types and docs.
- [ ] Signature help for calls and pipes.

### L4: references + rename

- [ ] Workspace-wide references.
- [ ] Rename with export updates and conflict checks.

### L5: formatting + code actions

- [ ] CST formatting with stable output.
- [ ] Quick fixes for imports, type annotations, match cases.

### L6: semantic tokens + quality

- [ ] Tokens for value/type/module/constructor IDs.
- [ ] Optional inlay hints (types, effects).

## Dependencies

- Stable `Span`/`Range` mapping and `SourceMap`.
- Resolver and typechecker resilience on partial code.
- Formatter correctness on the current CST.

## Risks

- Latency regressions with larger workspaces.
- Incomplete module graph when files are missing/unsaved.
- Rename/refactor requires canonical symbol IDs and safe edits.

## Success metrics

- <200ms diagnostics for small files.
- No crashes on malformed or incomplete input.
- Users can navigate and rename across `specs/` and `examples/`.
