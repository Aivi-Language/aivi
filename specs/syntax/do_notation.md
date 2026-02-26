# Generic Monadic `do` Blocks

> **Status**: Implemented (v0.1)   `do M { ... }` works for any type constructor with a `Chain` instance (including `Option`, `Result`, and `List`). Blocks are desugared to nested `chain`/lambda calls during HIR lowering. Native codegen is stubbed.  
> **Depends on**: Type classes ([§ 3.5](types/classes_and_hkts.md)), `Monad` hierarchy ([aivi.logic](../stdlib/core/logic.md)), effects ([§ 9](effects.md)), instance resolution (compiler).

## Overview

`do M { ... }` generalizes the existing `do Effect { ... }` block to work with **any type constructor `M` that has a `Monad` instance**. The `<-` and `=` syntax remains identical; the compiler resolves `chain`/`of` from the `Monad` dictionary for `M` instead of hardcoding `Effect` primitives.

### Design Principles

1. **`do Effect { ... }` remains the primary form**   it is the most common and retains its special features (`or` fallback, `resource` acquisition, `when`/`unless`/`given`, `loop`/`recurse`, `on`).
2. **`generate { ... }` stays separate**   generators have fundamentally different semantics (`yield`, guards, pull-based), and are not monadic in the standard sense.
3. **Generic `do M` supports only the common monadic subset**   `<-` (bind), `=` (let), final expression. Effect-specific statements are not available in generic blocks.
4. **Instance-driven**   the compiler uses the existing class/instance resolution to find `Chain M` (for `chain`) and `Applicative M` (for `of`).

## Syntax

### Grammar extension

The existing grammar rule:

```text
DoBlock := "do" UpperIdent "{" { DoStmt } "}"
```

is **unchanged**   the parser already accepts any `UpperIdent` after `do`. The change is semantic: the type checker and desugaring must handle the monad name generically.

### Statement subset by monad

| Statement             | `do Effect` | `do M` (generic) | `generate`          |
|:--------------------- |:-----------:|:----------------:|:-------------------:|
| `x <- expr`           | yes         | yes              | yes (from sequence) |
| `x = expr`            | yes         | yes              | yes                 |
| `expr` (sequencing)   | yes         | yes              | no                  |
| `yield expr`          | no          | no               | yes                 |
| `x -> pred` (guard)   | no          | no               | yes                 |
| `or` fallback         | yes         | **no**           | no                  |
| `when cond <- eff`    | yes         | **no**           | no                  |
| `unless cond <- eff`  | yes         | **no**           | no                  |
| `given cond or expr`  | yes         | **no**           | no                  |
| `on Event => handler` | yes         | **no**           | no                  |
| `loop`/`recurse`      | yes         | **no** (v1)      | yes                 |
| resource `<-`         | yes         | **no**           | no                  |

Rationale: `or`, `when`/`unless`/`given`, `on`, and resource acquisition are tightly coupled to the `Effect E A` type (error handling, cancellation, cleanup). Generic monadic blocks use only the universal monadic operations.

## Desugaring

### Generic `do M { ... }`

A `do M { ... }` block desugars to calls to `chain` and `of` from the `Chain M` and `Applicative M` dictionaries.

#### Bind

<<< ../snippets/from_md/syntax/do_notation/bind.aivi{aivi}

desugars to:

```text
chain (λx. ⟦do M { body }⟧) ⟦expr⟧
```

(using `chain : (A -> M B) -> M A -> M B` from `Chain M`)

#### Pure let-binding

<<< ../snippets/from_md/syntax/do_notation/pure_let_binding.aivi{aivi}

desugars to:

```text
let x = ⟦expr⟧ in ⟦do M { body }⟧
```

#### Sequencing (expression statement)

<<< ../snippets/from_md/syntax/do_notation/sequencing_expression_statement.aivi{aivi}

desugars to:

```text
chain (λ_. ⟦do M { body }⟧) ⟦expr⟧
```

#### Final expression

<<< ../snippets/from_md/syntax/do_notation/final_expression.aivi{aivi}

desugars to `⟦expr⟧`. It must have type `M A`.

#### Empty block

<<< ../snippets/from_md/syntax/do_notation/empty_block.aivi{aivi}

desugars to `of Unit` (using `of : A -> M A` from `Applicative M`).

### `do Effect { ... }` as a specialization

`do Effect { ... }` becomes sugar for `do (Effect E) { ... }` **plus** the effect-specific extensions (fallback, guards, resources, etc). In terms of desugaring:

- `chain` for `Effect E` is `bind : Effect E A -> (A -> Effect E B) -> Effect E B`
- `of` for `Effect E` is `pure : A -> Effect E A`
- The additional statements (`or`, `when`, `given`, `on`, `loop`, resource `<-`) are desugared as specified in [Effects § 9](effects.md).

The compiler detects `do Effect` specifically (by name) to enable the extended statement set. All other `do M` blocks get the generic subset only.

## Type Checking

Generic `do M` blocks are checked against in-scope type class instances:

- `Chain M` provides `chain` for `<-` binds and sequencing.
- `Applicative M` provides `of` for empty blocks / unit returns.
- For `M ≠ Effect`, effect-only statements (`or`, `when`, `unless`, `given`, `on`, resource binds, `loop`/`recurse`) are rejected.

When no suitable instance is found, compilation fails with an instance-resolution diagnostic for the target monad constructor.

## Runtime Behavior

`do M { ... }` lowers to nested `chain`/lambda calls, with `of` for empty blocks.
`do Effect { ... }` keeps the effect-specific statement set and runtime machinery described in [Effects § 9](effects.md).

## Common v0.1 Uses

| Type         | `do` block use case                                  |
|:------------ |:---------------------------------------------------- |
| `Option A`   | Short-circuit chaining when any step returns `None`. |
| `Result E A` | Pure error chaining without effects.                 |
| `List A`     | Non-deterministic computation / cartesian products.  |

## References

- Effects: [§ 9](effects.md)
- Generators: [§ 7](generators.md)
- Type classes: [§ 3.5](types/classes_and_hkts.md)
- Monad hierarchy: [aivi.logic](../stdlib/core/logic.md)
