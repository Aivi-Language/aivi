# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is AIVI

A purely functional programming language that compiles to native binaries via Rust and Cranelift. Strong static types, explicit effects, first-class GTK4 UI support. Early stage — breaking changes expected.

## Commands

### Rust (primary)
```bash
cargo fmt --all -- --check                              # format check
cargo clippy --workspace --all-targets -- -D warnings  # lint
cargo nextest run --workspace                           # all tests in parallel (preferred)
cargo t                                                 # alias for cargo nextest run --workspace
cargo nextest run --workspace --no-capture 2>&1 | grep -A 20 "FAILED\|panicked"  # debug failures
cargo build --workspace
```

Install nextest once with: `cargo install cargo-nextest --locked`

### Filtering noisy output
```bash
cargo build --workspace 2>&1 | grep -E "^error|^warning.*-->" | head -40
cargo clippy ... 2>&1 | grep -E "^error|^warning"
```

### Single integration test
Integration tests in `integration-tests/` are `.aivi` files run by the Rust test runner — run a specific one by matching the test name in `cargo test`:
```bash
cargo test --workspace <test_name>
```

### VSCode extension (only when `vscode/` or `aivi_lsp/` changed)
```bash
cd vscode && pnpm install --frozen-lockfile && pnpm compile
cargo run -p aivi --bin gen_vscode_syntax -- vscode/syntaxes  # regenerate grammars
```

### Docs site (only when `specs/` changed)
```bash
cd specs && pnpm install --frozen-lockfile && pnpm docs:build
```

### Format AIVI source files
```bash
cargo run -p aivi -- fmt <path>   # or: aivi fmt <path>
```

### Fuzzing
```bash
cargo test -p aivi-fuzz           # property-test mode (stable, CI-style)
# Extended fuzz (requires nightly + cargo-bolero):
cargo bolero test -p aivi-fuzz parser::parser --engine libfuzzer -T 30s
# Targets: parser, frontend, runtime, formatter, lsp_pipeline, type_inference
```

### Perf regression check
```bash
cargo run -p aivi --bin perf -- check --baseline crates/aivi/perf/baseline.json --max-multiplier 2.0
```

## Architecture

### Compilation pipeline
```
specs/  →  crates/aivi_core  →  crates/aivi (runtime/stdlib/CLI)
                              →  crates/aivi_lsp (LSP server)
crates/aivi (gen_vscode_syntax)  →  vscode/syntaxes/  (generated, do not hand-edit)
```

### Key source locations

| Concern | Path |
|---|---|
| Lexer | `crates/aivi/src/lexer.rs` |
| Parser / CST | `crates/aivi/src/syntax.rs` + `cst.rs` |
| Surface AST | `crates/aivi/src/surface/` |
| HIR | `crates/aivi/src/hir/` |
| Kernel IR | `crates/aivi/src/kernel/` |
| Typechecker | `crates/aivi/src/typecheck/` |
| Formatter | `crates/aivi_core/src/formatter/` |
| Stdlib (Rust) | `crates/aivi/src/stdlib/` |
| Cranelift backend | `crates/aivi/src/cranelift_backend/` |
| Runtime interpreter | `crates/aivi/src/runtime/` |
| LSP backend | `crates/aivi_lsp/src/backend/` |

### aivi_core sharing mechanism
`aivi_core` re-uses source files from `crates/aivi/src/` via `#[path = "..."]`. When physically moving files, update both crates.

### Integration test layout
`integration-tests/syntax/`, `runtime/`, `stdlib/`, `complex/`, `compile_fail/` (negative/expected-failure tests)

## The Golden Rules

**Specs are the source of truth.** `specs/` always wins over Rust code. Before touching any compiler or stdlib code, read the relevant spec. If code and specs disagree, the code is wrong. If genuinely unclear, stop and ask rather than silently picking a side.

**No dead code.** When replacing a feature: delete all old Rust code, integration tests, and spec pages. After the change, `git grep` for the old construct should return zero results.

**Do not check if problems are pre-existing** if you you find problems, fix them in
the same session.

**New features must fully propagate** in this order:
1. `specs/` — write/update spec first
2. `crates/aivi_core` — parser, typechecker, formatter
3. `crates/aivi` — runtime/stdlib, Cranelift lowering
4. `crates/aivi_lsp` — semantic tokens, hover, completion, diagnostics
5. `vscode/syntaxes/` — regenerate via `gen_vscode_syntax` binary
6. `integration-tests/` — at least one positive test + `compile_fail/` if applicable
7. `AIVI_LANGUAGE.md` — update LLM quick-reference

**Specs TOC**: `specs/nav.mjs` is the single TOC source of truth. Do NOT hand-edit `specs/index.md` or `specs/README.md` TOC sections (auto-synced by CI).

## AIVI Language Style (for .aivi files)

