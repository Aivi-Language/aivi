# Predicates (Unified Model)

## 4.1 Predicate expressions

Any expression of type `Bool` that uses only:

* literals
* field access
* patterns
* the implicit `_`

is a **predicate expression**.

Examples:

```aivi
price > 80
_.price > 80
email == Some `x`
Some _
Ok { value } when value > 10
```

---

## 4.2 Implicit binding rule

Inside a predicate expression:

* `_` is bound to the **current element**
* bare field names are resolved as `_ . field`

```aivi
price > 80        -- _.price > 80
```

---

## 4.3 Predicate lifting

Whenever a function expects:

```aivi
A => Bool
```

a predicate expression may be supplied.

Desugaring:

```text
predicateExpr
⇒ (_ => predicateExpr)
```

Applies to:

* `filter`, `find`, `takeWhile`, `dropWhile`
* generator guards
* patch predicates
* user-defined functions

---

## 4.4 No automatic lifting in predicates

Predicates do **not** auto-lift over `Option` or `Result`.

```aivi
filter (email == `x`)      -- ❌ if email : Option String
filter (email == Some `x`) -- ✅
```

Reason: predicates affect **cardinality**.
