---
apply: always
---

# AIVI Agent Guide

Authoritative guide for AI agents working on the AIVI language project. Covers the relationship between specifications, implementation crates, integration tests, and editor tooling.

## 1. Project Structure & Relationships

### 1.1 Directory Map

| Directory                 | Role                 | Notes                                                                                            |
|:------------------------- |:-------------------- |:------------------------------------------------------------------------------------------------ |
| `specs/`                  | **Source of Truth**  | All logic derives from here. If Rust code contradicts specs, the code is wrong (see §2.1).       |
| `crates/aivi_core`        | **Compiler core**    | Parser (CST/surface), HIR, Kernel, typechecker, formatter. Shared via `#[path]` by other crates. |
| `crates/aivi`             | **Runtime + CLI**    | Cranelift JIT/AOT backend, stdlib implementations, `aivi` binary.                                |
| `crates/aivi_lsp`         | **LSP server**       | `aivi-lsp` binary. Semantic tokens, hover, completion, formatting, diagnostics.                  |
| `crates/aivi_driver`      | **Driver lib**       | Shared driver logic consumed by CLI and LSP.                                                     |
| `crates/aivi_http_server` | **HTTP server**      | Runtime HTTP server support.                                                                     |
| `crates/doc_index_gen`    | **Doc tooling**      | Generates doc indices for LSP hover and the docs site.                                           |
| `integration-tests/`      | **Validation**       | Canonical `.aivi` files: `syntax/`, `runtime/`, `stdlib/`, `complex/`, `compile_fail/`.          |
| `vscode/`                 | **VSCode extension** | TypeScript, pnpm. Grammar auto-generated via `gen_vscode_syntax` binary.                         |
| `zed/`                    | **Zed extension**    | Tree-sitter grammar (`zed/grammars/aivi/`), language config, dark theme.                         |
| `specs/`                  | **Docs site**        | VitePress site. `specs/nav.mjs` is the single TOC source of truth.                               |
| `ui-client/`              | **Browser client**   | Vite micro-client for `aivi.ui.ServerHtml`: DOM patches via WebSocket, event delegation.         |
| `fuzz/`                   | **Fuzz targets**     | cargo-bolero fuzz targets: parser, frontend, runtime, formatter, LSP pipeline, type inference.   |

### 1.2 Crate Dependency Flow

```
specs/  →  crates/aivi_core  →  crates/aivi (runtime/stdlib/CLI)
                              →  crates/aivi_lsp (LSP server)
crates/aivi (gen_vscode_syntax bin)  →  vscode/syntaxes/
crates/aivi_lsp  →  vscode/ (bundled binary)
zed/grammars/aivi/  (Tree-sitter grammar, independent)
```

### 1.3 Key Source Locations

| Concern             | Path                                                   |
|:------------------- |:------------------------------------------------------ |
| Lexer               | `crates/aivi/src/lexer.rs`                             |
| Parser / CST        | `crates/aivi/src/syntax.rs` + `crates/aivi/src/cst.rs` |
| Surface AST         | `crates/aivi/src/surface/`                             |
| HIR                 | `crates/aivi/src/hir/`                                 |
| Kernel IR           | `crates/aivi/src/kernel/`                              |
| Typechecker         | `crates/aivi/src/typecheck/`                           |
| Formatter           | `crates/aivi_core/src/formatter/`                      |
| Stdlib (Rust)       | `crates/aivi/src/stdlib/`                              |
| Cranelift JIT/AOT   | `crates/aivi/src/cranelift_backend/`                   |
| Runtime interpreter | `crates/aivi/src/runtime/`                             |
| LSP backend         | `crates/aivi_lsp/src/backend/`                         |
| LSP semantic tokens | `crates/aivi_lsp/src/semantic_tokens/`                 |
| VSCode grammar gen  | `crates/aivi/src/bin/gen_vscode_syntax` (or similar)   |
| VSCode syntaxes     | `vscode/syntaxes/` (generated — do not hand-edit)      |
| Zed grammar         | `zed/grammars/aivi/src/grammar.json`                   |
| Zed lang config     | `zed/languages/aivi/config.toml`                       |

