# Generators

Generators are **pure, pull-based sequence producers**. They are distinct from effects: a `generate { ... }` block is purely functional and cannot perform I/O.

They:

* do not perform effects
* do not suspend execution stacks
* model finite or infinite data


## 7.1 Generator type

<<< ../snippets/from_md/02_syntax/07_generators/block_01.aivi{aivi}

## 7.2 Generator expressions

<<< ../snippets/from_md/02_syntax/07_generators/block_02.aivi{aivi}

### From Python/JavaScript
Similar to `yield` syntax, but purely functional (no mutable iterator state).

### From Haskell/Scala (no list comprehension syntax)

AIVI does **not** use Haskell-style list comprehensions like:

<<< ../snippets/from_md/02_syntax/07_generators/block_03.aivi{aivi}

Instead, write the equivalent logic with a `generate` block:

<<< ../snippets/from_md/02_syntax/07_generators/block_04.aivi{aivi}


## 7.3 Guards and predicates

Generators use a Scala/Haskell-style binder:

* `x <- xs` binds `x` to each element produced by `xs`
* `x = e` is a plain (pure) local binding
* `x -> pred` is a guard (filters `x`); multiple guards may appear

In a guard, `pred` is a predicate expression with the implicit `_` bound to `x` (so bare fields like `active` resolve to `x.active`).

This means these are equivalent:

<<< ../snippets/from_md/02_syntax/07_generators/block_05.aivi{aivi}

Note: `.email` is an accessor function (`x => x.email`). It’s useful for `map .email`, but in a predicate position you usually want a value like `email` / `_.email`, not a function.

<<< ../snippets/from_md/02_syntax/07_generators/block_06.aivi{aivi}

Predicate rules are identical to `filter`.


## 7.4 Effectful streaming (future direction)

The v0.1 surface syntax does **not** include `generate async`.

The recommended model is:

- keep `Generator` pure, and
- represent async / I/O-backed streams as an `Effect` that *produces* a generator, or via a dedicated `Stream` type in the standard library.

This aligns with the general design principle: generators stay pure; use `Effect` for async pull.
## 7.5 Expressive Sequence Logic

Generators provide a powerful, declarative way to build complex sequences without intermediate collections or mutation.

### Cartesian Products

<<< ../snippets/from_md/02_syntax/07_generators/block_07.aivi{aivi}

### Complex Filtering and Transformation

<<< ../snippets/from_md/02_syntax/07_generators/block_08.aivi{aivi}

### Expressive Infinity

<<< ../snippets/from_md/02_syntax/07_generators/block_09.aivi{aivi}

## 7.6 Tail-recursive loops

`loop (pat) = init => { ... }` introduces a local tail-recursive loop for generators.
Inside the loop body, `recurse next` continues with the next iteration with updated state.

### Syntax

```aivi
loop pattern = initialValue => {
  // body: may yield, may recurse
  yield someValue
  recurse nextState
}
```

- **`pattern`** binds the loop state (may be a tuple, record, or simple name).
- **`initialValue`** is the starting state.
- **`recurse expr`** restarts the loop with the new state `expr`. It must appear in tail position.
- If `recurse` is never reached (e.g. a branch doesn't call it), the loop terminates.

### Desugaring

`loop` is syntactic sugar for a local recursive function. The compiler transforms:

```aivi
loop (a, b) = (0, 1) => {
  yield a
  recurse (b, a + b)
}
```

into (approximately):

```aivi
__loop0 = (a, b) => {
  yield a
  __loop0 (b, a + b)
}
__loop0 (0, 1)
```

### Use in effect blocks

`loop`/`recurse` is also available in `do Effect { ... }` blocks for stateful iteration (see [Effects § 9.6](09_effects.md#96-tail-recursive-loops)).
