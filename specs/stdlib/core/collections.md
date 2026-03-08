# Collections

<!-- quick-info: {"kind":"module","name":"aivi.collections"} -->
The collections family provides persistent data structures for functional AIVI code. `List` is a built-in type with extra helpers in `aivi.list`, while `aivi.collections` brings the `Map`, `Set`, `Queue`, `Deque`, and `Heap` namespaces into scope together with the `Collections` and `MinHeap` domains.
<!-- /quick-info -->
<div class="import-badge">use aivi.collections</div>

## What this module is for

`aivi.collections` is the easiest way to bring AIVI's namespace-based immutable collection APIs into scope. The data structures in this family are **persistent**: when you insert, remove, or update, you get a new value back instead of mutating the old one.

That makes them a good fit for AIVI programs where you want predictable data flow, easy refactoring, and safe reuse of earlier values.

## Imports at a glance

- `List` is available as a built-in type; use `aivi.list` for top-level helper functions such as `chunk`, `findMap`, and `forM_`.
- `Map` helpers are available either as `Map.get`, `Map.insert`, and so on through `aivi.collections`, or as unqualified functions through `aivi.map`.
- `Set`, `Queue`, `Deque`, and `Heap` are used through their qualified namespaces (`Set.*`, `Queue.*`, `Deque.*`, `Heap.*`) from `aivi.collections` or `aivi`.

## Start here

A quick mental model:

- **List** keeps order and duplicates.
- **Map** keeps values by key.
- **Set** keeps unique members.
- **Queue / Deque** keeps arrival order for work.
- **Heap** keeps the next smallest item ready.

## Overview

<<< ../../snippets/from_md/stdlib/core/collections/block_01.aivi{aivi}


## Choosing a collection

- Use **`List`** when order matters and duplicates are allowed.
- Use **`Map`** when you need to look up values by key.
- Use **`Set`** when you care about membership and uniqueness.
- Use **`Queue`** or **`Deque`** when you are processing items in arrival order.
- Use **`Heap`** when you repeatedly need the smallest item first.

## Literals and merging

AIVI gives collections a compact literal syntax so common data structures stay readable.

### Map literals

Map entries use `key => value`. Keys and values can be full expressions, and `...expr` spreads another map into the literal.

When the same key appears more than once, the **last** value wins.

<<< ../../snippets/from_md/stdlib/core/collections/block_02.aivi{aivi}


### Set literals

Set literals hold values directly. `...expr` spreads another set into the literal.

Duplicate values are removed automatically because sets only keep unique members.

<<< ../../snippets/from_md/stdlib/core/collections/block_03.aivi{aivi}


### The `++` operator

`++` means “combine these collections,” but the exact behavior depends on the collection type:

- `List ++ List` appends items.
- `Map ++ Map` performs a right-biased merge.
- `Set ++ Set` performs a union.

`++` is **not** text concatenation. For `Text`, use interpolation such as `"Hello, {name}"`.

<<< ../../snippets/from_md/stdlib/core/collections/block_04.aivi{aivi}

::: repl
```aivi
xs = [1, 2, 3] ++ [4, 5]
// => [1, 2, 3, 4, 5]
m = ~map{ "a" => 1, "b" => 2 }
s = ~set[1, 2, 3, 2, 1]
// => ~set[1, 2, 3]
```
:::

## List helpers (`aivi.list`)

`List` is AIVI's general-purpose ordered collection. Reach for it when you want to process items in order, keep duplicates, or build pipeline-style transformations.

Common list jobs include filtering search results, mapping over rows, chunking work into batches, and folding a sequence into one summary value.

**Class instances** (via [`aivi.logic`](logic.md)): `Setoid` · `Ord` · `Functor` · `Filterable` · `Foldable` · `Traversable` · `Apply` · `Applicative` · `Chain` · `Monad` · `Semigroup` · `Monoid` · `Alternative` · `Plus`

Those class instances give you shared operations such as `map`, `filter`, `reduce`, `traverse`, `chain`, `concat`, `empty`, `alt`, and `zero`. The functions below are the list-specific helpers from `aivi.list` that you will use most often.

<<< ../../snippets/from_md/stdlib/core/collections/block_05.aivi{aivi}