### 1.4 Terminal Command Tips

- Filter build output: `cargo build --workspace 2>&1 | grep -E "^error|^warning.*-->" | head -40`
- Prefer `--quiet` / `--message-format=short` to reduce noise.
- Avoid printing full build logs; only show warnings/errors.

---

## 2. The Golden Rules

### 2.1 Specs Are the Source of Truth

`specs/` always wins over Rust code.

- Before touching any compiler or stdlib code, read the relevant spec file(s).
- **If Rust code and specs disagree**: the code is wrong. Fix the code to match the spec.
- **If it is genuinely unclear which is correct** (e.g., the spec is ambiguous, the divergence is intentional, or the feature predates the spec): **stop and ask for clarification before proceeding.** Do not silently pick a side.
- If the spec has a gap (describes a feature incompletely), document the gap with a `<!-- TODO: spec gap -->` comment and ask before filling it with assumptions.

### 2.2 No Dead Code After Feature Changes

When replacing a feature with something new (different syntax, different API, renamed construct, etc.):

- **Delete** all Rust code implementing the old feature.
- **Delete** all integration test files and AIVI snippets that used the old form.
- **Delete** the old spec page(s) or sections, not just add a note saying "deprecated".
- **Remove** any grammar rules or token types from the LSP/VSCode/Zed integration that only served the old feature.

The goal: after the change, `git grep` for the old construct returns zero results (except this AGENTS.md if mentioned for documentation purposes).

### 2.3 New Features Must Fully Propagate

Adding a new language or stdlib feature is never complete until all layers are updated. Propagation order:

1. **`specs/`** — write or update the spec page first (see §5).
2. **`crates/aivi_core`** — parser, typechecker, formatter.
3. **`crates/aivi`** — runtime/stdlib implementation, Cranelift lowering if applicable.
4. **`crates/aivi_lsp`** — semantic token types, hover docs, completion items, diagnostics.
5. **`vscode/syntaxes/`** — regenerate via `cargo run -p aivi --bin gen_vscode_syntax -- vscode/syntaxes` (do not hand-edit generated files).
6. **`zed/grammars/aivi/`** — update the Tree-sitter grammar (`grammar.json` / source) so Zed highlights the new syntax correctly; bump `zed/extension.toml` version if needed.
7. **`integration-tests/`** — add at least one positive test and, if the feature has failure modes, a `compile_fail/` test.
8. **`AIVI_LANGUAGE.md`** — update the LLM quick-reference.

Skipping any layer leaves the tooling in an inconsistent state and will break CI.

---

## 3. Development Workflow

### 3.1 Task Execution Protocol

1. **Read specs first**: `specs/syntax/`, `specs/stdlib/`, `specs/typed_codegen/`, `specs/runtime/`.
2. **Locate implementation**: find the Rust source using the table in §1.3.
3. **Check for discrepancies** between spec and code; apply §2.1.
4. **Implement** in small, testable units. Follow propagation order from §2.3.
5. **Verify** (see §6 for commands).

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
- Module path segments and file names: `snake_case` (e.g. `aivi.stdlib.core.text` → `text.aivi`).
- Bindings are immutable; use recursion, folds, or generators instead of loops.
- See `AIVI_LANGUAGE.md` for a concise syntax reference.

### 4.2 Safety

- No nulls — use `Option A` or `Result E A`.
- Exhaustive pattern matches; use `_` only when every remaining case is truly identical.
- Use `resource { ... }` blocks for handles (files, connections) to guarantee cleanup.

### 4.3 Concurrency

- Spawn tasks inside a scope (`concurrent.scope`).
- Communicate via typed channels (`Send`/`Recv`), not shared mutable state.

### 4.4 GTK4 UI — Signal Streams

