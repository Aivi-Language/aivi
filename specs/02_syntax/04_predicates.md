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

Pattern predicates like `Ok { value } when value > 10` are “match tests”: they succeed if the current value matches the pattern, and the `when` guard can refer to names bound by the pattern.

## 4.1.1 Predicate combinators

Predicate expressions support the usual boolean operators:

* `!p` (not)
* `p && q` (and, short-circuit)
* `p || q` (or, short-circuit)

These operators may appear inside any predicate position (including generator guards and patch predicates).

If you want to name predicate functions explicitly, you can treat them as ordinary functions:

```aivi
Pred A = A => Bool

andPred : Pred A -> Pred A -> Pred A
andPred p q = x => p x && q x

isActive : Pred User
isActive = .active

isPremium : Pred User
isPremium = u => u.tier == Premium

isActivePremium : Pred User
isActivePremium = andPred isActive isPremium
```


## 4.2 Implicit binding rule

Inside a predicate expression:

* `_` is bound to the **current element**
* bare field names are resolved as `_.field`
* `.field` is an accessor function (`x => x.field`), not a field value

> [!TIP]
> `filter active` is shorthand for `filter (_.active)` when `active` is a boolean field. If `active` is bound in scope, it refers to that binding instead.

```aivi
price > 80        // _.price > 80
active            // _.active
```


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
* generator guards (`x -> pred`)
* patch predicates
* user-defined functions

Examples:

```aivi
users |> filter active
users |> filter (age > 18)
users |> find (email == Some "x")
xs |> takeWhile (_ < 10)
xs |> dropWhile (_ < 0)
```

```aivi
generate {
  u <- users
  u -> active && tier == Premium
  yield u
}
```

```aivi
store <| { items[price > 80].discount: 0.1 }
store <| { categories[name == "Hardware"].items[active].price: _ * 1.1 }
```

```aivi
where : (A => Bool) -> List A -> List A
where pred xs = xs |> filter pred

admins = where (role == Admin) users
activeUsers = where active users
```


## 4.4 No automatic lifting in predicates

Predicates do **not** auto-lift over `Option` or `Result`.

```aivi
filter (email == "x")      // ❌ if email : Option Text
filter (email == Some "x") // ✅
```

Reason: predicates affect **cardinality**.
