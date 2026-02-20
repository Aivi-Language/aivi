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

- `on` statement in `do Effect { ... }` blocks: listed in the [grammar](02_syntax/00_grammar.md) but semantics are not documented in any syntax chapter.
- Generic `do M { ... }`: parser accepts it, but HIR lowering currently discards the monad name and hardcodes `Effect` (see [do notation § Type Checking](02_syntax/16_do_notation.md#type-checking)).

### Kernel & Desugaring

- ADT typed emission in codegen: `CgType` ADT constructors exist but type-to-cg-type lowering returns empty constructor args (see [Typed Codegen](08_typed_codegen/01_design.md#not-yet-implemented)).

### Standard Library

- A calculus module.
- Structured codecs (`file.json`, `file.csv`) and streaming sources — specified in [External Sources § 12.1](02_syntax/12_external_sources.md) but not yet implemented.
- `env.decode` (structured environment decoding) — specified but not implemented.
- Several stdlib modules have **stub or incomplete specs** (see below).

#### Stub specs (no API tables / no functions documented)

| Module | File | Status |
| :--- | :--- | :--- |
| Testing | [27_testing.md](05_stdlib/00_core/27_testing.md) | Stub — only overview blurb |
| Layout | [04_ui/01_layout.md](05_stdlib/04_ui/01_layout.md) | Stub — only overview blurb |
| System | [03_system/26_system.md](05_stdlib/03_system/26_system.md) | Stub — no functions |
| Log | [03_system/27_log.md](05_stdlib/03_system/27_log.md) | Stub — "Planned" status |

#### Incomplete specs (thin API coverage)

| Module | File | Gap |
| :--- | :--- | :--- |
| Instant | [02_chronos/01_instant.md](05_stdlib/02_chronos/01_instant.md) | No helper functions table |
| Duration | [02_chronos/03_duration.md](05_stdlib/02_chronos/03_duration.md) | No helper functions table |
| TimeZone | [02_chronos/04_timezone.md](05_stdlib/02_chronos/04_timezone.md) | No helper functions table |
| Regex | [00_core/24_regex.md](05_stdlib/00_core/24_regex.md) | No usage examples |
| Generator | [00_core/30_generator.md](05_stdlib/00_core/30_generator.md) | No usage examples |
| Tree | [01_math/19_tree.md](05_stdlib/01_math/19_tree.md) | No API table, no quick-info marker |
| Linear Algebra | [01_math/18_linear_algebra.md](05_stdlib/01_math/18_linear_algebra.md) | Only 3 functions; no general NxN solver |
| Probability | [01_math/13_probability.md](05_stdlib/01_math/13_probability.md) | No `sample` function; no standard distributions |
| Signal | [01_math/14_signal.md](05_stdlib/01_math/14_signal.md) | Only 4 functions; no signal creation or filtering |
| Geometry | [01_math/15_geometry.md](05_stdlib/01_math/15_geometry.md) | Only 5 functions; thin collision detection |
| File | [03_system/20_file.md](05_stdlib/03_system/20_file.md) | No directory operations; no usage examples |
| URL | [03_system/25_url.md](05_stdlib/03_system/25_url.md) | Only 2 functions |
| Path | [03_system/24_path.md](05_stdlib/03_system/24_path.md) | Only 5 functions |
| Sockets | [03_network/04_sockets.md](05_stdlib/03_network/04_sockets.md) | No UDP functions |
| Streams | [03_network/05_streams.md](05_stdlib/03_network/05_streams.md) | No stream combinators |
| HTTP | [03_network/01_http.md](05_stdlib/03_network/01_http.md) | Only 3 functions; no headers/timeouts |
| HTTP Server | [03_network/03_http_server.md](05_stdlib/03_network/03_http_server.md) | No routing or middleware |

### Runtime

- WASM/WASI compilation target: described in the introduction but not yet implemented.
- Native codegen for generic `do M { ... }` blocks: stubbed.

### Tooling (LSP / formatter / docs)

- `aivi mcp serve`: currently only serves spec resources; tool execution is not yet available.
