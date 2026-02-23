# Missing Features & Gap Analysis (v0.1)

This page tracks **documentation and implementation gaps** between the `specs/` and the current compiler/runtime in `crates/`.

> Scope: AIVI **v0.1**.
> 
> This document is intentionally lightweight: it is meant to provide a stable link target from the introduction/README and keep the VitePress build green.
> Update this page when gaps are discovered or resolved.

## How to use this page

- If the **spec defines** a feature but it is **not implemented**, note it here and link to the relevant spec section.
- If the **implementation supports** behavior not yet described in the spec, **update the spec first** and then reflect it here.
- Prefer objective status:
  - **Specified**: exists in `specs/`
  - **Implemented**: available in `crates/`
  - **Tested**: covered by `integration-tests/`

## Tracking

### Syntax

- `on` statement in `do Effect { ... }` blocks: listed in the [grammar](syntax/grammar.md) but semantics are not documented in any syntax chapter.

### Standard Library

- Streaming sources remain out of scope in runtime v0.1 (e.g. continuous `file.watch`/network stream source adapters).
- Full typed source error ADT plumbing (`SourceError K`) is partially implemented in runtime but not yet fully reflected in typechecker aliases.
- Several stdlib modules still have **stub specs** (see below).
- Add OAuth2 module
  
  #### Stub specs (no API tables / no functions documented)

| Module | File                                  | Status                                           |
|:------ |:------------------------------------- |:------------------------------------------------ |
| Log    | [system/log.md](stdlib/system/log.md) | Specified-only (not implemented in runtime v0.1) |

### Runtime

- WASM/WASI compilation target: described in the introduction but not yet implemented.

### Tooling (LSP / formatter / docs)

- `aivi mcp serve`: currently only serves spec resources; tool execution is not yet available.
