# AIVI

AIVI is a functional programming language and toolchain for building typed applications with a simpler developer experience than systems-level languages. It is designed to let you model data, business logic, and effects in a high-level language while still compiling and running on a fast Rust-powered foundation.

## Why AIVI exists

AIVI exists to make strong correctness guarantees practical without forcing developers to carry the full cognitive load of Rust syntax, ownership, borrowing, and memory management. The goal is to keep the benefits of modern static typing and performance while offering a language that is focused on expression, composition, and domain modeling.

In short: use the Rust ecosystem for speed and reliability, but work in a language surface that is optimized for product thinking rather than low-level mechanics.

## Core use cases

AIVI is a good fit when you want:

- **Type-safe application logic** with inference and algebraic data types.
- **Domain-heavy systems** where correctness and explicit modeling matter.
- **Effectful programs** that still keep error and side-effect handling explicit.
- **Toolable workflows** with CLI and editor integration (including LSP).
- **A Rust-backed runtime path** without writing Rust as the primary language.

## Architecture overview

AIVI is organized as a Rust workspace with language, runtime, and tooling crates:

- `crates/aivi`: main CLI and developer entry point
- `crates/aivi_core`: core language pipeline and shared compiler logic
- `crates/aivi_driver`: orchestration layer for compilation tasks
- `crates/aivi_native_runtime`: native runtime support
- `crates/aivi_http_server`: HTTP serving integration
- `crates/aivi_lsp`: language server for editor features
- `crates/doc_index_gen`: documentation indexing utilities

At a high level, source code flows through:

1. Lexing and parsing
2. AST/HIR-style lowering and resolution
3. Desugaring to a compact core representation
4. Type inference and checking
5. Runtime execution and/or backend emission paths

This separation keeps the language model clean while enabling multiple tooling and runtime surfaces.

## Using the Rust ecosystem without Rust overhead

AIVI is built in Rust and integrates naturally with Rust-native infrastructure, but AIVI users work mostly at the language level. That means you can benefit from Rust's performance and ecosystem maturity while avoiding day-to-day ownership bookkeeping and memory-management details in application code.

The project direction is to make this bridge explicit: a high-level functional language experience on top of a robust Rust implementation.
