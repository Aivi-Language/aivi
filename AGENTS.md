---
apply: always
---

# AIVI Agent Guide

This document defines the operating contract for AI agents contributing to the AIVI language project. It specifies authority boundaries, dependency flow, implementation rules, and validation requirements.

This file is normative for agent behavior.

---

# 1. Authority & Source of Truth

## 1.1 The Hierarchy

1. `specs/` — **Authoritative Source of Truth**
2. `integration-tests/` — Canonical language examples
3. `crates/` — Rust implementation
4. `vscode/` — Tooling built on top of crates

If implementation contradicts specs:
- The implementation is wrong.
- Or the specs must be updated first.

Agents must never silently diverge from `specs/`.

---

# 2. Repository Structure

| Directory | Responsibility | Notes |
|------------|---------------|--------|
| `specs/` | Language definition (syntax, kernel, desugaring, stdlib) | Must be updated alongside design changes |
| `crates/` | Compiler, runtime, LSP | Must faithfully implement specs |
| `integration-tests/` | Canonical AIVI programs | Used for validation + documentation |
| `vscode/` | Editor extension | Depends on `aivi_lsp` + grammar |

---

## 2.1 Dependency Flow

```mermaid
graph TD
    Specs[specs/] -->|Defines| Crates[crates/]
    Specs -->|Defines| Tests[integration-tests/]
    Crates -->|Builds| Binary[aivi binary]
    Binary -->|Powers| VSCode[vscode/]
    Tests -->|Validates| Binary
````

Key invariant:

> Specs define behavior.
> Tests validate behavior.
> Crates implement behavior.
> VSCode consumes behavior.

---

# 3. Golden Rule

## Never invent syntax, semantics, or stdlib features.

Before:

* Writing AIVI code
* Modifying parser or type system
* Generating integration examples
* Extending stdlib

You must verify the relevant `specs/` section.

If missing:

* Stop.
* Ask for clarification.
* Or update specs first.

---

# 4. Task Execution Protocol

## Step 1 — Analyze Specs

Identify the correct layer:

| Concern          | Spec Location          |
| ---------------- | ---------------------- |
| Syntax           | `specs/02_syntax/`     |
| Kernel semantics | `specs/03_kernel/`     |
| Desugaring       | `specs/04_desugaring/` |
| Stdlib           | `specs/05_stdlib/`     |

Do not rely on memory. Always check files.

---

## Step 2 — Locate Implementation

Common crate locations:

| Concern     | Crate Path                            |
| ----------- | ------------------------------------- |
| Parser      | `crates/aivi/src/parser`              |
| CST/AST     | `crates/aivi/src/syntax` (or similar) |
| Type system | `crates/aivi/src/ty`                  |
| LSP         | `crates/aivi_lsp`                     |

---

## Step 3 — Implement

* Make minimal, isolated changes.
* Preserve architecture boundaries.
* Avoid speculative refactors.
* Do not mix unrelated improvements unless trivial.

---

## Step 4 — Verify

From repository root:

```bash
cargo test --workspace
cargo build --workspace
```

Must complete without errors and warnings.

If relevant:

```bash
cd vscode && pnpm install && pnpm build
cd specs && pnpm install && pnpm docs:build
```

Integration tests must compile and pass.

---

# 5. Codebase Hygiene Rules

## 5.1 Clean as You Go

When touching a file:

* Fix spec violations immediately.
* Remove invalid syntax.
* Correct outdated AIVI examples.
* Add missing tests for discovered edge cases.
* Refactor large files only if safe and clearly beneficial.

Do not leave known inconsistencies behind.

---

## 5.2 Terminal Discipline

When running commands:

* Suppress noisy logs (`--quiet`, avoid verbose flags).
* Filter warnings like:

```
warning: *
--> crates/aivi/*.rs:*:*
```

* Avoid pasting massive build output.

---

# 6. AIVI Language Conventions

When writing AIVI code (tests or docs):

## 6.1 Naming

* `lowerCamelCase` — values, functions
* `UpperCamelCase` — types, modules

## 6.2 Language Principles

* Bindings are immutable
* Prefer recursion/generators over loops
* Use pattern matching idiomatically
* Use record patching where appropriate
* Use domain units when available
* Prefer stdlib modules over ad-hoc solutions

---

## 6.3 Safety Requirements

* No nulls — use `Option` or `Result`
* Exhaustive pattern matching
* Typed errors (`Result E A`)
* Resource management via `resource { ... }` blocks

Never introduce unsafe or partial constructs unless specified in specs.

---

# 7. Compiler Implementation Rules (Rust)

## 7.1 Parser

* Must be error tolerant (LSP requirement)
* Recover from syntax errors
* Preserve spans accurately

## 7.2 CST vs AST

* CST must preserve whitespace/comments
* AST represents semantic structure
* Do not collapse fidelity layers

---

## 7.3 Diagnostics

Diagnostics must include:

* Span
* Clear message
* Actionable help text (when possible)

Avoid vague errors.

---

## 7.4 Testing Strategy

Use:

* Unit tests — isolated behavior
* Snapshot tests — parser/codegen
* Integration tests — compile real AIVI programs

Never merge behavior without coverage.

---

# 8. Standard Library Policy

When implementing stdlib features:

* Prefer battle-tested Rust crates.
* Never reimplement complex algorithms unnecessarily.
* Validate that specs define required primitives first.

Example:

* Use `rustfft` for FFT.
* Do not implement cryptography manually.

---

# 9. Documentation Discipline

## 9.1 Specs First

Design changes require:

* Updating specs
* Updating TOC/menu
* Verifying links
* Keeping structure coherent

---

## 9.2 AIVI Code Blocks

All AIVI snippets must:

* Be formatted with `aivi fmt`
* Use fenced code blocks with `aivi` tag
* Match compiler formatting output

---

## 9.3 LSP Quick Info Markers

Use:

```
<!-- quick-info: {JSON} -->
...
<!-- /quick-info -->
```

Wrap existing prose.
Do not duplicate documentation for tooling.

---

## 9.4 Updating AGENTS.md

Update this file when:

* Workflow changes
* Repo structure changes
* Validation rules change
* Architectural decisions change

---

# 10. Pre-Submission Checklist

Before finalizing changes:

* [ ] Checked relevant specs
* [ ] No invented syntax/features
* [ ] Tests pass (`cargo test --workspace`)
* [ ] `cargo fmt --all -- --check` passes
* [ ] AIVI snippets formatted with `aivi fmt`
* [ ] Integration tests updated if needed
* [ ] Docs updated if design changed
* [ ] Links verified
* [ ] No unnecessary log noise introduced

---

# 11. Core Invariants

1. Specs define behavior.
2. Implementation follows specs.
3. Tests enforce specs.
4. Tooling depends on compiler.
5. Agents do not guess language features.
6. Clean up problems when found.
7. Keep the system small, precise, and coherent.

Violating these principles produces long-term instability.

This guide is binding for all AI agents operating in this repository.

```
