# Effects

`Effect E A` is how AIVI represents work that can do something observable, fail with a typed error, or be cancelled. Use it for operations like file I/O, network calls, UI actions, and other side effects.

## 9.1 The `Effect E A` Type

Think of `Effect E A` as a computation with three important properties:

- it may do observable work such as file I/O, HTTP calls, or UI actions,
- it may fail with a typed error `E`,
- if it succeeds, it produces a value `A`.

Effectful operations in AIVI use the type `Effect E A`, where:

- `E` is the **error domain**, describing what could go wrong
- `A` is the successful return value

### Semantics

- **Atomic progress:** an effect either completes successfully, fails with `E`, or is cancelled
- **Cancellation:** cancellation is an asynchronous signal that stops execution, but registered cleanup still runs; see [Resources](resources.md)
- **Transparent errors:** errors in `E` are part of the type signature, so callers can see what must be handled or propagated

### Core operations (surface names)

Effect sequencing is usually written with `do Effect { ... }`, but the underlying interface is:

- `pure : A -> Effect E A` — lift a plain value into an effect
- `bind : Effect E A -> (A -> Effect E B) -> Effect E B` — sequence one effect after another
- `fail : E -> Effect E A` — stop with an error value

In everyday code, most people mainly use `do Effect { ... }`, `pure`, and `attempt`. The lower-level names are still worth knowing because they explain what the block syntax expands to. The later sections of this page focus on the surface syntax built from these operations.

For handling an effect error as data, the standard library provides:

- `attempt : Effect E A -> Effect F (Result E A)`

`attempt` runs the inner effect and captures success or failure as a `Result E A`. The outer effect uses a different error type `F`, because the original `E` has been converted into data.

### Capability requirements

After you understand the basic `Effect E A` shape, the next refinement is authority: which outside operations this effect is allowed to use.

Capabilities refine `Effect E A` without turning it into a different effect type. Write a minimum-authority clause after the effect type:

```aivi
loadConfig : Text -> Effect ConfigError AppConfig with { file.read, process.env.read }
```

- the capability clause is checked statically and is **not** part of `E`
- callers may run an effect in any larger capability scope
- lexical narrowing uses `with { ... } in expr`
- handler or interpreter binding uses the same scope form via `with { capability = handler } in expr`

See [Capabilities](capabilities.md) and [Effect Handlers](effect_handlers.md) for the shared rules.

### Examples (core operations)

`pure` lifts a value into an effect:

<<< ../snippets/from_md/syntax/effects/examples_core_operations_01.aivi{aivi}

`bind` sequences effects explicitly; `do Effect { ... }` is the readable surface form of the same idea:

<<< ../snippets/from_md/syntax/effects/examples_core_operations_02.aivi{aivi}

`fail` aborts an effect with an error value:

<<< ../snippets/from_md/syntax/effects/examples_core_operations_03.aivi{aivi}

`attempt` runs an effect and captures success or failure as a `Result`:

<<< ../snippets/from_md/syntax/effects/examples_core_operations_04.aivi{aivi}

### `load`

The standard library function `load` lifts a typed `Source` (see [External Sources](external_sources.md)) into an `Effect`.

<<< ../snippets/from_md/syntax/effects/load.aivi{aivi}

The capability required by `load` depends on the source kind:

- file or image sources → `file.read`
- REST / HTTP / HTTPS sources → `network.http`
- environment sources → `process.env.read`
- database-backed source loads → `db.query`
- `@static` embedded sources → no runtime capability after compilation

When a source value carries composition metadata, `load` runs the source's canonical pipeline before returning the final value.

See [External Sources](external_sources.md) and [Capabilities](capabilities.md) for the full mapping.

## 9.2 `do Effect` blocks

A `do Effect { ... }` block is the everyday way to write effectful code. This section starts with the basic block rules, then covers fallback and branching, and finally shows advanced patterns such as loops and transition hooks.

<<< ../snippets/from_md/syntax/effects/do_effect_blocks.aivi{aivi}

