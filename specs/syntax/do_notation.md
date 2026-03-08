# Generic `do M` Blocks

Most AIVI code uses `do Effect { ... }` for side effects. This page covers the more general form `do M { ... }`, which lets the same block style work for other container-like types such as `Option`, `Result`, and `List`.

A few terms, in plain language:

- a **type constructor** is a type shape that still needs a value type, such as `Option`, `List`, or `Result ConfigError`
- **chain** means “run one step, then pass its successful result into the next step”
- **desugaring** means “the compiler rewrites friendly syntax into simpler core expressions”

Use `do M` when you want the same step-by-step reading style as `do Effect`, but your type is something like `Option`, `Result`, or `List` rather than an effectful computation.

## Start with concrete examples

### `Option`: stop at the first missing step

<<< ../snippets/from_md/syntax/do_notation/block_01.aivi{aivi}


If either lookup returns `None`, the whole block returns `None`. Otherwise the final line produces `Some profile.displayName`.

### `Result`: stop at the first validation error

<<< ../snippets/from_md/syntax/do_notation/block_02.aivi{aivi}


The block reads top to bottom. Each successful step feeds the next one. The first error ends the block and returns that `Err`.

## `do Effect` first, `do M` when you need the generic form

`do Effect { ... }` is the everyday version. It is built for typed side effects and supports extra effect-specific statements such as `or`, resource acquisition, `when`, `unless`, `given`, `loop` / `recurse`, and `on`.

`do M { ... }` is the smaller, reusable core. It works when a type constructor `M` has the class instances needed to chain steps together.

If you have seen the word **monadic** before, this generic chaining pattern is what it refers to: each step can decide what the next step sees.

## Overview

Generic `do M { ... }` reuses the same readable block layout as `do Effect { ... }`, but the meaning comes from type class instances instead of being hard-coded for effects.

### Design principles

1. **`do Effect { ... }` is the everyday form.** Use it for I/O, resources, cancellation, and the other effect-only statements described in [Effects](effects.md).
2. **`generate { ... }` stays separate.** Generators describe pure sequences with `yield`, so they use their own rules.
3. **Generic `do M { ... }` uses the common subset.** It supports `<-` for bind, `=` for pure local names, expression sequencing, and a final expression.
4. **Instances provide the behavior.** The compiler looks up `Chain M` and `Applicative M` to determine how the block sequences work and how an empty block produces `Unit`.

## Syntax

### Grammar extension

The parser rule is already broad enough:

```text
DoBlock := "do" UpperIdent "{" { DoStmt } "}"
```

The parser accepts any `UpperIdent` after `do`. The difference is semantic:

- `do Effect { ... }` gets the effect-specific rules
- every other `do M { ... }` is checked through type class instances

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

Those missing features are effect-specific because they depend on error handling, cancellation, or cleanup semantics that generic `do M` does not assume.

## How the compiler rewrites a `do M` block

**Desugaring** means the compiler rewrites the block into ordinary function calls.

In the examples below, `⟦ ... ⟧` means “the compiler’s rewritten form of ...”.

### Generic `do M { ... }`

A `do M { ... }` block becomes calls to `chain` and `of` from the `Chain M` and `Applicative M` instances in scope.

#### Bind

<<< ../snippets/from_md/syntax/do_notation/bind.aivi{aivi}

desugars to:

```text
chain (λvalue. ⟦do M { body }⟧) ⟦expr⟧
```

Read that as: run `expr`, name its successful result `value`, then continue with the rest of the block.

The required operation comes from `Chain M`:

```text
chain : (A -> M B) -> M A -> M B
```

#### Pure local binding

<<< ../snippets/from_md/syntax/do_notation/pure_let_binding.aivi{aivi}

desugars to:

```text
let value = ⟦expr⟧ in ⟦do M { body }⟧
```

This is just a normal local name. No `chain` call is needed because `expr` is pure.

#### Sequencing an expression statement

<<< ../snippets/from_md/syntax/do_notation/sequencing_expression_statement.aivi{aivi}

desugars to:

```text
chain (λ_. ⟦do M { body }⟧) ⟦expr⟧
```

This means “run `expr`, ignore its successful value, then keep going.”

#### Final expression

<<< ../snippets/from_md/syntax/do_notation/final_expression.aivi{aivi}

desugars to `⟦expr⟧`. The final expression must already have type `M A`.

#### Empty block

<<< ../snippets/from_md/syntax/do_notation/empty_block.aivi{aivi}

desugars to `of Unit`, using `of : A -> M A` from `Applicative M`.

### `do Effect { ... }` as a specialization

`do Effect { ... }` follows the same broad idea, but it has extra rules for effect-specific statements. In practice:

- `chain` for `Effect E` corresponds to `bind : Effect E A -> (A -> Effect E B) -> Effect E B`
- `of` for `Effect E` corresponds to `pure : A -> Effect E A`
- `or`, `when`, `given`, `on`, `loop`, and resource acquisition use the extra rules described in [Effects](effects.md) and [Resources](resources.md)

The compiler recognizes `Effect` specially so those extra statements remain available. All other `do M` blocks use only the generic subset.

## Type checking

Generic `do M` blocks are checked against in-scope type class instances:

- `Chain M` provides `chain` for `<-` binds and expression sequencing
- `Applicative M` provides `of` for empty blocks and value injection
- for `M ≠ Effect`, effect-only statements such as `or`, `when`, `unless`, `given`, `on`, resource binds, and `loop` / `recurse` are rejected

If no suitable instance is available, compilation fails with an instance-resolution error for the target constructor.

## Runtime behavior

`do M { ... }` lowers to nested `chain` calls plus lambdas, with `of Unit` for the empty-block case.

`do Effect { ... }` keeps the additional runtime behavior described in [Effects](effects.md).

## Common uses

| Type         | `do` block use case |
|:------------ |:-------------------- |
| `Option A`   | Stop early when any step returns `None`. |
| `Result E A` | Chain computations that may fail, without using `Effect`. |
| `List A`     | Describe non-deterministic combinations such as cartesian products. |

## When to use which block form

- Use `do Effect { ... }` for I/O, resource management, cancellation-aware work, and the effect-only statements from [Effects](effects.md).
- Use `do M { ... }` when you want the same readable sequencing for `Option`, `Result`, `List`, or another type constructor with the required class instances.
- Use `generate { ... }` when you are building a pure sequence of yielded values.

## References

- Effects: [§ 9](effects.md)
- Resources: [Resources](resources.md)
- Generators: [§ 7](generators.md)
- Type classes: [§ 3.5](types/classes_and_hkts.md)
- Monad hierarchy: [aivi.logic](../stdlib/core/logic.md)
