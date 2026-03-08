# Generators

Generators are **pure sequence builders**. They let you describe a stream of values without mutation, manual iterator state, or I/O.

A helpful mental model is: a generator describes how to produce the next value when a consumer asks for it. That is what **pull-based** means here: the consumer asks for the next item, and the generator tells it how to compute that item.

They:

- stay pure
- do not perform effects
- can model finite or infinite data

## 7.1 Generator type

<<< ../snippets/from_md/syntax/generators/generator_type.aivi{aivi}

A generator is an ordinary value you can pass around, return from a function, and combine with other pure code.

## 7.2 Generator expressions

A small example shows the overall shape:

<<< ../snippets/from_md/syntax/generators/block_01.aivi{aivi}


Read that block top to bottom:

1. take values from `xs`
2. keep only the positive ones
3. yield each remaining value after doubling it

A generator expression can also be as small as a few explicit `yield` statements:

<<< ../snippets/from_md/syntax/generators/generator_expressions.aivi{aivi}

### Common statement forms

Generators use three common statement forms:

- `item <- xs` binds `item` to each value produced by `xs`
- `name = expr` is a plain pure local binding
- `item -> predicate` keeps the current `item` only when the predicate is true

In a guard, the current item is also available implicitly. That is why these forms are equivalent:

<<< ../snippets/from_md/syntax/generators/guards_and_predicates_01.aivi{aivi}

A small but important distinction: `.email` is an accessor function (`user => user.email`). It is useful for `map .email`, but in a guard you usually want a boolean-valued expression such as `email`, `_.email`, or `user.email`.

<<< ../snippets/from_md/syntax/generators/guards_and_predicates_02.aivi{aivi}

Predicate rules are identical to `filter`.

### Comparisons to other languages

If you are translating an idea from another language, this quick note can help. If not, feel free to skip to the next section.

**From Python or JavaScript:** the `yield` spelling may look familiar, but AIVI generators are pure values, not mutable iterators with hidden local state.

**From Haskell or Scala:** AIVI does **not** use list-comprehension syntax.

Haskell-style syntax such as this is not used:

<<< ../snippets/from_md/syntax/generators/from_haskell_scala_no_list_comprehension_syntax_01.aivi{aivi}

Write the same idea with a `generate` block instead:

<<< ../snippets/from_md/syntax/generators/from_haskell_scala_no_list_comprehension_syntax_02.aivi{aivi}

## 7.3 Effectful streaming

AIVI does not include `generate async`. Keep generators pure, and model async or I/O-backed streaming in one of these ways:

- use an `Effect` that *produces* a generator
- use a dedicated stream type from the standard library when that abstraction fits better

This keeps a clear line between pure sequence construction and effectful work.

## 7.4 Building larger sequences

Generators are a concise way to express sequence logic without intermediate collections or mutation.

### Cartesian products

A **Cartesian product** is the combination of every item from one sequence with every item from another sequence.

<<< ../snippets/from_md/syntax/generators/cartesian_products.aivi{aivi}

### Filtering and transformation together

<<< ../snippets/from_md/syntax/generators/complex_filtering_and_transformation.aivi{aivi}

### Infinite sequences

<<< ../snippets/from_md/syntax/generators/expressive_infinity.aivi{aivi}

## 7.5 Tail-recursive loops

`loop (pattern) = initialState => { ... }` introduces a local tail-recursive loop for generators. Inside the loop body, `recurse nextState` continues with the next iteration and updated state.

This is the generator-friendly way to express “keep going with new state” without mutation.
If you only need the practical rule, remember: `recurse` means “run the loop again with this new state”.

### Syntax

<<< ../snippets/from_md/syntax/generators/syntax.aivi{aivi}

- **`pattern`** binds the loop state. It may be a tuple, record, or simple name.
- **`initialState`** is the starting state.
- **`recurse expr`** restarts the loop with the new state `expr`. It must appear in tail position.
- If `recurse` is never reached, the loop terminates.

### Desugaring

“Desugaring” means the compiler rewrites the loop into an ordinary local recursive function.

<<< ../snippets/from_md/syntax/generators/desugaring_01.aivi{aivi}

becomes approximately:

<<< ../snippets/from_md/syntax/generators/desugaring_02.aivi{aivi}

### Use in effect blocks

`loop` and `recurse` are also available in `do Effect { ... }` blocks for stateful iteration. See [Effects § 9.6](effects.md#96-tail-recursive-loops).