- `lowerCamelCase` for values/functions; `UpperCamelCase` for types, constructors, domains, classes
- Module path segments and file names: `snake_case`
- Bindings are immutable; use recursion, folds, or generators — no loops
- No nulls — use `Option A` or `Result E A`
- Exhaustive pattern matches
- No semicolons — bindings and block statements are separated by newlines
- Opening `{` always on the same line as the keyword (`do Effect {`, `generate {`, `match`)
- Avoid deeply nested expressions — extract inner logic into named helper functions

### GTK4 UI
Prefer the signal-first GTK runtime for new apps. Signals should drive widget props and structure directly, callbacks should mutate signals or trigger `Event` handles, and `gtkApp` / `signalBind*` helpers should not appear in new code. See `AGENTS.md §4.4`.

### Decorators
Only decorators listed in `specs/syntax/decorators.md` are valid in v0.1.

## Anti-Patterns (Do NOT write these)

| Wrong                  | Why                              | Correct                                             |
|:---------------------- |:-------------------------------- |:--------------------------------------------------- |
| `let x = 1`            | No `let` keyword                 | `x = 1`                                             |
| `def f(x):`            | No `def`, no parens for args     | `f = x => ...`                                      |
| `fn f(x: T) -> R`      | No `fn` keyword                  | `f : T -> R` / `f = x => ...`                       |
| `f :: T -> R`          | Single colon for type signatures | `f : T -> R`                                        |
| `var x = 1; x = 2`     | No mutation, no semicolons!      | `x = 1` (shadow with `x = x + 1` on next line)      |
| `x = 1; y = 2`        | No semicolons — use newlines     | `x = 1` (newline) `y = 2`                           |
| `do Effect` `{` on next line | Opening `{` must be on same line | `do Effect {`                               |
| `null` / `nil`         | No nulls                         | `None` / `Option A`                                 |
| `Just x` / `Nothing`   | AIVI is not Haskell              | `Some x` / `None`                                   |
| `Left e` / `Right x`   | AIVI is not Haskell              | `Err e` / `Ok x`                                    |
| `Some(x)` / `Ok(x)`   | Constructors take no parens      | `Some x` / `Ok x`                                   |
| `throw` / `try/catch`  | No exceptions                    | `fail e` / `attempt` / `or`                         |
| `for x in xs { ... }`  | No loops                         | `xs \|> map f` or `generate { x <- xs; yield f x }` |
| `while cond { ... }`   | No loops                         | Recursion or `loop`/`recurse` in generators         |
| `f(x, y)`              | No parens for function calls     | `f x y`                                             |
| `x.method()`           | No methods, no parens            | `method x` or `x \|> method`                        |
| `List<Int>` / `Option<T>` | No angle-bracket generics     | `List Int` / `Option T`                             |
| `List.map f xs`        | HKT methods are unqualified      | `map f xs` (with `use aivi.logic`)                  |
| `fmap` / `>>=` / `<$>` / `<*>` | Haskell operators       | `map` / `chain` / `map` / `ap`                      |
| `impl Trait for Type`  | Rust syntax                      | `instance Class (Type) = { ... }`                   |
| `newtype Foo = Foo T`  | Haskell syntax                   | `opaque Foo = T`                                    |
| `do { x <- m }`        | Must name the monad              | `do Effect { x <- m }`                              |
| `case x of ...`        | `case` is kernel only            | `x match \| pat => expr`                            |
| `String`               | Type is called `Text`            | `Text`                                              |
| `return x`             | No return statement              | Expression result is implicit; `pure x` in effects  |
| `{ x = 1 }` in records | `=` is binding, not record field | `{ x: 1 }`                                          |
| `a & b`, `a \| b`      | No bitwise operators             | `use aivi.bits; and a b`, `or a b`                  |
| `a << 2`, `a >> 2`     | No shift operators               | `use aivi.bits; shiftLeft 2 a`, `shiftRight 2 a`   |
| `~a` (bitwise not)     | `~` is for sigils only           | `use aivi.bits; complement a`                       |
| `"x" ++ "y"`           | No string concat operator        | `"{x}{y}"`                                          |
| `import X`             | No `import` keyword              | `use module.path`                                   |
| `use Aivi.List`        | Module paths are `snake_case`    | `use aivi.list`                                     |
| Deep nesting / inline lambdas in lambdas | Extract into named helpers | `step1 = ...; step2 = ...; result = step2 (step1 x)` |


## Pre-commit Checklist

- [ ] Read relevant `specs/` file(s) first
- [ ] No spec ↔ code discrepancy (or asked for clarification)
- [ ] No dead code from replaced features
- [ ] New feature propagated to all layers
- [ ] `specs/nav.mjs` updated (not `index.md`)
- [ ] `AIVI_LANGUAGE.md` updated
- [ ] AIVI snippets formatted with `aivi fmt`
- [ ] Integration test(s) added
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo nextest run --workspace` passes
