# aivi.core.dict

`aivi.core.dict` provides an immutable association map for keys with an `Eq` instance. A `Dict K V` stores entries in a list, so lookup, insertion, and removal take linear time. Building a dictionary with repeated insertion can take quadratic time. Use it for small maps in the MVP.

The empty dictionary is `{ entries: [] }`. `insert` places a new or replaced key first. `fromList` processes pairs from left to right, so the final occurrence of a duplicate key wins and appears first.

## Type class operations

`Dict K` implements `Functor`, `Foldable`, and `Filterable`. Ambient `map` transforms values,
`reduce` folds them in entry order, and `filterMap` removes entries whose transformed value is
`None`. Keys and the relative order of retained entries are preserved. `Default (Dict K V)`
creates an empty dictionary. These operations do not require `Eq K` because they never compare
or insert keys.

`mapValues` remains the explicitly named equivalent of `map`. There is no implicit class-level
merge policy for dictionaries.

## API

| Export | Type | Behavior |
| --- | --- | --- |
| `Dict` | `Dict K V` | Association-map record type |
| `singleton` | `K -> V -> Dict K V` | Create one entry |
| `insert` | `Eq K => K -> V -> Dict K V -> Dict K V` | Insert or replace a value |
| `insertWith` | `Eq K => (V -> V -> V) -> K -> V -> Dict K V -> Dict K V` | Combine an existing value with a new value |
| `get` | `Eq K => K -> Dict K V -> Option V` | Look up a key |
| `getWithDefault` | `Eq K => V -> K -> Dict K V -> V` | Look up a key with a fallback |
| `member` | `Eq K => K -> Dict K V -> Bool` | Test whether a key exists |
| `remove` | `Eq K => K -> Dict K V -> Dict K V` | Remove a key |
| `size` | `Dict K V -> Int` | Count entries |
| `keys` | `Dict K V -> List K` | Return keys in entry order |
| `values` | `Dict K V -> List V` | Return values in entry order |
| `toList` | `Dict K V -> List (K, V)` | Return key/value pairs in entry order |
| `fromList` | `Eq K => List (K, V) -> Dict K V` | Build a dictionary from pairs |
| `mapValues` | `(V1 -> V2) -> Dict K V1 -> Dict K V2` | Transform every value |
| `filterValues` | `(V -> Bool) -> Dict K V -> Dict K V` | Keep entries whose values pass a predicate |

```aivi
use aivi.core.dict (
    Dict
    get
    insert
)

value scores : Dict Text Int =
    insert "Ada" 10 {
        entries: []
    }

value adaScore : Option Int = get "Ada" scores
```
