# Overall Phases (Milestones + Acceptance Criteria)

This plan is intentionally incremental: each milestone produces something shippable and dogfoodable, even if the full spec is not implemented yet.

## Guiding principles

- Implement the **Kernel** first, then desugar the surface language into it (matches `specs/03_kernel` and `specs/04_desugaring`).
- Prefer a compiler architecture that is also an IDE engine: one parser, one name resolver, one typechecker.
- Start with **WASM MVP + WASI** and a small runtime; keep a clean seam to later adopt **WasmGC** and/or the **Component Model**.
- Treat “Domains”, “Effects”, and “Resources” as first-class: design them early, implement them progressively.

## M0 — Repo + CI scaffolding (1–3 days)

Deliverables:
- Rust workspace skeleton (`crates/*`) and a single `aivi` binary.
- `aivi --help` with subcommands: `parse`, `check`, `fmt` (stub), `lsp` (stub), `build` (stub), `run` (stub).
- “Hello world” golden test for parsing a single file.

Acceptance criteria:
- `cargo test` runs in CI.
- `aivi parse examples/...` prints a stable AST (or CST) and exits 0.

## M1 — Parser + CST + diagnostics (1–2 weeks)

Scope:
- Lexing + parsing for most of `specs/02_syntax` that doesn’t require type info.
- A CST that preserves trivia (comments/whitespace) for formatting and IDE.

Deliverables:
- Syntax errors with spans, recovery, and multiple diagnostics per file.
- Formatter prototype: a “pretty printer” for a subset (enough to format examples).
- Minimal VS Code integration: syntax highlighting already exists; add “format document” that shells out to `aivi fmt` (optional until LSP).

Acceptance criteria:
- Parse all files in `examples/` and most `specs/` code blocks (even if some fail).
- Stable diagnostics spans and messages.

## M2 — Modules + name resolution (1–2 weeks)

Scope:
- `module ... = { export ... }` forms and `use` imports (`specs/02_syntax/10_modules.md`).
- Symbol tables, module graph, and “go to definition”.

Deliverables:
- `aivi check` that resolves identifiers across a small workspace.
- LSP: `textDocument/definition` for values/types/modules (even before types).

Acceptance criteria:
- “Unknown name”, “duplicate export”, “cyclic module” errors with good spans.

## M3 — Kernel IR + desugaring pipeline (2–4 weeks)

Scope:
- A small Kernel that matches `specs/03_kernel/01_core_terms.md` through records/patterns/predicates.
- Surface-to-kernel desugaring (`specs/04_desugaring/*`).

Deliverables:
- `aivi desugar` debug output.
- Internal “HIR” (typed later) with stable IDs for IDE and incremental compilation.

Acceptance criteria:
- Any supported surface feature lowers to Kernel consistently (round-trip tests).

## M4 — Type system v1 (rank-1, no HKTs/classes yet) (3–6 weeks)

Scope:
- Enough types to make the language usable:
  - ADTs, functions, records (start closed), pattern matching.
  - `Option`, `Result`, `List`, `Text` as library types with compiler-known representation.
  - Effect typing as annotations (initially: check the *shape*, don’t implement full effect inference).

Deliverables:
- `aivi check` produces type errors with explanatory traces.
- LSP: hover types, signature help, completion using local typing.

Acceptance criteria:
- Small programs typecheck; errors are actionable; no “mystery type” output.

## M5 — WASM/WASI execution (interpreter optional, compiler required) (4–8 weeks)

Scope:
- Compile a typed Kernel program to WASM and run it under Wasmtime.
- WASI integration for basic IO through `Effect` operations.

Deliverables:
- `aivi build --target wasm32-wasi` emits a `.wasm`.
- `aivi run` runs the produced artifact.
- Basic runtime support: heap allocation and a minimal string/list representation.

Acceptance criteria:
- Deterministic outputs for golden tests; no UB; memory safe by construction.

## M6 — Effects, Resources, Concurrency (6–12 weeks, staged)

Scope:
- Implement `Effect E A` semantics and runtime handlers.
- Resource lifetimes and structured concurrency (`specs/06_runtime/01_concurrency.md`).

Deliverables:
- Built-in effects: `Clock`, `File`, `Random` (start small).
- Deterministic cancellation semantics.

Acceptance criteria:
- Concurrency tests for cancellation and channel select.

## M7 — Domains + patching + JSX/HTML (ongoing; prioritize for demos)

Scope:
- Domain definitions and operator interpretation (`specs/02_syntax/06_domains.md`, `specs/02_syntax/11_domain_definition.md`).
- Patching semantics (`specs/02_syntax/05_patching.md` + kernel equivalents).
- JSX literals to `Html` domain (`specs/02_syntax/13_jsx_literals.md`).

Deliverables:
- A small “HTML domain” demo that produces a tree and prints/serializes it.
- Domain-driven numeric deltas (calendar/duration/color) as in `specs/05_stdlib/*`.

Acceptance criteria:
- Domain-specific operators are typechecked and can be extended in user code.

## M8 — LSP “daily driver” (parallel track)

Scope:
- Make the language usable in an editor for real work.

Deliverables:
- Diagnostics, formatting, definition/references, rename, hover/types, semantic tokens, code actions.

Acceptance criteria:
- Comfortable editing experience on the existing `specs/` and `examples/`.

## M9 — MCP (parallel track; enabled once execution works)

Scope:
- A Rust MCP host that loads AIVI WASM modules and exposes decorated tools/resources.

Deliverables:
- `aivi mcp serve` exposing `@mcp_tool` and `@mcp_resource`.
- JSON Schema generation from AIVI types.

Acceptance criteria:
- An MCP client can call tools, list resources, and get typed errors.

## M10 — Type system v2 (row polymorphism, classes, HKTs) (longer-term)

Scope:
- Open structural records (rows).
- Classes (ad-hoc polymorphism).
- HKTs.

Deliverables:
- A principled, testable typechecker with exhaustive coverage.

Acceptance criteria:
- The “advanced” features don’t compromise IDE responsiveness or error quality.

