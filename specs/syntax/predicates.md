# Predicates (Unified Model)

A predicate is a `Bool`-valued test that reads naturally against a “current value”. AIVI uses the same mental model in helpers such as `filter`, in generator guards, and in patch predicates, so the rules on this page carry over directly to [Generators](generators.md) and [Patching Records](patching.md).

## 4.1 Predicate expressions

Predicate positions accept either:

- an ordinary expression of type `Bool`
- a **pattern predicate**, which the compiler desugars to a `Bool`-returning match test

Read the snippet below as predicate bodies. For example, `users |> filter (age > 18)` uses `age > 18`, while `users |> filter (Some _)` uses a parenthesized pattern predicate.

Examples:

<<< ../snippets/from_md/syntax/predicates/predicate_expressions.aivi{aivi}

::: repl
```aivi
items = [{ price: 50 }, { price: 120 }, { price: 80 }]
items |> filter (.price > 70)
// => [{ price: 120 }, { price: 80 }]
```
:::

Pattern predicates such as `Some _` or `Ok { value } when value > 10` are match tests: they succeed if the current value matches the pattern, and the optional `when` guard can refer to names bound by that pattern. In expression positions they are usually parenthesized; generator guards accept the same idea directly. See [Pattern Matching](pattern_matching.md) for the pattern syntax itself.

## 4.1.1 Predicate combinators

Predicate expressions support the usual boolean operators:

- `!p` (not)
- `p && q` (and, short-circuit)
- `p || q` (or, short-circuit)

These operators may appear inside any predicate position, including generator guards and patch predicates.

If you want to name predicate functions explicitly, define ordinary functions of type `A -> Bool` and compose them like any other helpers:

<<< ../snippets/from_md/syntax/predicates/predicate_combinators.aivi{aivi}

## 4.2 Implicit binding rule

When an expression is being interpreted against a "current element" (in filter, find, map, etc.):

- an unbound bare field name such as `price` is shorthand for a field accessor: `price` acts like `x => x.price`
- `.field` is different: it is an explicit accessor function (`x => x.field`), not a field value or boolean test
- `by prop` is sugar for `x => x.prop == prop`, capturing `prop` from the outer scope — useful when the local binding name matches the field name

> [!TIP]
> `users |> filter active` is shorthand for `users |> filter (x => x.active)` when `active` is an unbound name that refers to a boolean field. If `active` is already bound in scope, that existing binding wins instead.

> [!NOTE]
> `_.field` was removed in v0.1 and is now a compile error. Use `.field` for accessor functions or a bare unbound name for field lifting.

If you write a pattern predicate such as `Some _`, the `_` inside the pattern keeps its normal wildcard meaning.

<<< ../snippets/from_md/syntax/predicates/implicit_binding_rule.aivi{aivi}

## 4.3 Predicate lifting and function shorthand

Whenever a call expects a function `A -> B`, you can often write just the body of that function instead of spelling out `x => ...`.

<<< ../snippets/from_md/syntax/predicates/predicate_lifting_01.aivi{aivi}

In other words, the compiler can treat:

```text
expr
⇒ (_ => expr)
```

as shorthand when the surrounding context expects any single-argument function type `A -> B` — not just `A -> Bool`. This means bare unbound names and expressions lift to field accessors in any function position: `map name`, `sortBy age`, `find (price > 100)` all work without `x =>`.

If you already write `_` explicitly, as in `takeWhile (_ < 10)`, you are using the ordinary unary-function shorthand from [Functions](functions.md), so there is nothing extra to lift.

This applies to:

- stdlib helpers such as `filter`, `find`, `map`, `takeWhile`, `dropWhile`, `sortBy`
- generator guards (`item -> pred`) described in [Generators](generators.md)
- patch predicates such as `items[price > 80]` from [Patching Records](patching.md)
- your own helpers that expect a single-argument function

Examples:

<<< ../snippets/from_md/syntax/predicates/predicate_lifting_02.aivi{aivi}

<<< ../snippets/from_md/syntax/predicates/predicate_lifting_03.aivi{aivi}

<<< ../snippets/from_md/syntax/predicates/predicate_lifting_04.aivi{aivi}

<<< ../snippets/from_md/syntax/predicates/predicate_lifting_05.aivi{aivi}

The last two examples show the same shorthand working in patch selectors and in a user-defined helper typed `(A -> Bool) -> ...`.

## 4.3.1 `by prop` — same-name equality sugar

`by prop` desugars to `x => x.prop == prop`, where `prop` is captured from the outer scope. This is useful when the field name and the local variable name are the same, which would otherwise be ambiguous:

```aivi
getUserById = id => users |> find (by id)
// desugars to: id => users |> find (x => x.id == id)
```

Multi-field form: `by (id, name)` desugars to `x => x.id == id && x.name == name`.

## 4.4 No automatic unwrapping over `Option` or `Result`

Predicate sugar does **not** unwrap `Option` or `Result` for you. If a field has type `Option Text`, compare against `Some ...`, use a pattern predicate such as `(Some value when value == "x")`, or unwrap earlier with helpers from [Option](../stdlib/core/option.md) or [Result](../stdlib/core/result.md).

<<< ../snippets/from_md/syntax/predicates/no_automatic_lifting_in_predicates.aivi{aivi}

The same rule applies to `Result`: write the structure you mean, for example `(Ok value when value > 10)`, rather than expecting predicate sugar to look through `Ok` or `Err` automatically.

The reason is practical: predicates influence **cardinality** (how many results are produced). A failed match inside a predicate is not the same thing as “filter this element out”, so the language requires you to make that choice explicitly.
