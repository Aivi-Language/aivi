# Rust Workspace Layout (Suggested Crates + Responsibilities)

The primary goal is to keep a single “source of truth” for parsing, resolution, and typing that both the compiler and LSP use.

## Proposed workspace

Binary crates:
- `crates/aivi_cli`: `aivi` command, config, project loading, build graph.
- `crates/aivi_lsp`: LSP server entrypoint (or a lib + small bin).
- `crates/aivi_mcp`: MCP host entrypoint (or a lib + small bin).

Core libraries:
- `crates/aivi_span`: `Span`, `FileId`, `SourceMap`, diagnostics primitives.
- `crates/aivi_lexer`: tokenization; keep it deterministic and fast.
- `crates/aivi_cst`: green-tree or CST structures; preserves trivia.
- `crates/aivi_parser`: parses tokens → CST; error recovery; produces diagnostics.
- `crates/aivi_ast`: AST “views” over CST + lossless conversions where needed.
- `crates/aivi_hir`: lowered, resolved IR with stable IDs; IDE-friendly.
- `crates/aivi_resolve`: module graph, name resolution, import/export rules.
- `crates/aivi_desugar`: surface → kernel lowering (`specs/04_desugaring`).
- `crates/aivi_kernel`: kernel IR definitions (`specs/03_kernel`).
- `crates/aivi_types`: type representations, inference/checking, unification, errors.
- `crates/aivi_effects`: effect system typing and runtime interface.
- `crates/aivi_domains`: domain registration, operator dispatch, domain metadata.

Codegen + runtime:
- `crates/aivi_codegen_wasm`: WASM emission (core module and/or component).
- `crates/aivi_runtime`: runtime support library (heap, strings, lists, tags).
- `crates/aivi_wasi`: WASI bindings layer + effect handlers (host + guest sides).

Tooling + metadata:
- `crates/aivi_fmt`: formatter and printer (CST-driven).
- `crates/aivi_tests`: golden tests harness (snapshots of diagnostics/IR/WASM).
- `crates/aivi_doc`: doc extraction and optional site generation hooks.

## Recommended Rust dependencies (initial)

- Parsing:
  - Lexer: `logos` (fast) or a hand-written lexer if you need precise control.
  - Parser: `rowan` + hand-written Pratt/recursive-descent, or `chumsky` if you prefer parser combinators.
- Incremental compilation / IDE:
  - `salsa` (query-based incremental computation).
- LSP:
  - `tower-lsp`.
- WASM:
  - `wasm-encoder` (direct emission) or `cranelift` (higher-level codegen).
  - `wasmtime` for running in dev/tests.
  - Later (component model): `wasmtime::component`, `wit-component`, `wit-parser`.
- Testing:
  - `insta` for snapshots, `proptest` for fuzz/property tests.

## Workspace conventions

- Every phase adds tests: parser recovery, resolution, type errors, codegen golden outputs.
- “Debug printing” is a supported interface: AST/HIR/Kernel pretty printers should be stable to snapshot.
- Keep the boundary between “surface language” and “kernel” extremely explicit.

