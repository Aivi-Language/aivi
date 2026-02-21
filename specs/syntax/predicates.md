# Predicates (Unified Model)

## 4.1 Predicate expressions

Any expression of type `Bool` that uses only:

* literals
* field access
* patterns
* the implicit `_`

is a **predicate expression**.

Examples:

<<< ../snippets/from_md/syntax/predicates/predicate_expressions.aivi{aivi}

Pattern predicates like `Ok { value } when value > 10` are “match tests”: they succeed if the current value matches the pattern, and the `when` guard can refer to names bound by the pattern.

## 4.1.1 Predicate combinators

Predicate expressions support the usual boolean operators:

* `!p` (not)
* `p && q` (and, short-circuit)
* `p || q` (or, short-circuit)

These operators may appear inside any predicate position (including generator guards and patch predicates).

If you want to name predicate functions explicitly, you can treat them as ordinary functions:

<<< ../snippets/from_md/syntax/predicates/predicate_combinators.aivi{aivi}


## 4.2 Implicit binding rule

Inside a predicate expression:

* `_` is bound to the **current element**
* bare field names are resolved as `_.field`
* `.field` is an accessor function (`x => x.field`), not a field value

> [!TIP]
> `filter active` is shorthand for `filter (_.active)` when `active` is a boolean field. If `active` is bound in scope, it refers to that binding instead.

<<< ../snippets/from_md/syntax/predicates/implicit_binding_rule.aivi{aivi}


## 4.3 Predicate lifting

Whenever a function expects:

<<< ../snippets/from_md/syntax/predicates/predicate_lifting_01.aivi{aivi}

a predicate expression may be supplied.

> [!NOTE]
> Predicates can also perform complex transformations by deconstructing multiple fields:
> `map { name, id } => if id > 10 then name else "no name"`

Desugaring:

```text
predicateExpr
⇒ (_ => predicateExpr)
```

Applies to:

* `filter`, `find`, `takeWhile`, `dropWhile`
* generator guards (`x -> pred`)
* patch predicates
* user-defined functions

Examples:

<<< ../snippets/from_md/syntax/predicates/predicate_lifting_02.aivi{aivi}

<<< ../snippets/from_md/syntax/predicates/predicate_lifting_03.aivi{aivi}

<<< ../snippets/from_md/syntax/predicates/predicate_lifting_04.aivi{aivi}

<<< ../snippets/from_md/syntax/predicates/predicate_lifting_05.aivi{aivi}


## 4.4 No automatic lifting in predicates

Predicates do **not** auto-lift over `Option` or `Result`.

<<< ../snippets/from_md/syntax/predicates/no_automatic_lifting_in_predicates.aivi{aivi}

Reason: predicates affect **cardinality**.
