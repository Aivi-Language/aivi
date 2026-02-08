# Classes and instances

Classes elaborate to records of methods (dictionary passing is compile-time, but can be expressed in kernel).

| Surface | Desugaring |
| :--- | :--- |
| `class Functor (F *) = { map: ... }` | type-level: `FunctorDict F = { map : ∀A B. F A -> (A -> B) -> F B }` |
| `instance Monad (Option *) = { ... }` | value-level: `monadOption : MonadDict Option = { ... }` |
| method call `map xs f` | `map{dict} xs f` after resolution (or `dict.map xs f`) |

(Resolution/elaboration is a compile-time phase; kernel representation is dictionary passing.)
