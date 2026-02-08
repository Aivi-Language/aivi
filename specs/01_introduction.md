# AIVI Language Specification (v0.1)

---

## 0. Overview

AIVI is a statically typed, purely functional language designed for **high-integrity data pipelines** and **domain-driven design**.

### Core characteristics

**Logic**

* Global type inference
* Classes (ad-hoc polymorphism)
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
* Explicit effect tracking

**Intentional omissions**

* No loops (use recursion, folds, generators)
* No exceptions (use `Result`)
* No `null` / `undefined` (use `Option`)
* No string concatenation (use interpolation)

### Naming

* **Uppercase** identifiers → types and constructors
* **lowercase** identifiers → values and functions

---

## 11. Modules

```aivi
module aivi/app/main = {
  export main
  use aivi/std/core
}
```

---

## Normative Principles

> **Bindings are immutable.**
> **Patterns may bind structure and identity simultaneously.**
> **Predicates are expressions with implicit scope.**
> **Patches describe structure, not mutation.**
> **Domains own semantics.**
> **Generators model data; effects model reality.**

---

This document defines **AIVI v0.1** as a language where **data shape, transformation, and meaning are explicit, uniform, and statically enforced**.
