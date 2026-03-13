---
apply: always
---

# AIVI Agent Guide

Authoritative guide for AI agents working on the AIVI language project. Covers the relationship between specifications, implementation crates, integration tests, and editor tooling.

This file lives at `~/desk/mendrik/aivi` alongside the AIVI source tree.

Full language specs live under `specs/`. `AIVI_LANGUAGE.md` is the quick-reference companion, not the source of truth.

## 1. Project Structure & Relationships

### 1.1 Directory Map

| Directory                 | Role                            | Notes                                                                                                                       |
|:------------------------- |:------------------------------- |:--------------------------------------------------------------------------------------------------------------------------- |
| `specs/`                  | **Source of Truth** & **Docs Site** | All language/spec docs live here. If code contradicts specs, the code is wrong (see §2.1). `specs/nav.mjs` is the TOC source of truth. |
| `crates/aivi_core`        | **Compiler core**               | Parser (CST/surface), HIR, Kernel, typechecker, formatter. Re-uses shared source via `#[path]` from `crates/aivi`.      |
| `crates/aivi`             | **Runtime + CLI**               | Cranelift JIT/AOT backend, stdlib implementations, `aivi` binary.                                                          |
| `crates/aivi_database`    | **Database support**            | SQLite, PostgreSQL, and MySQL support used by runtime/database features.                                                   |
| `crates/aivi_email`       | **Email support**               | IMAP/email support used by runtime/source features.                                                                         |
| `crates/aivi_gtk4`        | **GTK4 UI runtime**             | GTK4/libadwaita bindings backing the signal-first UI runtime.                                                               |
| `crates/aivi_lsp`         | **LSP server**                  | `aivi-lsp` binary. Semantic tokens, hover, completion, formatting, diagnostics.                                            |
| `crates/aivi_driver`      | **Driver lib**                  | Shared driver logic consumed by CLI and LSP.                                                                                |
| `crates/aivi_http_server` | **HTTP server**                 | Runtime HTTP server support.                                                                                                |
| `crates/aivi_mcp`         | **MCP server**                  | Local MCP server and GTK inspection/automation bridge.                                                                      |
| `crates/doc_index_gen`    | **Doc tooling**                 | Generates doc indices for LSP hover and the docs site.                                                                      |
| `integration-tests/`      | **Validation**                  | Canonical `.aivi` files: `syntax/`, `runtime/`, `stdlib/`, `complex/`, `compile_fail/`.                                    |
| `vscode/`                 | **VSCode extension**            | TypeScript, pnpm. Grammar auto-generated via `gen_vscode_syntax`; ships a bundled `aivi-lsp`.                              |
| `fuzz/`                   | **Fuzz targets**                | cargo-bolero fuzz targets: parser, frontend, runtime, formatter, LSP pipeline, type inference.                              |

### 1.2 Crate Dependency Flow

```
specs/  →  crates/aivi_core  →  crates/aivi (runtime/stdlib/CLI)
                              →  crates/aivi_lsp (LSP server)
crates/aivi  →  crates/aivi_database / crates/aivi_email / crates/aivi_gtk4 / crates/aivi_http_server / crates/aivi_mcp
crates/aivi (gen_vscode_syntax bin)  →  vscode/syntaxes/
crates/aivi_lsp  →  vscode/ (bundled binary)
```

### 1.3 Key Source Locations

| Concern              | Path                                                                 |
|:-------------------- |:-------------------------------------------------------------------- |
| Lexer                | `crates/aivi/src/lexer.rs`                                           |
| Parser / CST         | `crates/aivi/src/syntax.rs` + `crates/aivi/src/cst.rs`               |
| Surface AST          | `crates/aivi/src/surface/`                                           |
| HIR                  | `crates/aivi/src/hir/`                                               |
| Kernel IR            | `crates/aivi/src/kernel/`                                            |
| Typechecker          | `crates/aivi/src/typecheck/`                                         |
| Formatter            | `crates/aivi_core/src/formatter/`                                    |
| Stdlib (Rust)        | `crates/aivi/src/stdlib/`                                            |
| Cranelift JIT/AOT    | `crates/aivi/src/cranelift_backend/`                                 |
| Runtime interpreter  | `crates/aivi/src/runtime/`                                           |
| GTK runtime bindings | `crates/aivi_gtk4/src/`                                              |
| Database support     | `crates/aivi_database/src/`                                          |
| Email support        | `crates/aivi_email/src/`                                             |
| LSP backend          | `crates/aivi_lsp/src/backend/`                                       |
| LSP semantic tokens  | `crates/aivi_lsp/src/semantic_tokens/`                               |
| MCP tooling          | `crates/aivi_mcp/src/mcp/`                                           |
| VSCode grammar gen   | `crates/aivi/src/bin/gen_vscode_syntax.rs`                           |
| VSCode syntaxes      | `vscode/syntaxes/` (generated — do not hand-edit)                    |

