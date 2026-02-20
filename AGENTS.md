---
apply: always
---

# AIVI Agent Guide

This document serves as the authoritative guide for AI agents working on the AIVI language project. It establishes the relationships between specifications, implementation crates, integration tests, and tooling, ensuring safe and consistent contributions.

## 1. Project Structure & Relationships

The AIVI repository is organized into distinct layers. Understanding these relationships is crucial for making correct changes.

### 1.1 Directory Map

| Directory | Role | Relationship |
| :--- | :--- | :--- |
| `specs/` | **Source of Truth** | All implementation logic MUST derive from here (see [specs/index.md](file:///home/mendrik/desk/mendrik/aivi/specs/index.md)). If code contradicts specs, the code is wrong (or specs need updating first). |
| `crates/` | **Implementation** | The Rust codebase implementing the compiler, LSP, and runtime. |
| `integration-tests/` | **Validation** | Canonical AIVI code validating syntax + stdlib. Used for integration testing and documentation. |
| `vscode/` | **Tooling** | Editor extension. Depends on `crates/aivi_lsp` and `specs/02_syntax` (grammar). |

### 1.2 Dependency Flow

```mermaid
graph TD
    Specs[specs/] -->|Defines| Crates[crates/]
    Specs -->|Defines| Tests[integration-tests/]
    Crates -->|Builds| Binary[aivi binary]
    Binary -->|Powers| VSCode[vscode/]
    Tests -->|Tests| Binary
```

### 1.3 Running terminal commands
- make sure to filter (ie. grep) output by warnings and errors:
    warning: *
    --> crates/aivi/*.rs:*:*
- Use shorter logs (--quiet, --no-verbose)    
- Avoid massive build outputs

## 2. Development Workflow

### 2.1 The Golden Rule
**Never invent syntax or features.**
Always verify against `specs/` before writing AIVI code or compiler logic. If a feature is missing from specs (e.g., [`specs/02_syntax/`](file:///home/mendrik/desk/mendrik/aivi/specs/02_syntax/)), ask for clarification.

### 2.2 Task Execution Protocol

1.  **Analyze**: Read the relevant `specs/` files first.
    *   *Syntax*: [`specs/02_syntax/`](file:///home/mendrik/desk/mendrik/aivi/specs/02_syntax/)
    *   *Semantics*: [`specs/03_kernel/`](file:///home/mendrik/desk/mendrik/aivi/specs/03_kernel/) & [`specs/04_desugaring/`](file:///home/mendrik/desk/mendrik/aivi/specs/04_desugaring/)
    *   *Stdlib*: [`specs/05_stdlib/`](file:///home/mendrik/desk/mendrik/aivi/specs/05_stdlib/)
2.  **Locate**: Identify the corresponding Rust crates in `crates/`.
    *   *Parsing*: `crates/aivi/src/parser` (or similar)
    *   *Typing*: `crates/aivi/src/ty`
    *   *LSP*: `crates/aivi_lsp`
3.  **Implement**: Make changes in small, testable units.
4.  **Verify**:
    *   Run `cargo test` in `crates/`.
    *   Check if `integration-tests/` still compile/run (if applicable).
    *   Ensure `specs/` are updated if the change involves a design decision.

### 2.3 Clean as You Cook
Maintain hygiene in the codebase and documentation.
*   **Syntax Correction**: If you see syntax that violates the specs (e.g., `let x =` instead of `x =`, or `def foo()`), fix it immediately to match [`specs/02_syntax`](file:///home/mendrik/desk/mendrik/aivi/specs/02_syntax/).
*   **Gap Filling**: If you encounter code using features not present in `specs/` or `integration-tests/`, document them or add a test case.
*   **Refactoring**: Keep files small and readable. Propose splitting large files into logical units with good naming and subfolder structure if needed.
*   **Pre-existing issues** when you encounter pre-existing problems, fix them up as part of the current task.

## 3. AIVI Language Best Practices

When writing or generating AIVI code (e.g., in `integration-tests/` or tests), adhere to these principles derived from the specs.

### 3.1 Style & Syntax
*   **Identifiers**: `lowerCamelCase` for values/functions, `UpperCamelCase` for types/modules.
*   **Immutability**: Bindings are immutable. Use recursion or generators instead of loops.
*   **Destilled language syntax** at ./AIVI_LANGUAGE.md
*   **When writing AIVI code**: make sure to pick the right tools: sigils, generators, pattern matching, record patching, domain units are all available, stdlib modules and so on.

### 3.2 Safety & Robustness
*   **No Nulls**: Always use `Option` or `Result`.
*   **Exhaustive Matching**: Ensure `case` or `?` covers all patterns. Use `_` only when necessary.
*   **Typed Errors**: Use `Result E A` with specific error types, not generic strings.
*   **Resources**: Use `resource { ... }` blocks for file/network handles to ensure cleanup (see [specs/02_syntax/15_resources.md](file:///home/mendrik/desk/mendrik/aivi/specs/02_syntax/15_resources.md)).

### 3.3 Concurrency
*   **Structured**: Always spawn tasks within a scope (`concurrent.scope`).
*   **Communication**: Use channels (`Send`/`Recv`) for data exchange, not shared memory (see [specs/06_runtime/01_concurrency.md](file:///home/mendrik/desk/mendrik/aivi/specs/06_runtime/01_concurrency.md)).

### 3.4 Decorators

This file applies to all of `specs/`.

- Decorators are reserved for **compiler/tooling pragmas** only (compile-time metadata).
- Do **not** add new integration-specific decorators (examples of forbidden patterns: `@sql`, `@schema`, `@table`, `@model`).
- Prefer **typed values** and **type-driven decoding/validation** at boundaries (e.g. `Source` configuration records, decoders derived from types).
- Only the decorators enumerated in [`specs/02_syntax/14_decorators.md`](file:///home/mendrik/desk/mendrik/aivi/specs/02_syntax/14_decorators.md) are allowed in v0.1; unknown decorators should be considered a spec violation.

## 4. Rust Implementation Guidelines

When working on the compiler (`crates/`):

*   **Parser**: Must be error-tolerant (recover from syntax errors) to support LSP.
*   **CST vs AST**: Preserve full fidelity (whitespace/comments) in CST for formatting/refactoring.
*   **Diagnostics**: Emit rich error messages with spans and help text.
*   **Testing**:
    *   *Unit Tests*: For individual functions.
    *   *Snapshot Tests*: For parser/codegen output (use `insta` or similar if available).
    *   *Integration Tests*: Compile and run files from `integration-tests/`.

### 4.1 Standard Library & Dependencies
*   **Rely on Battle-Tested Libraries**: When implementing standard library features, always prioritize established Rust crates.
*   **Avoid Reinventing the Wheel**: Do not implement complex algorithms (like FFT, crypto, etc.) from scratch if a robust Rust solution exists.
    *   *Example*: Use `rustfft` instead of implementing your own FFT.
*   Before implementing a new standard library feature, check that it has all the necessary primitives in the documentation. We want to keep it simple but not too limited.

## 5. Documentation Maintenance

*   **Specs**: Update `specs/` *before* or *alongside* code changes.
*   **Specs**: Update indices and make sure links are working.
*   **AIVI Code Blocks**: Format all AIVI snippets in docs to match `aivi fmt` output (use `aivi fmt path` or `cargo run -p aivi -- fmt path`). In particular, keep formatter-style alignment for `<-` inside `effect { ... }` blocks and `=>` in pattern matching arms. Prefer fenced code blocks with the `aivi` language tag.
*   **Quick Info Markers**: For LSP hover/quick-info, wrap existing spec markdown with `<!-- quick-info: {JSON} --> ... <!-- /quick-info -->` (see [specs/doc-markers-spec.md](file:///home/mendrik/desk/mendrik/aivi/specs/doc-markers-spec.md)). Do not duplicate prose just to satisfy tooling.
*   **Guidelines**: Follow `.junie/guidelines.md`.
*   **AGENTS.md**: Update this file if workflows or structures change significantly.

## 6. Project Validation

To validate the project end-to-end, run these from the repo root:

1.  **All tests** (workspace): `cargo test --workspace`
2.  **Build all crates**: `cargo build --workspace`
3.  **Build VSCode extension**: `cd vscode && pnpm install && pnpm build`
4.  **Build docs site**: `cd specs && pnpm install && pnpm docs:build`
5.  On test failure run `cargo test -vv -- --nocapture --show-output` to see the output.

## 7. Safety Checklist

Before submitting changes:
- [ ] Did I check the specs?
- [ ] Did I run existing tests?
- [ ] Did I run `cargo fmt --all -- --check`?
- [ ] Did I run `aivi fmt` for any AIVI doc snippets I touched?
- [ ] Did I add a new test case?
- [ ] Is the code consistent with AIVI style?
- [ ] Did I avoid hallucinating features? (Always check [`specs/`](file:///home/mendrik/desk/mendrik/aivi/specs/) first)
- [ ] Did I update AIVI_LANGUAGE.md for LLM use?

