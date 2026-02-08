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
email == Some "x"
Some _
Ok { value } when value > 10
```

---

## 4.2 Implicit binding rule

Inside a predicate expression:

* `_` is bound to the **current element**
* bare field names are resolved as `_ . field`

> [!TIP]
> This creates a powerful deconstruction shortcut. `filter active` is interpreted as `filter ({ active } => active)`, meaning boolean fields can be used directly as filters without extra syntax.

```aivi
price > 80        // _.price > 80
active            // _.active
```

---

## 4.3 Predicate lifting

Whenever a function expects:

```aivi
A => Bool
```

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
* generator guards
* patch predicates
* user-defined functions

---

## 4.4 No automatic lifting in predicates

Predicates do **not** auto-lift over `Option` or `Result`.

```aivi
filter (email == "x")      // ❌ if email : Option Text
filter (email == Some "x") // ✅
```

Reason: predicates affect **cardinality**.