### 1.4 Terminal Command Tips

- Avoid printing full build logs; only show warnings/errors.

---

## 2. The Golden Rules

### 2.1 Specs Are the Source of Truth

`specs/` always wins over Rust code.

- Before touching any compiler or stdlib code, read the relevant spec file(s).
- **If Rust code and specs disagree**: the code is wrong. Fix the code to match the spec.
- **If it is genuinely unclear which is correct** (e.g., the spec is ambiguous, the divergence is intentional, or the feature predates the spec): **stop and ask for clarification before proceeding.** Do not silently pick a side.
- If the spec has a gap (describes a feature incompletely), document the gap with a `<!-- TODO: spec gap -->` comment and ask before filling it with assumptions.
- If something should work according to specs but does not, you MUST fix the underlying problem rather than working around it.

### 2.2 No Dead Code After Feature Changes

When replacing a feature with something new (different syntax, different API, renamed construct, etc.):

- **Delete** all Rust code implementing the old feature.
- **Delete** all integration test files and AIVI snippets that used the old form.
- **Delete** the old spec page(s) or sections, not just add a note saying "deprecated".
- **Remove** any grammar rules or token types from the LSP/VSCode integration that only served the old feature.

The goal: after the change, `git grep` for the old construct returns zero results (except this AGENTS.md if mentioned for documentation purposes).

### 2.3 New Features Must Fully Propagate

Adding a new language or stdlib feature is never complete until all layers are updated. Propagation order:

1. **`specs/`** — write or update the spec page first (see §5).
2. **`crates/aivi_core`** — parser, typechecker, formatter.
3. **`crates/aivi`** and any support crates (`crates/aivi_database`, `crates/aivi_email`, `crates/aivi_gtk4`, `crates/aivi_http_server`, `crates/aivi_mcp`) — runtime/stdlib implementation, connector/native support, and Cranelift lowering if applicable.
4. **`crates/aivi_lsp`** — semantic token types, hover docs, completion items, diagnostics.
5. **`vscode/syntaxes/`** — regenerate via `cargo run -p aivi --bin gen_vscode_syntax -- vscode/syntaxes` (do not hand-edit generated files).
6. **`integration-tests/`** — add at least one positive test and, if the feature has failure modes, a `compile_fail/` test.
7. **`AIVI_LANGUAGE.md`** — update the LLM quick-reference.

Skipping any layer leaves the tooling in an inconsistent state and will break CI.

---

## 3. Development Workflow

### 3.1 Task Execution Protocol

1. **Read specs first**: `specs/syntax/`, `specs/stdlib/`, `specs/tools/`, `specs/typed_codegen/`.
2. **Locate implementation**: find the Rust source using the table in §1.3.
3. **Check for discrepancies** between spec and code; apply §2.1.
4. **Implement** in small, testable units. Follow propagation order from §2.3.
5. **Verify** (see §7 for commands).

### 3.2 Clean as You Cook

- Fix syntax violations you encounter in passing (e.g., `let x =` → `x =`).
- If a file/function is referenced nowhere after your change, delete it.
- Keep files focused; propose splitting large files when they become unwieldy.
- Fix pre-existing issues you notice as part of the current task.

---

## 4. AIVI Language Best Practices

When writing `.aivi` code in `integration-tests/`, `specs/` snippets, or doc examples:

### 4.1 Style

- `lowerCamelCase` for values/functions; `UpperCamelCase` for types, constructors, domains, classes.
- Module path segments and file names: `snake_case` (e.g. `my_app.command_queue` → `my_app/command_queue.aivi`).
- Bindings are immutable; use recursion, folds, or generators instead of loops.
- See `AIVI_LANGUAGE.md` for a concise syntax reference.

### 4.2 Safety

- No nulls — use `Option A` or `Result E A`.
- Exhaustive pattern matches; use `_` only when every remaining case is truly identical.
- Use `resource { ... }` blocks for handles (files, connections) to guarantee cleanup.

### 4.3 Concurrency