A `do Effect { ... }` block lets you write effectful code in the same order the steps happen.

```aivi
do Effect {
  user <- loadUser id   // run the effect and bind its result
  pure user.name        // wrap the final plain value back into Effect
}
```

Inside a `do Effect { ... }` block:

- `x <- eff` runs an `Effect` and binds its result to `x`
- `x = e` is a pure local binding and does not run effects
- `x <- res` acquires a `Resource`; see [Resources](resources.md)
- branching uses ordinary expressions such as `if` and `match`
- if a final expression is present, it must itself be an `Effect`
- if there is no final expression, the block defaults to `pure Unit`

Compiler checks:

- `x = e` requires `e` to be a pure expression, not an `Effect` and not a `Resource`
- expression statements in statement position, rather than the final position, must be `Effect E Unit`
- if an effect returns a non-`Unit` value and you want to ignore it, bind it explicitly, even if the binding name is `_`

### Fallback with `or` (fallback-only)

`or` is focused sugar for common “default on error” patterns. It is not a second general-purpose `match` syntax.

Two forms exist:

1. **Effect fallback** inside `do Effect { ... }` and only after `<-`:

<<< ../snippets/from_md/syntax/effects/fallback_with_or_fallback_only_01.aivi{aivi}

This runs the effect and, if it fails, produces the fallback value instead.

You can also match on the error value using arms. In this form, patterns match the **error**, not `Err`:

<<< ../snippets/from_md/syntax/effects/fallback_with_or_fallback_only_02.aivi{aivi}

2. **Result fallback** as an expression form:

<<< ../snippets/from_md/syntax/effects/fallback_with_or_fallback_only_03.aivi{aivi}

Or with explicit `Err ...` arms:

<<< ../snippets/from_md/syntax/effects/fallback_with_or_fallback_only_04.aivi{aivi}

Restrictions:

- effect fallback arms match the error value, so write `NotFound msg`, not `Err NotFound msg`
- in `do Effect { ... }`, `x <- eff or | Err ... => ...` is parsed as a **Result** fallback
- result fallback arms must match only `Err ...` at the top level; include a final `Err _` catch-all arm

### `if ... else Unit` as a statement

In `do Effect { ... }`, this common pattern is allowed without `_ <-`:

```aivi
do Effect {
  _ <- if cond then print "branch" else pure Unit
}
```

This is equivalent to the shorter statement form:

<<< ../snippets/from_md/syntax/effects/if_else_unit_as_a_statement.aivi{aivi}

Conceptually, the `Unit` branch is lifted to `pure Unit` so both branches still have an effect type.

### Concise vs explicit `do Effect` style

These forms are equivalent:

<<< ../snippets/from_md/syntax/effects/concise_vs_explicit_do_effect_style_01.aivi{aivi}

<<< ../snippets/from_md/syntax/effects/concise_vs_explicit_do_effect_style_02.aivi{aivi}

Choose the version that makes sequencing and error flow easiest to read.

### `if` with nested blocks inside `do Effect`

`if` is an expression, so you can branch inside a `do Effect { ... }` block. When a branch needs multiple effectful steps, use a nested `do Effect { ... }` block, because `{ ... }` on its own is reserved for record-shaped forms.

<<< ../snippets/from_md/syntax/effects/if_with_nested_blocks_inside_do_effect.aivi{aivi}

Desugaring-wise, the `if ... then ... else ...` sits inside the continuation of a `bind`, and each branch desugars to its own sequence of `bind` calls.

### Nested `do Effect { ... }` expressions inside `if`

An explicit `do Effect { ... }` is itself an expression of type `Effect E A`. If you write one in an `if` branch, you usually want to run the chosen effect by binding it:

<<< ../snippets/from_md/syntax/effects/nested_do_effect_expressions_inside_if.aivi{aivi}

If you instead write `if ... then do Effect { ... } else do Effect { ... }` without binding it, the `if` expression evaluates to an `Effect ...` value. It only becomes the next step of the surrounding block when it is used in a bind or returned as that block's final expression.

