# Classes and instances

Classes elaborate to records of methods (dictionary passing is compile-time, but can be expressed in kernel).

| Surface | Desugaring |
| :--- | :--- |
| `class Functor (F *) = { map: ... }` | type-level: `FunctorDict F = { map : ∀A B. (A -> B) -> F A -> F B }` |
| `instance Monad (Option *) = { ... }` | value-level: `monadOption : MonadDict Option = { ... }` |
| method call `map xs f` | `map{dict} xs f` after resolution (or `dict.map xs f`) |

(Resolution/elaboration is a compile-time phase; kernel representation is dictionary passing.)

## Type-variable constraints

Surface class syntax may declare constraints on member type variables:

```aivi
class Collection (C *) = with (A: Eq) {
  unique: C A -> C A
}
```

Informal elaboration: constraints like `(A: Eq)` behave like additional dictionary requirements
for methods whose signatures mention `A` (e.g. `unique` elaborates as if it can use an `Eq A`
dictionary when typechecking/elaborating its implementation).