- Spawn tasks inside `scope` from `aivi.concurrency`.
- Communicate via typed channels (`Send`/`Recv`), not shared mutable state.

### 4.4 GTK4 UI — Signal-First Runtime

When writing GTK4 UI code, **prefer the signal-first runtime**. Signals are the source of truth, GTK sigils bind widget props and child structure directly to those signals, and callbacks mutate signals or trigger `Event` handles. Do **not** introduce `gtkApp`, `Model -> Msg -> update` host loops, or the deprecated `signalBind*` helper family in new code.

The public AIVI surface is `aivi.ui.gtk4`; the Rust bindings that back it live in `crates/aivi_gtk4/`.

**Preferred pattern — mount a live GTK tree backed by signals:**

```aivi
use aivi.reactive

state = signal { count: 0 }
title = derive state (s => "Count {s.count}")
increment = _ => update state (patch { count: _ + 1 })

root = ~<gtk>
  <GtkApplicationWindow title="My App" defaultWidth={800} defaultHeight={600}>
    <GtkBox orientation="vertical" spacing="8">
      <GtkLabel label={title} />
      <GtkButton label="Increment" onClick={increment} />
    </GtkBox>
  </GtkApplicationWindow>
</gtk>

main = runGtkApp {
  appId: "com.example.app"
  root: root
  onStart: pure Unit
}
```

**Lower-level pattern — `signalStream` for manual integrations:**

```aivi
use aivi.concurrency

rx <- signalStream {}
concurrency.forEach rx (event =>
  event match
    | GtkTick                            => update state step
    | GtkInputChanged _ "nameInput" txt  => set name txt
    | _                                  => pure Unit
)
```

**Rules for agents:**

- Prefer mounted signal-bound trees over host loops.
- Prefer `runGtkApp` for normal root-window apps and `mountAppWindow` when startup needs the mounted `WindowId`; use `buildFromNode` / `buildWithIds` for subtrees, tests, and lower-level escape hatches.
- `reconcileNode` is a low-level escape hatch for structural hosting work, not the primary architecture.
- Signal events carry both `WidgetId` and the widget's `id="..."` name (e.g., `GtkClicked widgetId "saveBtn"`). Match by name string instead of comparing integer IDs.
- Call `signalStream {}` once per manual event flow and keep ownership/cleanup explicit.
- Use `attempt (widgetById "id")` when a widget may not exist yet (e.g., dialog content not yet built).
- Prefer typed variants (`GtkClicked`, `GtkInputChanged`, `GtkToggled`, etc.) over `GtkUnknownSignal` wherever possible.
- `signalPoll` is available for one-shot reads; `signalStream` is preferred for continuous loops.
- `channel.fold` threads state over events; `channel.forEach` runs a stateless action per event.

### 4.5 Decorators

Only decorators listed in `specs/syntax/decorators.md` are valid in v0.1. Decorators are compiler/tooling pragmas only — never domain-specific annotations like `@sql`, `@schema`, `@model`.

---

## 5. Documentation Maintenance

### 5.1 Specs

- Update `specs/` **before or alongside** code changes, never after the fact.
- **TOC**: `specs/nav.mjs` is the single source of truth. Do NOT hand-edit `specs/index.md` or `specs/README.md` TOC sections (they are auto-synced by the `sync-specs-toc.yml` GitHub Action).
- Ensure all internal spec links are valid after changes.
- When removing a feature: delete the spec page, remove it from `specs/nav.mjs`.
- When adding a feature: add the spec page, add it to `specs/nav.mjs`.

### 5.2 Code Examples in Specs

- Format all AIVI snippets with `cargo run -p aivi -- fmt <path>` or `aivi fmt <path>`.
- Use ```` ```aivi ```` fenced blocks.
- Keep formatter-style alignment: `<-` in `do`/effect blocks, `=>` in match arms.

### 5.3 LSP Hover / Quick-Info Markers

Wrap prose in spec files with `<!-- quick-info: {JSON} --> ... <!-- /quick-info -->` markers (see `specs/doc-markers-spec.md`) so the LSP can surface it on hover. Do not duplicate prose just for tooling.

### 5.4 External Source Docs

When touching source-boundary runtime behavior, keep these aligned:

- `specs/syntax/external_sources.md`
- `specs/syntax/external_sources/schema_first.md`
- `specs/syntax/external_sources/composition.md`
- `specs/syntax/external_sources/file.md`
- `specs/syntax/external_sources/rest_http.md`
- `specs/syntax/external_sources/environment.md`
- `specs/syntax/external_sources/imap_email.md`
- `specs/syntax/external_sources/image.md`
- `specs/syntax/external_sources/compile_time.md`

Each source doc must include: current v0.1 API surface, one realistic example, and failure/diagnostic behavior notes.

---

## 6. Rust Implementation Guidelines

- **Parser**: error-tolerant (must recover from syntax errors for LSP use).
- **CST**: preserve full fidelity (whitespace, comments) for the formatter.
- **Diagnostics**: rich spans with help text.
- **`aivi_core`** re-uses source files from `crates/aivi/src/` via `#[path = "..."]` — when physically moving files, update both crates.
- **Dependencies**: prefer established Rust crates over hand-rolled algorithms (e.g., `rustfft`, not a custom FFT).