When writing GTK4 UI code, **always use `signalStream` for event handling**. The old `on Msg => handler` callback style and the `signalBind*` helper family (`signalBindBoolProperty`, `signalBindCssClass`, `signalBindToggleBoolProperty`, `signalToggleCssClass`, `signalBindDialogPresent`, `signalBindStackPage`) are **deprecated** and must not appear in new code.

**Canonical pattern — recursive event loop:**

```aivi
// 1. Build UI, get a single push-based receiver
root    <- buildFromNode myNode
widgets <- fetchMyWidgets   // { saveBtnId: WidgetId, nameInputId: WidgetId, ... }
rx      <- signalStream {}

// 2. Tail-recursive dispatch loop
runLoop = win => state => widgets => rx => do Effect {
  result <- channel.recv rx
  result match
    | Err _ => pure Unit   // channel closed
    | Ok event =>
        event match
          | GtkClicked wid when wid == widgets.saveBtnId => handleSave state
          | GtkInputChanged wid txt when wid == widgets.nameInputId => do Effect {
              runLoop win (state <| { name: txt }) widgets rx
            }
          | _ => runLoop win state widgets rx
}
```

**Rules for agents:**

- Call `signalStream {}` **once** per UI flow; pass the `rx` value down through the loop.
- Re-fetch widget IDs with `widgetById "id"` after any `windowSetChild`/`dialogSetChild` call, since widget tree rebuilds produce new IDs.
- Use `attempt (widgetById "id")` when a widget may not exist yet (e.g., dialog content not yet built).
- For state-driven re-renders (sidebar toggle, tab switch, form validation), update state → `buildFromNode` → `windowSetChild`/`dialogSetChild` → re-fetch IDs → tail-recurse.
- Prefer typed variants (`GtkClicked`, `GtkInputChanged`, `GtkToggled`, etc.) over `GtkUnknownSignal` wherever possible.
- `signalPoll` is available for one-shot reads; `signalStream` is preferred for continuous loops.

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

| Test kind         | Location                        | Command                                              |
|:----------------- |:------------------------------- |:---------------------------------------------------- |
| Rust unit tests   | `#[test]` in source or `tests/` | `cargo test --workspace`                             |
| AIVI integration  | `integration-tests/`            | Exercised by `cargo test --workspace` (runner tests) |
| LSP tests         | `crates/aivi_lsp/src/tests/`    | `cargo test --workspace`                             |
| Snapshot tests    | `crates/aivi/tests/` (insta)    | `cargo test --workspace`                             |
| VSCode unit tests | `vscode/src/test/`              | `cd vscode && pnpm test:unit`                        |
| Fuzz (CI)         | `fuzz/fuzz_targets/`            | `cargo bolero test ...` (see CI workflow)            |

---

## 7. Project Validation (must all pass before concluding)

Run from the repo root in order:

```bash
# 1. Rust: format check
cargo fmt --all -- --check

# 2. Rust: lint
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -E "^error|^warning"

# 3. Rust + AIVI integration tests
cargo test --workspace 2>&1 | tail -20

# 4. VSCode extension (only needed when vscode/ or aivi_lsp/ changed)
cd vscode && pnpm install --frozen-lockfile && pnpm compile && cd ..

# 5. Docs site — dead-link check + build (only needed when specs/ changed)
cd specs && pnpm install --frozen-lockfile && pnpm docs:build 2>&1 | grep -E "dead link|error|Error" && cd ..
```

On test failure, get full output with:

```bash
cargo test --workspace -- --nocapture 2>&1 | grep -A 20 "FAILED\|panicked"
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
- [ ] Make sure *.aivi files in the codebase have no lsp/formatting issues
- [ ] No dead links in `specs/` (`pnpm docs:build` in `specs/` reports none)
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` is fully green (Rust + AIVI tests)
- [ ] VSCode extension compiles (`pnpm compile` in `vscode/`) if LSP or grammar changed
- [ ] Zed grammar updated if new syntax was introduced
- [ ] Aivi builds without warnings (also pre-existing ones).
 