## 9.3 Effects and patching

A patch is still a pure value, even when you use it in effectful code. The effectful part is obtaining the record to patch or deciding when to persist the updated record; the patch expression itself stays an ordinary expression you can name and reuse.

<<< ../snippets/from_md/syntax/effects/effects_and_patching.aivi{aivi}

In practice, compute or load the base record first, then apply the patch where that record value is available.

## 9.4 Comparison and Translation

`do Effect` is the main surface syntax for sequencing impure operations. It translates directly to monadic binds.

Short translations:

<<< ../snippets/from_md/syntax/effects/comparison_and_translation_01.aivi{aivi}

<<< ../snippets/from_md/syntax/effects/comparison_and_translation_02.aivi{aivi}

<<< ../snippets/from_md/syntax/effects/comparison_and_translation_03.aivi{aivi}

<<< ../snippets/from_md/syntax/effects/comparison_and_translation_04.aivi{aivi}

Worked translation:

<<< ../snippets/from_md/syntax/effects/comparison_and_translation_05.aivi{aivi}

## 9.5 Expressive Effect Composition

Effect blocks combine well with pipelines and pattern matching, which makes everyday business logic read as a sequence of named steps rather than as deeply nested callbacks.

### Concatenating effectful operations

<<< ../snippets/from_md/syntax/effects/concatenating_effectful_operations.aivi{aivi}

### Expressive Error Handling

<<< ../snippets/from_md/syntax/effects/expressive_error_handling.aivi{aivi}

## 9.6 Tail-recursive loops

`loop` and `recurse` can also be used inside `do Effect { ... }` blocks for stateful iteration without mutation or explicit named recursion. This is the idiomatic way to express repeated effectful steps such as graph traversal or iterative refinement.

### Syntax

<<< ../snippets/from_md/syntax/effects/syntax.aivi{aivi}

### Example: Dijkstra's shortest paths

<<< ../snippets/from_md/syntax/effects/example_dijkstra_s_shortest_paths.aivi{aivi}

### Desugaring

Inside effect blocks, `loop` desugars to a local recursive function at parse time, using the same basic idea as in [Generators § 7.5](generators.md#75-tail-recursive-loops):

```text
loop pat = init => { body }
```

becomes:

```text
__loopN = pat => body'   // `recurse x` becomes `__loopN x`
__loopN init
```

The loop body's `{ ... }` block is promoted to the parent effect-block kind, so `<-` binds, `when` / `unless`, and `recurse` work correctly inside.

## 9.7 Conditional effects

### `when`

`when cond <- eff` runs `eff` only if `cond` is true:

<<< ../snippets/from_md/syntax/effects/when.aivi{aivi}

### `unless`

`unless cond <- eff` runs `eff` only if `cond` is false:

<<< ../snippets/from_md/syntax/effects/unless.aivi{aivi}

### `given`

`given cond or failExpr` asserts a precondition. If `cond` is false, `failExpr` is evaluated, typically as a `fail ...` call:

<<< ../snippets/from_md/syntax/effects/given.aivi{aivi}

## 9.8 Transition event wiring (`on`)

`on Transition => handler` registers a handler for a machine state transition event inside a `do Effect { ... }` block.

### Syntax

```text
on PostfixExpr => Expr
```

- **`PostfixExpr`** evaluates to a machine transition function value
- **`Expr`** is the handler effect to run when the transition fires

### Ordering

For machine transitions, runtime order is:

1. transition guard check
2. machine state update
3. registered `on` handlers for that transition

If a handler fails, the transition remains applied. See [Machine Runtime Semantics](machines_runtime.md) for the full runtime model.

### Example (minimal)

```aivi
on Click => pure Unit
```

### Example: persistent todo list

<<< ../snippets/from_md/syntax/effects/example_persistent_todo_list.aivi{aivi}

`on` is allowed only inside `do Effect { ... }` blocks. Using it in a generic `do M { ... }` block is a type error.
