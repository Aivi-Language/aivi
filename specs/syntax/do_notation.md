# Generic Monadic `do` Blocks

Most everyday AIVI code uses `do Effect { ... }` for side effects. This page covers the more general form: `do M { ... }`, which works with any type constructor `M` that has the right type class instances.

If you do not spend much time with FP terminology, you can read “monadic” here as “a type that supports chaining one step into the next”. `Option`, `Result`, and `List` are common examples.

## Overview

`do M { ... }` reuses the convenient block syntax of `do Effect { ... }`, but the compiler resolves `chain` and `of` from the `Monad`-style type class instances for `M` instead of hardcoding effect behavior.

### Design Principles

1. **`do Effect { ... }` is the everyday form.** It supports the extra statements that make sense for typed effects, such as `or` fallback, resource acquisition, `when` / `unless`, `given`, `loop` / `recurse`, and `on`.
2. **`generate { ... }` stays separate.** Generators have pull-based sequence semantics and `yield`; they are not described by the same surface rules.
3. **Generic `do M` uses the common subset.** It supports `<-` for binding, `=` for pure local names, expression sequencing, and a final expression.
4. **Instances drive the meaning.** The compiler finds `Chain M` and `Applicative M` instances to determine how the block chains computations and produces values.

## Syntax

### Grammar extension

The parser rule is already broad enough:

```text
DoBlock := "do" UpperIdent "{" { DoStmt } "}"
```

The parser accepts any `UpperIdent` after `do`. The difference is semantic: the type checker and desugaring treat `Effect` specially and treat every other `M` through instances.

### Statement subset by block kind

| Statement             | `do Effect` | `do M` (generic) | `generate`          |
|:--------------------- |:-----------:|:----------------:|:-------------------:|
| `x <- expr`           | yes         | yes              | yes (from sequence) |
| `x = expr`            | yes         | yes              | yes                 |
| `expr` (sequencing)   | yes         | yes              | no                  |
| `yield expr`          | no          | no               | yes                 |
| `x -> pred` (guard)   | no          | no               | yes                 |
| `or` fallback         | yes         | no               | no                  |
| `when cond <- eff`    | yes         | no               | no                  |
| `unless cond <- eff`  | yes         | no               | no                  |
| `given cond or expr`  | yes         | no               | no                  |
| `on Event => handler` | yes         | no               | no                  |
| `loop`/`recurse`      | yes         | no               | yes                 |
| resource `<-`         | yes         | no               | no                  |

Those missing features are effect-specific because they depend on typed errors, cancellation, or cleanup.

## Desugaring

### Generic `do M { ... }`

A `do M { ... }` block desugars to calls to `chain` and `of` from the `Chain M` and `Applicative M` dictionaries.

#### Bind

<<< ../snippets/from_md/syntax/do_notation/bind.aivi{aivi}

desugars to:

```text
chain (λx. ⟦do M { body }⟧) ⟦expr⟧
```

using `chain : (A -> M B) -> M A -> M B` from `Chain M`.

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

desugars to `of Unit` using `of : A -> M A` from `Applicative M`.

### `do Effect { ... }` as a specialization

`do Effect { ... }` follows the same overall idea, but with the effect-specific extensions described in [Effects § 9](effects.md).

In desugaring terms:

- `chain` for `Effect E` is `bind : Effect E A -> (A -> Effect E B) -> Effect E B`
- `of` for `Effect E` is `pure : A -> Effect E A`
- statements such as `or`, `when`, `given`, `on`, `loop`, and resource acquisition use the extra rules from the effects and resources specifications

The compiler recognizes `do Effect` specifically to enable that extended statement set. All other `do M` blocks use the generic subset only.

## Type Checking

Generic `do M` blocks are checked against in-scope type class instances:

- `Chain M` provides `chain` for `<-` binds and sequencing
- `Applicative M` provides `of` for empty blocks and value injection
- for `M ≠ Effect`, effect-only statements (`or`, `when`, `unless`, `given`, `on`, resource binds, `loop` / `recurse`) are rejected

When no suitable instance is found, compilation fails with an instance-resolution diagnostic for the target constructor.

## Runtime Behavior

`do M { ... }` lowers to nested `chain` / lambda calls, with `of` for empty blocks.

`do Effect { ... }` keeps the effect-specific runtime machinery described in [Effects § 9](effects.md).

## Common Uses

| Type         | `do` block use case |
|:------------ |:-------------------- |
| `Option A`   | Stop early when any step returns `None`. |
| `Result E A` | Chain computations that may fail, without using effects. |
| `List A`     | Describe non-deterministic combinations such as cartesian products. |

## References

- Effects: [§ 9](effects.md)
- Generators: [§ 7](generators.md)
- Type classes: [§ 3.5](types/classes_and_hkts.md)
- Monad hierarchy: [aivi.logic](../stdlib/core/logic.md)
