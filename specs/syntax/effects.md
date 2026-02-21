# Effects

## 9.1 The `Effect E A` Type

Effectful operations in AIVI are modeled using the `Effect E A` type, where:
- `E` is the **error domain** (describing what could go wrong).
- `A` is the **successful return value**.

### Semantics
- **Atomic Progress**: Effects are either successfully completed, failed with `E`, or **cancelled**.
- **Cancellation**: Cancellation is an asynchronous signal that stops the execution of an effect. When cancelled, the effect is guaranteed to run all registered cleanup (see [Resources](resources.md)).
- **Transparent Errors**: Errors in `E` are part of the type signature, forcing explicit handling or propagation.

### Core operations (surface names)

Effect sequencing is expressed via `do Effect { ... }` blocks, but the underlying interface is:

- `pure : A -> Effect E A` (return a value)
- `bind : Effect E A -> (A -> Effect E B) -> Effect E B` (sequence)
- `fail : E -> Effect E A` (abort with an error)

For *handling* an effect error as a value, the standard library provides:

- `attempt : Effect E A -> Effect F (Result E A)`

`attempt` runs the inner effect and captures its outcome (success or failure with `E`) as a `Result E A`. The outer effect uses a *different* error type `F`, since the original error `E` has been caught and is now represented as data inside the `Result`. If `F` is unconstrained, the outer effect cannot fail (equivalent to `Effect Never (Result E A)` in practice).

### Examples (core operations)

`pure` lifts a value into an effect:

<<< ../snippets/from_md/syntax/effects/examples_core_operations_01.aivi{aivi}

`bind` sequences effects explicitly (the `do Effect { ... }` block desugars to `bind`):

<<< ../snippets/from_md/syntax/effects/examples_core_operations_02.aivi{aivi}

`fail` aborts an effect with an error value:

<<< ../snippets/from_md/syntax/effects/examples_core_operations_03.aivi{aivi}

`attempt` runs an effect and captures success/failure as a `Result`:

<<< ../snippets/from_md/syntax/effects/examples_core_operations_04.aivi{aivi}

### `load`

The standard library function `load` lifts a typed `Source` (see [External Sources](external_sources.md)) into an `Effect`.

<<< ../snippets/from_md/syntax/effects/load.aivi{aivi}

## 9.2 `do Effect` blocks

<<< ../snippets/from_md/syntax/effects/do_effect_blocks.aivi{aivi}

This is syntax sugar for monadic binding (see Desugaring section). All effectful operations within these blocks are automatically sequenced.

Inside a `do Effect { ... }` block:

- `x <- eff` binds the result of an `Effect` to `x`
- `x = e` is a pure local binding (does not run effects)
- `x <- res` acquires a `Resource` (see [Resources](resources.md))
- Branching is done with ordinary expressions (`if`, `match`); `->` guards are generator-only.
- If a final expression is present, it must be an `Effect` (commonly `pure value` or an effect call like `print "..."`).
- If there is no final expression, the block defaults to `pure Unit`.

Compiler checks:

- `x = e` requires `e` to be a pure expression (not `Effect` and not `Resource`).
  If you want to run an effect, use `<-`:
  `use '<-' to run effects; '=' binds pure values`.
- Expression statements in statement position (not the final expression) must be `Effect E Unit`.
  If an effect returns a non-`Unit` value, you must bind it explicitly (even if you bind to `_`).

### Fallback with `or` (fallback-only)

`or` is **not** a general matcher. It is fallback-only sugar for common "default on error" patterns.

Two forms exist:

1) **Effect fallback** (inside `do Effect {}` and only after `<-`):

<<< ../snippets/from_md/syntax/effects/fallback_with_or_fallback_only_01.aivi{aivi}

This runs the effect; if it fails, it produces the fallback value instead.

You can also match on the error value using arms (patterns match the **error**, not `Err`):

<<< ../snippets/from_md/syntax/effects/fallback_with_or_fallback_only_02.aivi{aivi}

2) **Result fallback** (expression form):

<<< ../snippets/from_md/syntax/effects/fallback_with_or_fallback_only_03.aivi{aivi}

Or with explicit `Err ...` arms:

<<< ../snippets/from_md/syntax/effects/fallback_with_or_fallback_only_04.aivi{aivi}

Restrictions (v0.1):

- Effect fallback arms match the error value (so write `NotFound m`, not `Err NotFound m`).
- In `do Effect { ... }`, `x <- eff or | Err ... => ...` is parsed as a **Result** fallback (for ergonomics).
  If you mean effect-fallback, write error patterns directly (`NotFound ...`) rather than `Err ...`.
- Result fallback arms must match only `Err ...` at the top level (no `Ok ...`, no `_`).
  Include a final `Err _` catch-all arm.

### `if ... else Unit` as a statement

In `do Effect { ... }`, this common pattern is allowed without `_ <-`:

<<< ../snippets/from_md/syntax/effects/if_else_unit_as_a_statement.aivi{aivi}

Conceptually, the `Unit` branch is lifted to `pure Unit` so both branches have an `Effect` type.

