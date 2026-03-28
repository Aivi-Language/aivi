# aivi.core.set

Unordered set for any `Eq` type. `Set A` is backed by a deduplicated list. All operations are O(n). Use for small membership collections; for large sets prefer index-backed structures.

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

An unordered, deduplicated collection of `A` values. The element type `A` can be any type that supports equality. The empty set is the literal `{ items: [] }`.

---

## Construction

### `singleton : A -> Set A`

```aivi
# <unparseable item>
```

### `fromList : List A -> Set A`

Build a set from a list, discarding duplicates (first occurrence wins).

```aivi
# <unparseable item>
```

---

## Querying

### `isEmpty : Set A -> Bool`

```aivi
# <unparseable item>
```

### `member : A -> Set A -> Bool`

```aivi
# <unparseable item>
```

### `size : Set A -> Int`

```aivi
# <unparseable item>
```

### `toList : Set A -> List A`

Returns the items in insertion order.

```aivi
# <unparseable item>
```

---

## Modification

### `insert : A -> Set A -> Set A`

Add a value. If already present, the set is unchanged.

```aivi
# <unparseable item>
```

### `remove : A -> Set A -> Set A`

Remove a value. No-op if not present.

```aivi
# <unparseable item>
```

---

## Set algebra

### `union : Set A -> Set A -> Set A`

All items from both sets (items from `b` appended when not already in `a`).

```aivi
# <unparseable item>
```

### `intersection : Set A -> Set A -> Set A`

Items that appear in both sets.

```aivi
# <unparseable item>
```

### `difference : Set A -> Set A -> Set A`

Items in `a` that are not in `b`.

```aivi
# <unparseable item>
```

### `subsetOf : Set A -> Set A -> Bool`

`True` when every item in `a` is also in `b`.

```aivi
# <unparseable item>
```

---

## Real-world example

```aivi
use aivi.core.set (
    Set
    fromList
    member
    union
    difference
    toList
)

type TagFilter = {
    required: Set Text,
    excluded: Set Text
}

fun matchesTags:Bool filter:TagFilter tags: (List Text) =>
    let tagSet
```