| Function | Explanation |
| --- | --- |
| **isEmpty** list<br><code>List a -> Bool</code> | Checks whether the list has any elements. |
| **length** list<br><code>List a -> Int</code> | Returns the number of elements. |
| **foldr** f init list<br><code>(a -> b -> b) -> b -> List a -> b</code> | Folds from the right; useful when the order of nesting matters. |
| **scanl** f init list<br><code>(b -> a -> b) -> b -> List a -> List b</code> | Like a running fold: it returns every intermediate accumulator, including `init`. |
| **take** n list<br><code>Int -> List a -> List a</code> | Keeps up to the first `n` elements. For `n <= 0`, the result is `[]`. |
| **drop** n list<br><code>Int -> List a -> List a</code> | Skips up to the first `n` elements. For `n <= 0`, the original list is returned. |
| **takeWhile** pred list<br><code>(a -> Bool) -> List a -> List a</code> | Keeps the longest prefix for which `pred` stays true. |
| **dropWhile** pred list<br><code>(a -> Bool) -> List a -> List a</code> | Drops the longest prefix for which `pred` stays true. |
| **partition** pred list<br><code>(a -> Bool) -> List a -> (List a, List a)</code> | Splits the list into matching and non-matching elements, preserving order. |
| **find** pred list<br><code>(a -> Bool) -> List a -> Option a</code> | Returns the first matching element, or `None` when nothing matches. |
| **findMap** f list<br><code>(a -> Option b) -> List a -> Option b</code> | Applies `f` until it returns `Some`, then stops. |
| **at** index list<br><code>Int -> List a -> Option a</code> | Safely reads a zero-based index. Out-of-range access returns `None` instead of crashing. |
| **indexOf** needle list<br><code>a -> List a -> Option Int</code> | Returns the first index of `needle`, or `None`. |
| **zip** left right<br><code>List a -> List b -> List (a, b)</code> | Pairs items from two lists and stops at the shorter one. |
| **zipWith** f left right<br><code>(a -> b -> c) -> List a -> List b -> List c</code> | Zips and combines in one step. |
| **unzip** pairs<br><code>List (a, b) -> (List a, List b)</code> | Splits a list of pairs into two lists. |
| **intersperse** sep list<br><code>a -> List a -> List a</code> | Inserts `sep` between items with no leading or trailing separator. |
| **chunk** size list<br><code>Int -> List a -> List (List a)</code> | Breaks a list into sublists of length `size`. For `size <= 0`, the result is `[]`. |
| **dedup** list<br><code>List a -> List a</code> | Removes only **consecutive** duplicates. |
| **uniqueBy** key list<br><code>(a -> k) -> List a -> List a</code> | Keeps the first occurrence of each key and discards later duplicates. |
| **traverse_** f list<br><code>(a -> Effect e b) -> List a -> Effect e Unit</code> | Runs an effect for each item and discards the collected results. |
| **sequence_** list<br><code>List (Effect e a) -> Effect e Unit</code> | Executes a list of effects from left to right and discards their results. |
| **mapM** f list<br><code>(a -> Effect e b) -> List a -> Effect e (List b)</code> | Effectful mapping; an alias of `traverse`. |
| **mapM_** f list<br><code>(a -> Effect e b) -> List a -> Effect e Unit</code> | Effectful mapping for side effects only; an alias of `traverse_`. |
| **forM** list f<br><code>List a -> (a -> Effect e b) -> Effect e (List b)</code> | `traverse` with the arguments flipped, which reads well in pipelines. |
| **forM_** list f<br><code>List a -> (a -> Effect e b) -> Effect e Unit</code> | `traverse_` with the arguments flipped. |
| **forEachEffect** f list<br><code>(a -> Effect e b) -> List a -> Effect e Unit</code> | A descriptive alias for effectful iteration. |

The effect-related helpers (`traverse_`, `sequence_`, `mapM`, `forM`, and their `_` variants) work with AIVI's [`Effect`](../../syntax/effects.md) type. Reach for them when each list item triggers logging, I/O, validation, or another effectful step.

## Map helpers (`aivi.map` / `Map.*`)

Use `Map` when your program needs named lookups, caches, indexes, or configuration tables.