### Concise vs explicit `do Effect` style

These are equivalent:

<<< ../snippets/from_md/syntax/effects/concise_vs_explicit_do_effect_style_01.aivi{aivi}

<<< ../snippets/from_md/syntax/effects/concise_vs_explicit_do_effect_style_02.aivi{aivi}

### `if` with nested blocks inside `do Effect`

`if` is an expression, so you can branch inside a `do Effect { … }` block. When a branch needs multiple steps, use a nested `do Effect { … }` block (since `{ … }` is reserved for record-shaped forms).

This pattern is common when a branch needs multiple effectful steps:

<<< ../snippets/from_md/syntax/effects/if_with_nested_blocks_inside_do_effect.aivi{aivi}

Desugaring-wise, the `if … then … else …` appears inside the continuation of a `bind`, and each branch desugars to its own sequence of `bind` calls.

### Nested `do Effect { … }` expressions inside `if`

An explicit `do Effect { … }` is itself an expression of type `Effect E A`. If you write `do Effect { … }` in an `if` branch, you usually want to run (bind) the chosen effect:

<<< ../snippets/from_md/syntax/effects/nested_do_effect_expressions_inside_if.aivi{aivi}

If you instead write `if … then do Effect { … } else do Effect { … }` *without* binding it, the result of the `if` is an `Effect …` value, not a sequence of steps in the surrounding block (unless it is the final expression of that surrounding `do Effect { … }`).


## 9.3 Effects and patching

<<< ../snippets/from_md/syntax/effects/effects_and_patching.aivi{aivi}

Patches are pure values. Apply them where you have the record value available (often inside a `do Effect` block after decoding/loading).


## 9.4 Comparison and Translation

The `do Effect` block is the primary way to sequence impure operations. It translates directly to monadic binds.

Example translations:

<<< ../snippets/from_md/syntax/effects/comparison_and_translation_01.aivi{aivi}

<<< ../snippets/from_md/syntax/effects/comparison_and_translation_02.aivi{aivi}

<<< ../snippets/from_md/syntax/effects/comparison_and_translation_03.aivi{aivi}

<<< ../snippets/from_md/syntax/effects/comparison_and_translation_04.aivi{aivi}

Example translation:

<<< ../snippets/from_md/syntax/effects/comparison_and_translation_05.aivi{aivi}

## 9.5 Expressive Effect Composition

Effect blocks can be combined with pipelines and pattern matching to create very readable business logic.

### Concatenating effectful operations

<<< ../snippets/from_md/syntax/effects/concatenating_effectful_operations.aivi{aivi}

### Expressive Error Handling

<<< ../snippets/from_md/syntax/effects/expressive_error_handling.aivi{aivi}

## 9.6 Tail-recursive loops

`loop`/`recurse` can also be used inside `do Effect { ... }` blocks for stateful iteration without mutation or explicit recursion. This is the primary way to implement algorithms that need repeated effectful steps (e.g. graph traversal, iterative convergence).

### Syntax

<<< ../snippets/from_md/syntax/effects/syntax.aivi{aivi}


### Example: Dijkstra's shortest paths

<<< ../snippets/from_md/syntax/effects/example_dijkstra_s_shortest_paths.aivi{aivi}


### Desugaring

Inside effect blocks, `loop` desugars to a local recursive function at parse time (same as in generators   see [Generators § 7.6](generators.md#76-tail-recursive-loops)):

```
loop pat = init => { body }
```

becomes:

```
__loopN = pat => body'   // body' has `recurse x` replaced with `__loopN x`
__loopN init
```

The loop body's `{ ... }` block is promoted to the parent effect-block kind, so `<-` binds, `when`/`unless` guards, and `recurse` work correctly inside.

## 9.7 Conditional effects

### `when`

`when cond <- eff` runs `eff` only if `cond` is true:

<<< ../snippets/from_md/syntax/effects/when.aivi{aivi}


### `unless`

`unless cond <- eff` runs `eff` only if `cond` is false:

<<< ../snippets/from_md/syntax/effects/unless.aivi{aivi}


### `given`

`given cond or failExpr` asserts a precondition. If `cond` is false, `failExpr` is evaluated (typically a `fail` call):

<<< ../snippets/from_md/syntax/effects/given.aivi{aivi}


## 9.8 Transition event wiring (`on`)

`on Transition => handler` registers a handler for a machine state transition event inside a `do Effect { ... }` block. This is the mechanism for wiring [Machine Types (§ 3.7)](types.md#37-machine-types-state-machines) to effectful handlers.

### Syntax

```text
on PostfixExpr => Expr
```

- **`PostfixExpr`** evaluates to a machine transition (constructor).
- **`Expr`** is the handler effect to run when the transition fires.

### Example (minimal)

```aivi
on Click => pure Unit
```

### Example: persistent todo list

<<< ../snippets/from_md/syntax/effects/example_persistent_todo_list.aivi{aivi}

`on` is only allowed inside `do Effect { ... }` blocks. Using it in a generic `do M { ... }` block is a type error.
