# Predicates (Unified Model)

A predicate is a boolean test that reads naturally against a “current value”. AIVI uses the same mental model in helpers such as `filter`, in generator guards, and in patch predicates.

## 4.1 Predicate expressions

Any expression of type `Bool` that uses only:

- literals
- field access
- patterns
- the implicit `_`

is a **predicate expression**.

Examples:

<<< ../snippets/from_md/syntax/predicates/predicate_expressions.aivi{aivi}

Pattern predicates like `Ok { value } when value > 10` are “match tests”: they succeed if the current value matches the pattern, and the `when` guard can refer to names bound by the pattern.

## 4.1.1 Predicate combinators

Predicate expressions support the usual boolean operators:

- `!p` (not)
- `p && q` (and, short-circuit)
- `p || q` (or, short-circuit)

These operators may appear inside any predicate position, including generator guards and patch predicates.

If you want to name predicate functions explicitly, you can treat them as ordinary functions:

<<< ../snippets/from_md/syntax/predicates/predicate_combinators.aivi{aivi}

## 4.2 Implicit binding rule

Inside a predicate expression:

- `_` is bound to the **current element**
- bare field names are resolved as `_.field`
- `.field` is an accessor function (`x => x.field`), not a field value

> [!TIP]
> `users |> filter active` is shorthand for `users |> filter (_.active)` when `active` is a boolean field. If `active` is already bound in scope, that existing binding wins instead.

<<< ../snippets/from_md/syntax/predicates/implicit_binding_rule.aivi{aivi}

## 4.3 Predicate lifting

Whenever a function expects a predicate of shape `A -> Bool`, you can often write the body of the test directly as a predicate expression.

<<< ../snippets/from_md/syntax/predicates/predicate_lifting_01.aivi{aivi}

In other words, the compiler can treat:

```text
predicateExpr
⇒ (_ => predicateExpr)
```

as shorthand in predicate positions.

This applies to:

- `filter`, `find`, `takeWhile`, `dropWhile`
- generator guards (`x -> pred`)
- patch predicates
- user-defined functions that expect a predicate argument

Examples:

<<< ../snippets/from_md/syntax/predicates/predicate_lifting_02.aivi{aivi}

<<< ../snippets/from_md/syntax/predicates/predicate_lifting_03.aivi{aivi}

<<< ../snippets/from_md/syntax/predicates/predicate_lifting_04.aivi{aivi}

<<< ../snippets/from_md/syntax/predicates/predicate_lifting_05.aivi{aivi}

## 4.4 No automatic lifting in predicates

Predicates do **not** automatically lift over `Option` or `Result`.

<<< ../snippets/from_md/syntax/predicates/no_automatic_lifting_in_predicates.aivi{aivi}

The reason is practical: predicates influence **cardinality**. A failed match inside a predicate is not the same thing as “filter this element out”, so the language requires you to make that choice explicitly.
