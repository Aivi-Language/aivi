# Missing Features (v0.1 Gap Analysis)

This document lists features, modules, and behaviors described in the **AIVI Language Specification** vs. the **v0.1 Rust Implementation**.

> **Note to Users:** AIVI v0.1 is primarily an interpreted language embedding a CST-to-Kernel pipeline.
> Native Rust code generation exists as an **experimental** backend that emits standalone Rust logic for a limited subset of AIVI
> (see **Native Codegen** below). The interpreter remains the most complete backend today.

## 1. Syntax & Language Features

| Feature | Spec Section | Implementation Status | Notes |
| :--- | :--- | :--- | :--- |
| **Generators** | `02_syntax/07_generators` | **Implemented** | Desugared to Church-encoded lambdas in `kernel.rs`. `generate` blocks supported. `loop`/`recurse` inside generators partial. |
| **Decorators** | `02_syntax/14_decorators` | **Implemented (Syntax)** | Parsed and validated; only standard decorators allowed. |
| **User-defined Domains** | `02_syntax/06_domains` | **Implemented** | `DomainDecl` exists in CST/HIR. |
| **Patching** | `02_syntax/05_patching` | **Implemented** | `Patch` alias exists; desugaring logic present. |

## 2. Type System

| Feature | Spec Section | Status | Notes |
| :--- | :--- | :--- | :--- |
| **Higher-Kinded Types** | `02_syntax/03_types` | **Structurally Implemented** | `Kind` enum (Star/Arrow) and builtins (`List: *->*`, `Effect: *->*->*`) exist in `checker.rs`. Complex inference scenarios may vary. |
| **Row Polymorphism** | `02_syntax/03_types` | **Implemented** | Open records and row extension/restriction logic present in `checker.rs`. |
| **Effect Typing** | `02_syntax/09_effects` | **Implemented** | `Effect E A` is a first-class type; `attempt`/`pure`/`fail` are built-ins. |

## 3. Standard Library Status

| Module | Status | Backend |
| :--- | :--- | :--- |
| `aivi.regex` | **Implemented** | Backed by `runtime/builtins/regex.rs`. |
| `aivi.net.http` | **Implemented** | Backed by `runtime/url_http.rs`. |
| `aivi.net.server` | **Implemented** | Backed by `runtime/http.rs` (using `aivi_http_server`). |
| `aivi.db` | **Partial** | `database.rs` exists in stdlib/runtime, likely SQLite wrapper. |
| `aivi.math` | **Implemented** | Extensive `math.rs` in stdlib. |

## 4. Tooling & Execution

| Component | Status | Notes |
| :--- | :--- | :--- |
| **Native Codegen** | **Experimental (Partial)** | `aivi build` can emit standalone Rust logic via `[build].codegen = "native"`. Current limitations include incomplete builtins/stdlib coverage and missing `match` support in the native backend. |
| **Package Manager** | **Implemented (Minimal)** | Cargo-backed `search`/`install` plus `package`/`publish` wrappers. Dependency installs validate `[package.metadata.aivi]` and enforce `kind = "lib"`; publishing validates `aivi.toml` ↔ `Cargo.toml` metadata consistency. |
| **LSP** | **Implemented** | `aivi_lsp` crate exists with diagnostics, formatting, and definition lookup. |

---

## Walkthrough: The v0.1 Reality

If you are using AIVI v0.1 today, you are using a **high-integrity interpreter**.

1.  **The Code is the Truth**: The `crates/aivi/src/` directory contains the definition of the language.
    *   `syntax.rs` / `cst.rs` define what you can write.
    *   `checker.rs` defines a surprisingly capable type system (HKTs, Classes, Rows).
    *   `runtime/` implements the "magical" effects and IO.

2.  **Performance**:
    *   Code is lowered to a Kernel IR and interpreted.
    *   It is fast enough for scripting, servers (via Tokio integration), and tooling.
    *   It is **not yet** generating optimized WASM for high-performance compute, though the type system allows expressing it.

3.  **The "Rust" Target**:
    *   When you run `aivi build`, you get a Rust binary (via `cargo`).
    *   With `codegen = "embed"`, the binary contains your HIR program as JSON and runs it via the interpreter runtime.
    *   With `codegen = "native"`, the generated Rust corresponds to the AIVI logic directly (experimental; partial coverage).