Choose `Map` when the first question is “what value belongs to this key?” rather than “what item is next in order?”.

**Class instances** (via [`aivi.logic`](logic.md)): `Setoid` · `Functor` · `Filterable` · `Foldable` · `Semigroup` · `Monoid`

The shared class methods operate on map **values**. Use the functions below when you need key-aware behavior. `aivi.map` exposes these as unqualified helpers; `aivi.collections` exposes the same operations as `Map.empty`, `Map.get`, `Map.insert`, and so on.

<<< ../../snippets/from_md/stdlib/core/collections/block_06.aivi{aivi}


| Function | Explanation |
| --- | --- |
| **empty**<br><code>Map k v</code> | Creates an empty map. |
| **size** map<br><code>Map k v -> Int</code> | Returns the number of entries. |
| **has** key map<br><code>k -> Map k v -> Bool</code> | Checks whether a key is present. |
| **get** key map<br><code>k -> Map k v -> Option v</code> | Safely looks up a key and returns `Some value` or `None`. |
| **insert** key value map<br><code>k -> v -> Map k v -> Map k v</code> | Returns a new map with the entry inserted. |
| **update** key f map<br><code>k -> (v -> v) -> Map k v -> Map k v</code> | Changes an existing value in place conceptually; if the key is missing, nothing happens. |
| **remove** key map<br><code>k -> Map k v -> Map k v</code> | Returns a new map without that key. |
| **mapWithKey** f m<br><code>(k -> v -> v2) -> Map k v -> Map k v2</code> | Transforms each value while also seeing its key. |
| **keys** m<br><code>Map k v -> List k</code> | Returns all keys as a list. |
| **values** m<br><code>Map k v -> List v</code> | Returns all values as a list. |
| **entries** m<br><code>Map k v -> List (k, v)</code> | Returns key/value pairs. |
| **fromList** entries<br><code>List (k, v) -> Map k v</code> | Builds a map from a list of pairs. |
| **toList** m<br><code>Map k v -> List (k, v)</code> | Converts a map back into key/value pairs. |
| **union** left right<br><code>Map k v -> Map k v -> Map k v</code> | Merges two maps; when the same key appears in both, the right map wins. |
| **getOrElse** key default m<br><code>k -> v -> Map k v -> v</code> | Reads a key or returns `default` when it is missing. |
| **alter** key f m<br><code>k -> (Option v -> Option v) -> Map k v -> Map k v</code> | Handles insert, update, and remove in one function by transforming `Option v`. |
| **mergeWith** combine left right<br><code>(k -> v -> v -> v) -> Map k v -> Map k v -> Map k v</code> | Merges two maps and uses `combine` only for keys that exist on both sides. |
| **filterWithKey** pred m<br><code>(k -> v -> Bool) -> Map k v -> Map k v</code> | Keeps entries that satisfy a key-aware predicate. |
| **foldWithKey** f init m<br><code>(b -> k -> v -> b) -> b -> Map k v -> b</code> | Folds over entries when you need both key and value. |

Notes:

- `union` is right-biased.
- `update` changes existing entries only.
- Do not depend on the order returned by folds, `keys`, `values`, `entries`, or `toList`. If presentation order matters, convert to a list and sort explicitly before iterating.

## Set operations (`Set.*`)

Use `Set` when membership matters more than order, such as tracking enabled features, selected IDs, or permissions.

`Set` is a good fit for questions like “is this enabled?”, “have I seen this already?”, or “which permissions does this user have?”.

There is no separate `aivi.set` helper module in v0.1; use the qualified `Set.*` names from `aivi.collections` or `aivi`.

<<< ../../snippets/from_md/stdlib/core/collections/block_07.aivi{aivi}


