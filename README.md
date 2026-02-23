# AIVI

AIVI is a functional programming language and toolchain for building strongly typed applications on a Rust-powered foundation.
It is designed for developers who want high confidence in correctness, explicit effect handling, and fast execution, without writing Rust as their day-to-day language.

## What makes AIVI special

AIVI sits in a focused niche: **domain-first functional programming with systems-level runtime credibility**.

- **High-level language, low-level foundation**  
  Model business logic in a concise, expressive language while relying on a Rust implementation for speed and reliability.
- **Correctness without ownership overhead**  
  Keep strong typing and compositional modeling without carrying Rust's full ownership/borrowing mental load in application code.
- **Explicit effects and errors**  
  Build effectful programs where side effects and failures stay visible in program design rather than hidden in ad hoc conventions.
- **Tooling as a first-class feature**  
  CLI, language server support, and workspace-oriented architecture are part of the core platform, not afterthoughts.
- **Built to scale from model to runtime**  
  The language pipeline and runtime path are designed together, so high-level abstractions remain practical in real systems.

## Where AIVI fits in the language landscape

AIVI is not trying to replace every language category.
It is meant for teams that want stronger guarantees than dynamic ecosystems, a more domain-centric developer experience than systems languages, and more runtime pragmatism than purely academic functional stacks.

| Language space | Typical strength | Common trade-off | Where AIVI fits |
| --- | --- | --- | --- |
| Dynamic scripting (Python/JS) | Fast iteration, huge ecosystems | Weaker static guarantees | Offers stronger type-driven modeling and explicit effects |
| Systems languages (Rust/C++) | Performance, control, predictability | Higher cognitive load for application logic | Keeps Rust-backed performance with a higher-level language surface |
| Mainstream typed app languages (TypeScript/Kotlin/Go) | Practical productivity and tooling | Less algebraic/domain-centric modeling in many codebases | Emphasizes functional composition and domain precision |
| Pure FP ecosystems (Haskell/OCaml/F#) | Powerful type systems and abstractions | Ecosystem/runtime integration varies by context | Brings FP modeling style with a Rust-native implementation path |

## When to choose AIVI

AIVI is a strong fit when you are building:

- Domain-heavy services where correctness matters (finance, compliance, workflow engines, policy systems).
- Type-safe core business logic that should remain easy to reason about as complexity grows.
- Effectful applications where you want explicit control over side effects and error propagation.
- Rust-adjacent platforms where runtime performance and operational reliability are key requirements.

## Architecture at a glance

AIVI is organized as a Rust workspace with language, runtime, and tooling crates:

- `crates/aivi`: CLI and developer entry point
- `crates/aivi_core`: core language pipeline
- `crates/aivi_driver`: compilation orchestration
- `crates/aivi_native_runtime`: native runtime support
- `crates/aivi_http_server`: HTTP serving integration
- `crates/aivi_lsp`: language server support
- `crates/doc_index_gen`: documentation indexing utilities

High-level flow:

1. Lexing and parsing
2. AST/HIR-style lowering and resolution
3. Desugaring to a compact core representation
4. Type inference and checking
5. Runtime execution and/or backend emission

## Project direction

AIVI's direction is clear: **make high-assurance, typed application development feel expressive and practical**, while preserving the reliability and performance profile enabled by a Rust foundation.
