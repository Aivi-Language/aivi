# aivi.core.set

Set helpers for any `Eq` type, backed by a list. Membership and insertion scan the list;
`fromList` and unions can take quadratic work, while intersection, difference, and subset
checks compare the two collections with nested scans. Costs also depend on element equality.
This module is intended for small membership collections, not hash-set performance.

```aivi
use aivi.core.set (
    Set
    isEmpty
    singleton
    member
    insert
    remove
    size
    toList
    fromList
    union
    intersection
    difference
    subsetOf
)
```

---

## Type

### `Set`

```aivi
type Set A = { items: List A }
```

The helper constructors preserve deduplication, but `Set` is a public record: directly
constructing `{ items: [...] }` can introduce duplicates. Prefer `fromList` for nonempty
input. The element type can be any type that supports equality. The empty set is
`{ items: [] }`.

---

## Construction

### `singleton : A -> Set A`

```aivi
use aivi.core.set (
    Set
    singleton
)

value tags : (Set Text) = singleton "urgent"
```

### `fromList : Eq A => List A -> Set A`

Build a set from a list, discarding duplicates (first occurrence wins).

```aivi
use aivi.core.set (
    Set
    fromList
)

value tags : (Set Text) =
    fromList [
        "urgent",
        "work",
        "urgent"
    ]
```

---

## Querying

### `isEmpty : Set A -> Bool`

```aivi
use aivi.core.set (
    Set
    fromList
    isEmpty
)

value noTags : Bool = isEmpty (fromList [])
```

### `member : Eq A => A -> Set A -> Bool`

```aivi
use aivi.core.set (
    Set
    fromList
    member
)

value hasWork : Bool =
    member "work" (
        fromList [
            "home",
            "work"
        ]
    )
```

### `size : Set A -> Int`

```aivi
use aivi.core.set (
    Set
    fromList
    size
)

value tagCount : Int =
    size (
        fromList [
            "a",
            "b",
            "a"
        ]
    )
```

### `toList : Set A -> List A`

Returns the items in insertion order.

```aivi
use aivi.core.set (
    Set
    fromList
    toList
)

value items : (List Text) =
    toList (
        fromList [
            "a",
            "b",
            "a"
        ]
    )
```

---

## Modification

### `insert : Eq A => A -> Set A -> Set A`

Add a value. If already present, the set is unchanged.

```aivi
use aivi.core.set (
    Set
    fromList
    insert
)

value tags : (Set Text) =
    insert "work" (
        fromList [
            "home"
        ]
    )
```

### `remove : Eq A => A -> Set A -> Set A`

Remove a value. No-op if not present.

```aivi
use aivi.core.set (
    Set
    fromList
    remove
)

value tags : (Set Text) =
    remove "home" (
        fromList [
            "home",
            "work"
        ]
    )
```

---

## Set algebra

### `union : Eq A => Set A -> Set A -> Set A`

All items from both sets (items from `b` appended when not already in `a`).

```aivi
use aivi.core.set (
    Set
    fromList
    union
)

value merged : (Set Text) =
    union (fromList ["a"]) (
        fromList [
            "b",
            "a"
        ]
    )
```

### `intersection : Eq A => Set A -> Set A -> Set A`

Items that appear in both sets.

```aivi
use aivi.core.set (
    Set
    fromList
    intersection
)

value shared : (Set Text) =
    intersection (fromList ["a", "b"]) (
        fromList [
            "b",
            "c"
        ]
    )
```

### `difference : Eq A => Set A -> Set A -> Set A`

Items in `a` that are not in `b`.

```aivi
use aivi.core.set (
    Set
    fromList
    difference
)

value remaining : (Set Text) =
    difference (fromList ["a", "b"]) (
        fromList [
            "b"
        ]
    )
```

### `subsetOf : Eq A => Set A -> Set A -> Bool`

`True` when every item in `a` is also in `b`.

```aivi
use aivi.core.set (
    Set
    fromList
    subsetOf
)

value isSubset : Bool =
    subsetOf (fromList ["a"]) (
        fromList [
            "a",
            "b"
        ]
    )
```

---

## Real-world example

```aivi
use aivi.core.set (
    Set
    fromList
    isEmpty
    difference
    intersection
)

type TagFilter = {
    required: Set Text,
    excluded: Set Text
}

type TagFilter -> (Set Text) -> Bool
func matchesTagSet = filter tagSet => filter
 ||> { required, excluded } -> isEmpty (difference required tagSet) and isEmpty (intersection excluded tagSet)

type TagFilter -> (List Text) -> Bool
func matchesTags = filter tags =>
    matchesTagSet filter (fromList tags)
```