| Function | Explanation |
| --- | --- |
| **Set.empty**<br><code>Set a</code> | Creates an empty set. |
| **Set.size** s<br><code>Set a -> Int</code> | Returns the number of elements. |
| **Set.has** value s<br><code>a -> Set a -> Bool</code> | Checks whether the set contains `value`. |
| **Set.insert** value s<br><code>a -> Set a -> Set a</code> | Returns a new set with `value` included. |
| **Set.remove** value s<br><code>a -> Set a -> Set a</code> | Returns a new set without `value`. |
| **Set.union** left right<br><code>Set a -> Set a -> Set a</code> | Combines both sets. |
| **Set.intersection** left right<br><code>Set a -> Set a -> Set a</code> | Keeps only values found in both sets. |
| **Set.difference** left right<br><code>Set a -> Set a -> Set a</code> | Keeps values that are in `left` but not in `right`. |
| **Set.fromList** values<br><code>List a -> Set a</code> | Builds a set from a list, dropping duplicates. |
| **Set.toList** s<br><code>Set a -> List a</code> | Converts a set into a list. |
| **Set.contains** value s<br><code>a -> Set a -> Bool</code> | Alias of `Set.has`. |

## Queue and deque operations (`Queue.*`, `Deque.*`)

A `Queue` is for first-in, first-out work: task scheduling, breadth-first search, or message buffering.

Choose `Queue` when “oldest item first” is the rule. If you need to add or remove items at both ends, move to `Deque`.

There is no separate `aivi.queue` helper module in v0.1; use `Queue.*` and `Deque.*` from `aivi.collections` or `aivi`.

<<< ../../snippets/from_md/stdlib/core/collections/block_08.aivi{aivi}


| Function | Explanation |
| --- | --- |
| **Queue.empty**<br><code>Queue a</code> | Creates an empty queue. |
| **Queue.enqueue** value q<br><code>a -> Queue a -> Queue a</code> | Adds a value at the back. |
| **Queue.dequeue** q<br><code>Queue a -> Option (a, Queue a)</code> | Removes the front value and returns it together with the remaining queue. |
| **Queue.peek** q<br><code>Queue a -> Option a</code> | Reads the front value without removing it. |

### Deque

A `Deque` (“double-ended queue”) lets you add or remove items from both ends.

That makes it useful for undo buffers, sliding windows, and algorithms that grow from either side.

<<< ../../snippets/from_md/stdlib/core/collections/block_09.aivi{aivi}


| Function | Explanation |
| --- | --- |
| **Deque.empty**<br><code>Deque a</code> | Creates an empty deque. |
| **Deque.pushFront** value d<br><code>a -> Deque a -> Deque a</code> | Adds a value to the front. |
| **Deque.pushBack** value d<br><code>a -> Deque a -> Deque a</code> | Adds a value to the back. |
| **Deque.popFront** d<br><code>Deque a -> Option (a, Deque a)</code> | Removes and returns the front value, if any. |
| **Deque.popBack** d<br><code>Deque a -> Option (a, Deque a)</code> | Removes and returns the back value, if any. |
| **Deque.peekFront** d<br><code>Deque a -> Option a</code> | Reads the front value without removing it. |
| **Deque.peekBack** d<br><code>Deque a -> Option a</code> | Reads the back value without removing it. |

## Heap operations (`Heap.*`)

A `Heap` is a priority queue. It is useful when the “next” item is whichever value compares smallest, not whichever arrived first.

Use it when urgency or score decides what happens next: shortest path search, “run the soonest deadline first”, or “pick the smallest cost so far”.

There is no separate `aivi.heap` helper module in v0.1; use the qualified `Heap.*` names from `aivi.collections` or `aivi`.

<<< ../../snippets/from_md/stdlib/core/collections/block_10.aivi{aivi}


| Function | Explanation |
| --- | --- |
| **Heap.empty**<br><code>Heap a</code> | Creates an empty heap. |
| **Heap.push** value h<br><code>a -> Heap a -> Heap a</code> | Inserts a value into the heap. |
| **Heap.popMin** h<br><code>Heap a -> Option (a, Heap a)</code> | Removes and returns the smallest value together with the remaining heap. |
| **Heap.peekMin** h<br><code>Heap a -> Option a</code> | Reads the smallest value without removing it. |
| **Heap.fromList** xs<br><code>List a -> Heap a</code> | Builds a heap from a list. |
| **Heap.size** h<br><code>Heap a -> Int</code> | Returns the number of stored elements. |

`Heap` ordering comes from `Ord` for the element type. Comparable primitives and tuples of comparable primitives use their normal ordering, with tuples ordered lexicographically.