### 6.1 Testing

| Test kind              | Location                        | Command                                |
|:---------------------- |:------------------------------- |:-------------------------------------- |
| Rust unit tests        | `#[test]` in source or `tests/` | `cargo nextest run --workspace`        |
| AIVI integration       | `integration-tests/`            | `cargo nextest run --workspace` via the runners in `crates/aivi/tests/` |
| LSP tests              | `crates/aivi_lsp/src/`          | `cargo nextest run --workspace`        |
| Snapshot tests         | `crates/aivi/tests/` (insta)    | `cargo nextest run --workspace`        |
| VSCode unit tests      | `vscode/src/test/unit/`         | `cd vscode && pnpm run test:unit`      |
| VSCode integration     | `vscode/src/test/integration/`  | `cd vscode && pnpm run test:integration` |
| Fuzz (CI)              | `fuzz/fuzz_targets/`            | `cargo bolero test ...` (see CI workflow) |

When `crates/aivi_lsp` changes, rebuild the bundled server with `pnpm run build` in `vscode/` before running the VSCode integration suite; the extension prefers `vscode/bin/aivi-lsp` over `PATH`.

---

## 7. Project Validation (must all pass before concluding)

Run from the repo root in order:

```bash
# 1. Rust: format check
cargo fmt --all -- --check

# 2. Rust: lint
cargo clippy --workspace --all-targets -- -D warnings

# 3. Rust + AIVI integration tests
cargo nextest run --workspace

# 4. VSCode extension (only needed when vscode/ or aivi_lsp/ changed)
cd vscode && pnpm install --frozen-lockfile && pnpm run build && pnpm run test:all && cd ..

# 5. Docs site — dead-link check + build (only needed when specs/ changed)
cd specs && pnpm install --frozen-lockfile && pnpm docs:build && cd ..
```

On test failure, get full output with:

```bash
cargo nextest run --workspace --no-capture 2>&1 | grep -A 20 "FAILED\|panicked"
```

Both Rust tests and AIVI integration tests must be green before any task is considered complete.

---

## 8. Stale Build Artefacts

- `aivi run` → Cranelift JIT (in-memory, no filesystem artefacts).
- `aivi build` → Cranelift AOT: emits `target/aivi-gen/aivi_program.o` + thin Rust harness → linked by `cargo build`.

---

## 9. Safety Checklist

Before finishing any change:

- [ ] Read the relevant `specs/` file(s) first
- [ ] No spec ↔ code discrepancy (or asked for clarification)
- [ ] No dead code left from a replaced feature
- [ ] New feature propagated to all layers (§2.3)
- [ ] `specs/nav.mjs` updated (not `index.md` TOC)
- [ ] `AIVI_LANGUAGE.md` updated
- [ ] AIVI code snippets formatted with `aivi fmt`
- [ ] At least one integration test added (positive + `compile_fail/` if applicable)
- [ ] Make sure touched `.aivi` files have no unexpected LSP/formatting issues (intentional `compile_fail` and fuzz fixtures are exempt)
- [ ] No dead links in `specs/` (`pnpm docs:build` in `specs/` reports none)
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo nextest run --workspace` is fully green (Rust + AIVI tests)
- [ ] VSCode extension builds/tests pass (`pnpm run build && pnpm run test:all` in `vscode/`) if LSP or grammar changed
- [ ] Aivi builds without warnings (also pre-existing ones).
 
---

# Acceptance criteria

- Produce complete, working code.
- Do not leave TODOs, placeholders, or stub functions.
- Every function must contain a full implementation.
- If something is complex, implement a simple working version rather than leaving a stub.
- If the solution is long, continue generating until the implementation is complete.
