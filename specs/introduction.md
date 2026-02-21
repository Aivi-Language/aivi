# AIVI Language Specification (v0.1)

> Note: **AIVI v0.1** executes via a tree-walking interpreter (CST → HIR → Kernel → native runtime) or compiles to Rust (CST → HIR → Kernel → Rust IR → Rust source).

## Overview

AIVI is a statically typed, purely functional language designed for **high-integrity data pipelines** and **domain-driven design**.

### Core characteristics

**Logic**

* Global type inference
* (Type) Classes (ad-hoc polymorphism)
* Higher-Kinded Types (HKTs)

**Data**

* Immutable by default
* **Open structural records** (row polymorphism)
* Algebraic Data Types (ADTs)

**Control**

* Pattern matching
* **Predicate-driven transformations**
* **Pure generators**
* Fiber-based structured concurrency
* Explicit effect tracking with `Effect E A`
* **Declarative Resource Management**

**Intentional omissions**

* No loops (use recursion, folds, generators)
* No exceptions (use `Result`)
* No `null` / `undefined` (use `Option`)
* No string concatenation (use interpolation)

### Naming

* **Uppercase** identifiers → types and constructors
* **lowercase** identifiers → values and functions


## Normative Principles

> **Bindings are immutable.**
> **Patterns are total by default; use `match` for partial matches.**
> **Predicates are expressions with implicit scope (`.prop`).**
> **Patches describe structure, not mutation (`<|`).**
> **Domains own semantics and domain-defined operators.**
> **Generators model data streams; effects model typed I/O (`Effect E A`).**

## Why AIVI?

AIVI is designed to solve the complexity of modern data-heavy applications by shifting the focus from **how** data is moved to **what** data means. 

### High Integrity by Design
By eliminating `null`, exceptions, and mutable state, AIVI ensures that if a program compiles, it is fundamentally sound. Its exhaustive pattern matching and totality requirements for bindings make "unhandled state" an impossibility at the type level.

### Universal Portability (WASM & WASI)
AIVI is built from the ground up to target **WebAssembly (WASM)**. 
- **Browser**: High-performance client-side logic and server-driven UI frontends via `aivi.ui.ServerHtml`.
- **Server/Edge**: Using **WASI** (WebAssembly System Interface), AIVI runs in highly isolated, secure sandboxes across cloud and edge infrastructure with near-native speed and instant startup.
- **Security**: The WASM capability-based security model naturally complements AIVI's explicit effect tracking.

### The Power of Domains
In AIVI, the language doesn't try to know everything. Instead, it provides **Domains** a mechanism to extend the language's semantics.
- **Semantic Arithmetic**: Operators like `+` and `-` are not restricted to numbers; they are resolved by domains to perform calendar shifts, color blending, or vector math.
- **Syntactic Sugar**: Surface-level syntax can desugar into a small kernel, keeping the core language minimal.
- **Extensibility**: Developers can define their own domains, creating a language that speaks the vocabulary of their specific business area (Finance, IoT, UI) without losing the safety of the AIVI core.


This document defines **AIVI v0.1** as a language where **data shape, transformation, and meaning are explicit, uniform, and statically enforced**.
