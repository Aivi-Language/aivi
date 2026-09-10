# aivi.core.set

`aivi.core.set` provides immutable sets for values with an `Eq` instance. A `Set A` is list-backed, so membership and updates take linear time and `fromList` can take quadratic time. Use it for small membership collections in the MVP.

The public representation is `{ items: List A }`, but direct record construction can contain duplicates. `singleton`, `insert`, and `fromList` preserve uniqueness and insertion order.

## Type class operations

This module's `Set` implements `Foldable`: ambient `reduce` visits each member once in insertion
order. `Default (Set A)` creates an empty set. These operations need no equality comparisons.

An unrestricted `Functor Set` would need equality evidence for the mapped element type and could
merge distinct members. Use operations that make that change of membership explicit.

## API

| Export | Type | Behavior |
| --- | --- | --- |
| `Set` | `Set A` | List-backed set record type |
| `isEmpty` | `Set A -> Bool` | Test whether the set has no items |
| `singleton` | `A -> Set A` | Create a one-item set |
| `member` | `Eq A => A -> Set A -> Bool` | Test membership |
| `insert` | `Eq A => A -> Set A -> Set A` | Add an item if absent |
| `remove` | `Eq A => A -> Set A -> Set A` | Remove an item if present |
| `size` | `Set A -> Int` | Count items |
| `toList` | `Set A -> List A` | Return items in insertion order |
| `fromList` | `Eq A => List A -> Set A` | Build a set and discard duplicates |

```aivi
use aivi.core.set (
    Set
    fromList
    member
)

value tags : Set Text =
    fromList [
        "work",
        "urgent",
        "work"
    ]

value isUrgent : Bool = member "urgent" tags
```
