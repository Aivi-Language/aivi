# Missing Features & Gap Analysis (v0.1)

This page tracks **documentation and implementation gaps** between the `specs/` and the current compiler/runtime in `crates/`.

> Scope: AIVI **v0.1**.
>
> This document is intentionally lightweight: it is meant to provide a stable link target from the introduction/README and keep the VitePress build green.
> Please expand sections as features stabilize or as gaps are discovered.

## How to use this page

- If the **spec defines** a feature but it is **not implemented**, note it here and link to the relevant spec section.
- If the **implementation supports** behavior not yet described in the spec, **update the spec first** and then reflect it here.
- Prefer objective status:
  - **Specified**: exists in `specs/`
  - **Implemented**: available in `crates/`
  - **Tested**: covered by `integration-tests/`

## Tracking

### Syntax

- **`do M { ... }` — Generic Monadic Blocks** (Future consideration)
  - Status: Not specified, not implemented
  - Proposal: Introduce `do M { ... }` as general monadic do-notation where `M` is any type with a `Monad` instance.
  - `effect { ... }` would become sugar for `do (Effect E) { ... }` (with special features like `or` fallback, resource acquisition).
  - `generate { ... }` stays separate (different semantics: `yield`, guards, `recurse`).
  - Enables: `do Option { x <- someOpt; y <- anotherOpt; pure (x + y) }`
  - Trade-offs: Requires explicit monad annotation to avoid inference ambiguity; adds surface syntax complexity.
  - Decision: Defer to post-v0.1; current `?` pattern matching and `flatMap`/`chain` suffice for `Option`/`Result`.

### Kernel & Desugaring

- (add items; start at `specs/03_kernel/` and `specs/04_desugaring/`)

### Standard Library

- (add items; start at `specs/05_stdlib/`)

### Runtime

- (add items; start at `specs/06_runtime/`)

### Tooling (LSP / formatter / docs)

- (add items; start at `crates/aivi_lsp/` and `specs/07_tools/`)
