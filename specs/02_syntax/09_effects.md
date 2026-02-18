# Effects

## 9.1 The `Effect E A` Type

Effectful operations in AIVI are modeled using the `Effect E A` type, where:
- `E` is the **error domain** (describing what could go wrong).
- `A` is the **successful return value**.

### Semantics
- **Atomic Progress**: Effects are either successfully completed, failed with `E`, or **cancelled**.
- **Cancellation**: Cancellation is an asynchronous signal that stops the execution of an effect. When cancelled, the effect is guaranteed to run all registered cleanup (see [Resources](15_resources.md)).
- **Transparent Errors**: Errors in `E` are part of the type signature, forcing explicit handling or propagation.

### Core operations (surface names)

Effect sequencing is expressed via `effect { ... }` blocks, but the underlying interface is:

- `pure : A -> Effect E A` (return a value)
- `bind : Effect E A -> (A -> Effect E B) -> Effect E B` (sequence)
- `fail : E -> Effect E A` (abort with an error)

For *handling* an effect error as a value, the standard library provides:

- `attempt : Effect E A -> Effect F (Result E A)`

`attempt` runs the inner effect and captures its outcome (success or failure with `E`) as a `Result E A`. The outer effect uses a *different* error type `F`, since the original error `E` has been caught and is now represented as data inside the `Result`. If `F` is unconstrained, the outer effect cannot fail (equivalent to `Effect Never (Result E A)` in practice).

### Examples (core operations)

`pure` lifts a value into an effect:

<<< ../snippets/from_md/02_syntax/09_effects/block_01.aivi{aivi}

`bind` sequences effects explicitly (the `effect { ... }` block desugars to `bind`):

<<< ../snippets/from_md/02_syntax/09_effects/block_02.aivi{aivi}

`fail` aborts an effect with an error value:

<<< ../snippets/from_md/02_syntax/09_effects/block_03.aivi{aivi}

`attempt` runs an effect and captures success/failure as a `Result`:

<<< ../snippets/from_md/02_syntax/09_effects/block_04.aivi{aivi}

### `load`

The standard library function `load` lifts a typed `Source` (see [External Sources](12_external_sources.md)) into an `Effect`.

<<< ../snippets/from_md/02_syntax/09_effects/block_05.aivi{aivi}

## 9.2 `effect` blocks

<<< ../snippets/from_md/02_syntax/09_effects/block_06.aivi{aivi}

This is syntax sugar for monadic binding (see Desugaring section). All effectful operations within these blocks are automatically sequenced.

Inside an `effect { ... }` block:

- `x <- eff` binds the result of an `Effect` to `x`
- `x = e` is a pure local binding (does not run effects)
- `x <- res` acquires a `Resource` (see [Resources](15_resources.md))
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

1) **Effect fallback** (inside `effect {}` and only after `<-`):

<<< ../snippets/from_md/02_syntax/09_effects/block_07.aivi{aivi}

This runs the effect; if it fails, it produces the fallback value instead.

You can also match on the error value using arms (patterns match the **error**, not `Err`):

<<< ../snippets/from_md/02_syntax/09_effects/block_08.aivi{aivi}

2) **Result fallback** (expression form):

<<< ../snippets/from_md/02_syntax/09_effects/block_09.aivi{aivi}

Or with explicit `Err ...` arms:

<<< ../snippets/from_md/02_syntax/09_effects/block_10.aivi{aivi}

Restrictions (v0.1):

- Effect fallback arms match the error value (so write `NotFound m`, not `Err NotFound m`).
- In `effect { ... }`, `x <- eff or | Err ... => ...` is parsed as a **Result** fallback (for ergonomics).
  If you mean effect-fallback, write error patterns directly (`NotFound ...`) rather than `Err ...`.
- Result fallback arms must match only `Err ...` at the top level (no `Ok ...`, no `_`).
  Include a final `Err _` catch-all arm.

### `if ... else Unit` as a statement

In `effect { ... }`, this common pattern is allowed without `_ <-`:

<<< ../snippets/from_md/02_syntax/09_effects/block_11.aivi{aivi}

Conceptually, the `Unit` branch is lifted to `pure Unit` so both branches have an `Effect` type.

### Concise vs explicit `effect` style

These are equivalent:

<<< ../snippets/from_md/02_syntax/09_effects/block_12.aivi{aivi}

<<< ../snippets/from_md/02_syntax/09_effects/block_13.aivi{aivi}

### `if` with nested blocks inside `effect`

`if` is an expression, so you can branch inside an `effect { … }` block. When a branch needs multiple steps, use a nested `effect { … }` block (since `{ … }` is reserved for record-shaped forms).

This pattern is common when a branch needs multiple effectful steps:

<<< ../snippets/from_md/02_syntax/09_effects/block_14.aivi{aivi}

Desugaring-wise, the `if … then … else …` appears inside the continuation of a `bind`, and each branch desugars to its own sequence of `bind` calls.

### Nested `effect { … }` expressions inside `if`

An explicit `effect { … }` is itself an expression of type `Effect E A`. If you write `effect { … }` in an `if` branch, you usually want to run (bind) the chosen effect:

<<< ../snippets/from_md/02_syntax/09_effects/block_15.aivi{aivi}

If you instead write `if … then effect { … } else effect { … }` *without* binding it, the result of the `if` is an `Effect …` value, not a sequence of steps in the surrounding block (unless it is the final expression of that surrounding `effect { … }`).


## 9.3 Effects and patching

<<< ../snippets/from_md/02_syntax/09_effects/block_16.aivi{aivi}

Patches are pure values. Apply them where you have the record value available (often inside an `effect` block after decoding/loading).


## 9.4 Comparison and Translation

The `effect` block is the primary way to sequence impure operations. It translates directly to monadic binds.

Example translations:

<<< ../snippets/from_md/02_syntax/09_effects/block_17.aivi{aivi}

<<< ../snippets/from_md/02_syntax/09_effects/block_18.aivi{aivi}

<<< ../snippets/from_md/02_syntax/09_effects/block_19.aivi{aivi}

<<< ../snippets/from_md/02_syntax/09_effects/block_20.aivi{aivi}

Example translation:

<<< ../snippets/from_md/02_syntax/09_effects/block_21.aivi{aivi}

## 9.5 Expressive Effect Composition

Effect blocks can be combined with pipelines and pattern matching to create very readable business logic.

### Concatenating effectful operations

<<< ../snippets/from_md/02_syntax/09_effects/block_22.aivi{aivi}

### Expressive Error Handling

<<< ../snippets/from_md/02_syntax/09_effects/block_23.aivi{aivi}
