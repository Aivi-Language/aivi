# Generators

Generators are **pure, pull-based sequence producers**. They are for describing sequences of values without mutation, manual iterator state, or I/O.

They:

- do not perform effects
- do not suspend execution stacks like coroutine systems do
- can model finite or infinite data

## 7.1 Generator type

<<< ../snippets/from_md/syntax/generators/generator_type.aivi{aivi}

A generator is a value you can pass around, return from a function, and combine with other pure code.

## 7.2 Generator expressions

<<< ../snippets/from_md/syntax/generators/generator_expressions.aivi{aivi}

A small example shows the overall shape:

```aivi
generate {
  x <- xs
  x -> x > 0 // keep only positive elements
  yield x * 2
}
```

### From Python/JavaScript

The `yield` spelling may look familiar, but the model is different: AIVI generators are pure values, not mutable iterators with hidden local state.

### From Haskell/Scala (no list comprehension syntax)

AIVI does **not** use Haskell-style list comprehensions like:

<<< ../snippets/from_md/syntax/generators/from_haskell_scala_no_list_comprehension_syntax_01.aivi{aivi}

Instead, write the equivalent logic with a `generate` block:

<<< ../snippets/from_md/syntax/generators/from_haskell_scala_no_list_comprehension_syntax_02.aivi{aivi}

## 7.3 Guards and predicates

Generators use three common statement forms:

- `x <- xs` binds `x` to each element produced by `xs`
- `x = e` is a plain pure local binding
- `x -> pred` is a guard that filters the current `x`

In a guard, `pred` is a predicate expression with the implicit `_` bound to `x`, so bare field names like `active` resolve to `x.active`.

This means these are equivalent:

<<< ../snippets/from_md/syntax/generators/guards_and_predicates_01.aivi{aivi}

Note that `.email` is an accessor function (`x => x.email`). It is useful for `map .email`, but in a predicate position you usually want a value such as `email` or `_.email`, not a function.

<<< ../snippets/from_md/syntax/generators/guards_and_predicates_02.aivi{aivi}

Predicate rules are identical to `filter`.

## 7.4 Effectful streaming

AIVI does not include `generate async`. Keep generators pure, and model async or I/O-backed streaming in one of these ways:

- use an `Effect` that *produces* a generator
- use a dedicated stream type from the standard library when that abstraction fits better

This keeps a clear line between pure sequence construction and effectful work.

## 7.5 Expressive Sequence Logic

Generators are a concise way to build complex sequences without intermediate collections or mutation.

### Cartesian Products

<<< ../snippets/from_md/syntax/generators/cartesian_products.aivi{aivi}

### Complex Filtering and Transformation

<<< ../snippets/from_md/syntax/generators/complex_filtering_and_transformation.aivi{aivi}

### Expressive Infinity

<<< ../snippets/from_md/syntax/generators/expressive_infinity.aivi{aivi}

## 7.6 Tail-recursive loops

`loop (pat) = init => { ... }` introduces a local tail-recursive loop for generators. Inside the loop body, `recurse next` continues with the next iteration and updated state.

### Syntax

<<< ../snippets/from_md/syntax/generators/syntax.aivi{aivi}

- **`pattern`** binds the loop state. It may be a tuple, record, or simple name.
- **`initialValue`** is the starting state.
- **`recurse expr`** restarts the loop with the new state `expr`. It must appear in tail position.
- If `recurse` is never reached, the loop terminates.

### Desugaring

`loop` is syntactic sugar for a local recursive function. The compiler transforms:

<<< ../snippets/from_md/syntax/generators/desugaring_01.aivi{aivi}

into approximately:

<<< ../snippets/from_md/syntax/generators/desugaring_02.aivi{aivi}

### Use in effect blocks

`loop` and `recurse` are also available in `do Effect { ... }` blocks for stateful iteration. See [Effects § 9.6](effects.md#96-tail-recursive-loops).
