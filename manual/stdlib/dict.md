# aivi.core.dict

Text-keyed dictionary. `Dict V` is an ordered association map backed by a list of entries. All operations are `O(n)`. For small to medium-sized dicts this is practical and requires no additional runtime support.

The empty dict is written as the record literal `{ entries: [] }`.

```aivi
use aivi.core.dict (Dict, singleton, insert, insertWith, get, getWithDefault,
                    member, remove, size, keys, values, toList, fromList,
                    mapValues, filterValues, mergeWith, union)
```

---

## Dict

```
type Dict V = { entries: List (DictEntry V) }
type DictEntry V = { key: Text, value: V }
```

A `Dict V` is a record with a single field `entries` holding an association list. You can construct an empty dict with the record literal directly:

```aivi
use aivi.core.dict (Dict)

value emptyScores:(Dict Int) = { entries: [] }
```

---

## singleton

Creates a dict with exactly one entry.

```
singleton : Text -> V -> Dict V
```

```aivi
use aivi.core.dict (singleton)

value greeting:(Dict Text) = singleton "hello" "world"
```

---

## insert

Inserts or replaces a key. If the key already exists, the old value is discarded.

```
insert : Text -> V -> Dict V -> Dict V
```

```aivi
use aivi.core.dict (Dict, insert)

value scores:(Dict Int) =
    { entries: [] }
     |> insert "alice" 100
     |> insert "bob" 85
```

---

## insertWith

Inserts a key, combining the new value with the existing one using `merge` if the key is already present.

```
insertWith : (V -> V -> V) -> Text -> V -> Dict V -> Dict V
```

```aivi
use aivi.core.dict (Dict, insertWith)

fun addScore:(Dict Int) key:Text n:Int d:(Dict Int) =>
    insertWith (fun total:Int old:Int new:Int => old + new) key n d
```

---

## get

Looks up a key. Returns `None` when the key is absent.

```
get : Text -> Dict V -> Option V
```

```aivi
use aivi.core.dict (Dict, insert, get)

value d:(Dict Int) = insert "x" 42 { entries: [] }
value found:(Option Int) = get "x" d
```

---

## getWithDefault

Looks up a key, returning a fallback value when the key is absent.

```
getWithDefault : V -> Text -> Dict V -> V
```

```aivi
use aivi.core.dict (Dict, insert, getWithDefault)

value d:(Dict Int) = insert "level" 5 { entries: [] }
value level:Int = getWithDefault 1 "level" d
```

---

## member

Returns `True` if the key exists in the dict.

```
member : Text -> Dict V -> Bool
```

```aivi
use aivi.core.dict (Dict, insert, member)

value d:(Dict Int) = insert "exists" 1 { entries: [] }
value hasIt:Bool = member "exists" d
```

---

## remove

Removes a key. Has no effect if the key is absent.

```
remove : Text -> Dict V -> Dict V
```

```aivi
use aivi.core.dict (Dict, insert, remove)

value d:(Dict Int) = insert "temp" 0 { entries: [] }
value cleaned:(Dict Int) = remove "temp" d
```

---

## size

Returns the number of entries.

```
size : Dict V -> Int
```

```aivi
use aivi.core.dict (Dict, insert, size)

value d:(Dict Int) =
    { entries: [] }
     |> insert "a" 1
     |> insert "b" 2
value count:Int = size d
```

---

## keys / values

Return the keys or values as a list, in insertion order.

```
keys   : Dict V -> List Text
values : Dict V -> List V
```

```aivi
use aivi.core.dict (Dict, insert, keys, values)

value d:(Dict Int) = insert "score" 99 { entries: [] }
value ks:(List Text) = keys d
value vs:(List Int)  = values d
```

---

## toList / fromList

Convert between a `Dict V` and a list of `(Text, V)` pairs.

```
toList   : Dict V -> List (Text, V)
fromList : List (Text, V) -> Dict V
```

```aivi
use aivi.core.dict (Dict, fromList, toList)

value pairs:(List (Text, Int)) = [("a", 1), ("b", 2)]
value d:(Dict Int) = fromList pairs
value back:(List (Text, Int)) = toList d
```

---

## mapValues

Applies a function to every value, preserving keys.

```
mapValues : (V1 -> V2) -> Dict V1 -> Dict V2
```

```aivi
use aivi.core.dict (Dict, insert, mapValues)

value d:(Dict Int) = insert "pts" 5 { entries: [] }
value doubled:(Dict Int) = mapValues (fun n:Int x:Int => x * 2) d
```

---

## filterValues

Keeps only entries whose value satisfies a predicate.

```
filterValues : (V -> Bool) -> Dict V -> Dict V
```

```aivi
use aivi.core.dict (Dict, insert, filterValues)

value d:(Dict Int) =
    { entries: [] }
     |> insert "low" 3
     |> insert "high" 99
value highOnly:(Dict Int) = filterValues (fun b:Bool n:Int => n > 10) d
```

---

## mergeWith

Merges two dicts. When both contain the same key, `combine` is called with the left and right values to produce the merged value.

```
mergeWith : (V -> V -> V) -> Dict V -> Dict V -> Dict V
```

```aivi
use aivi.core.dict (Dict, insert, mergeWith)

value left:(Dict Int) = insert "a" 1 { entries: [] }
value right:(Dict Int) = insert "a" 10 { entries: [] }
value merged:(Dict Int) = mergeWith (fun sum:Int x:Int y:Int => x + y) left right
```

---

## union

Merges two dicts. When a key exists in both, the **right** dict wins.

```
union : Dict V -> Dict V -> Dict V
```

```aivi
use aivi.core.dict (Dict, insert, union)

value defaults:(Dict Int) = insert "timeout" 30 { entries: [] }
value overrides:(Dict Int) = insert "timeout" 60 { entries: [] }
value config:(Dict Int) = union defaults overrides
```
