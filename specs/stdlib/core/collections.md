# Collections

<!-- quick-info: {"kind":"module","name":"aivi.collections"} -->
The collections family provides persistent data structures for functional code. Each data structure lives in its own module:

- `aivi.list`   ordered sequences (built-in type, standard API)
- `aivi.map`   key-value maps
- `aivi.set`   unique value sets
- `aivi.queue`   FIFO queues and double-ended queues
- `aivi.heap`   priority queues

The facade module `aivi.collections` re-exports all of the above for convenience:

<<< ../../snippets/from_md/stdlib/core/collections/overview.aivi{aivi}


<!-- /quick-info -->
<div class="import-badge">use aivi.collections</div>

## Overview

<<< ../../snippets/from_md/stdlib/core/collections/overview.aivi{aivi}

## Literals and Merging

Collections introduce sigil-based literals for concise construction. These are domain literals and are validated at compile time.

### Map literal

<<< ../../snippets/from_md/stdlib/core/collections/aivi_list.aivi{aivi}

Rules:
- Entries use `key => value`.
- Keys and values are full expressions.
- `...expr` spreads another map into the literal.
- When duplicate keys exist, the **last** entry wins (right-biased).

<<< ../../snippets/from_md/stdlib/core/collections/aivi_map.aivi{aivi}

### Set literal

<<< ../../snippets/from_md/stdlib/core/collections/aivi_set.aivi{aivi}

Rules:
- Elements are expressions.
- `...expr` spreads another set.
- Duplicates are removed (set semantics).

### Merge operator

The `++` operator is provided as concatenation for `List`, right-biased merge for `Map`, and union for `Set`. Note that `++` is **not** supported for `Text` — use text interpolation (`"prefix {expr} suffix"`) instead.

<<< ../../snippets/from_md/stdlib/core/collections/aivi_queue.aivi{aivi}

## `aivi.list`

While `List` is a built-in type, AIVI provides a standard `List` API for pipeline-friendly functional programming.

<<< ../../snippets/from_md/stdlib/core/collections/aivi_list.aivi{aivi}


| Function | Explanation |
| --- | --- |
| **empty**<br><code>List a</code> | The empty list `[]`. |
| **isEmpty** list<br><code>List a -> Bool</code> | Returns `true` when the list has zero length. |
| **length** list<br><code>List a -> Int</code> | Returns the number of elements. |
| **map** f list<br><code>(a -> b) -> List a -> List b</code> | Transforms all elements. |
| **filter** pred list<br><code>(a -> Bool) -> List a -> List a</code> | Keeps only elements where `pred` returns `true`. |
| **flatMap** f list<br><code>(a -> List b) -> List a -> List b</code> | Maps and concatenates (List monad bind). |
| **foldl** f init list<br><code>(b -> a -> b) -> b -> List a -> b</code> | Left fold. |
| **foldr** f init list<br><code>(a -> b -> b) -> b -> List a -> b</code> | Right fold. |
| **scanl** f init list<br><code>(b -> a -> b) -> b -> List a -> List b</code> | Like `foldl`, but returns all intermediate accumulators (including `init`). |
| **take** n list<br><code>Int -> List a -> List a</code> | Takes up to `n` elements. For `n <= 0`, returns `[]`. |
| **drop** n list<br><code>Int -> List a -> List a</code> | Drops up to `n` elements. For `n <= 0`, returns the original list. |
| **takeWhile** pred list<br><code>(a -> Bool) -> List a -> List a</code> | Takes the longest prefix where `pred` holds. |
| **dropWhile** pred list<br><code>(a -> Bool) -> List a -> List a</code> | Drops the longest prefix where `pred` holds. |
| **partition** pred list<br><code>(a -> Bool) -> List a -> (List a, List a)</code> | Stable partition into `(yes, no)`. |
| **find** pred list<br><code>(a -> Bool) -> List a -> Option a</code> | Returns the first matching element (or `None`). |
| **findMap** f list<br><code>(a -> Option b) -> List a -> Option b</code> | Returns the first `Some` produced by `f` (or `None`). |
| **at** index list<br><code>Int -> List a -> Option a</code> | Returns `Some element` at `index`, or `None` (supports only `index >= 0`). |
| **indexOf** needle list<br><code>a -> List a -> Option Int</code> | Returns the first index of `needle` (or `None`). |
| **zip** left right<br><code>List a -> List b -> List (a, b)</code> | Zips two lists, truncating to the shorter length. |
| **zipWith** f left right<br><code>(a -> b -> c) -> List a -> List b -> List c</code> | Zips with a combining function, truncating to the shorter length. |
| **unzip** pairs<br><code>List (a, b) -> (List a, List b)</code> | Unzips a list of pairs. |
| **intersperse** sep list<br><code>a -> List a -> List a</code> | Inserts `sep` between elements (no leading/trailing). |
| **chunk** size list<br><code>Int -> List a -> List (List a)</code> | Chunks into sublists of length `size`. For `size <= 0`, returns `[]`. |
| **dedup** list<br><code>List a -> List a</code> | Stable consecutive de-duplication (`[a,a,b,b,a] -> [a,b,a]`). |
| **uniqueBy** key list<br><code>(a -> k) -> List a -> List a</code> | Stable uniqueness by key (keeps first occurrence). Key must be hashable. |
| **traverse** f list<br><code>(a -> Effect e b) -> List a -> Effect e (List b)</code> | Runs effectful mapping left-to-right, collecting results. |
| **traverse_** f list<br><code>(a -> Effect e b) -> List a -> Effect e Unit</code> | Runs effectful mapping left-to-right and discards results. |
| **sequence** list<br><code>List (Effect e a) -> Effect e (List a)</code> | Executes a list of effects left-to-right and collects results. |
| **sequence_** list<br><code>List (Effect e a) -> Effect e Unit</code> | Executes a list of effects left-to-right and discards results. |
| **mapM** f list<br><code>(a -> Effect e b) -> List a -> Effect e (List b)</code> | Alias of `traverse`. |
| **mapM_** f list<br><code>(a -> Effect e b) -> List a -> Effect e Unit</code> | Alias of `traverse_`. |
| **forM** list f<br><code>List a -> (a -> Effect e b) -> Effect e (List b)</code> | Flipped-argument alias of `traverse`. |
| **forM_** list f<br><code>List a -> (a -> Effect e b) -> Effect e Unit</code> | Flipped-argument alias of `traverse_`. |
| **forEachEffect** f list<br><code>(a -> Effect e b) -> List a -> Effect e Unit</code> | Alias of `mapM_` for side-effecting iteration. |

## `aivi.map`

<<< ../../snippets/from_md/stdlib/core/collections/aivi_map.aivi{aivi}


| Function | Explanation |
| --- | --- |
| **empty**<br><code>Map k v</code> | Creates an empty map. |
| **size** map<br><code>Map k v -> Int</code> | Returns the number of entries. |
| **has** key map<br><code>k -> Map k v -> Bool</code> | Returns whether `key` is present. |
| **get** key map<br><code>k -> Map k v -> Option v</code> | Returns `Some value` or `None`. |
| **insert** key value map<br><code>k -> v -> Map k v -> Map k v</code> | Returns a new map with the entry inserted. |
| **update** key f map<br><code>k -> (v -> v) -> Map k v -> Map k v</code> | Applies `f` when `key` exists; otherwise no-op. |
| **remove** key map<br><code>k -> Map k v -> Map k v</code> | Returns a new map without `key`. |
| **map** f m<br><code>(v -> v2) -> Map k v -> Map k v2</code> | Transforms all values with `f`. |
| **mapWithKey** f m<br><code>(k -> v -> v2) -> Map k v -> Map k v2</code> | Transforms values with access to keys. |
| **keys** m<br><code>Map k v -> List k</code> | Returns all keys as a list. |
| **values** m<br><code>Map k v -> List v</code> | Returns all values as a list. |
| **entries** m<br><code>Map k v -> List (k, v)</code> | Returns all entries as key/value pairs. |
| **fromList** entries<br><code>List (k, v) -> Map k v</code> | Builds a map from key/value pairs. |
| **toList** m<br><code>Map k v -> List (k, v)</code> | Converts a map into key/value pairs. |
| **union** left right<br><code>Map k v -> Map k v -> Map k v</code> | Merges maps with right-biased keys. |
| **getOrElse** key default m<br><code>k -> v -> Map k v -> v</code> | Returns the value for `key`, or `default` when missing. |
| **alter** key f m<br><code>k -> (Option v -> Option v) -> Map k v -> Map k v</code> | Inserts/updates/removes by transforming the existing `Option`. |
| **mergeWith** combine left right<br><code>(k -> v -> v -> v) -> Map k v -> Map k v -> Map k v</code> | Merges, resolving conflicts with `combine` (only for keys present in both). |
| **filterWithKey** pred m<br><code>(k -> v -> Bool) -> Map k v -> Map k v</code> | Keeps entries where `pred key value` returns `true`. |
| **foldWithKey** f init m<br><code>(b -> k -> v -> b) -> b -> Map k v -> b</code> | Folds over entries (iteration order is unspecified). |

Notes:
- `union` is right-biased (keys from the right map override).
- `update` applies only when the key exists; otherwise it is a no-op.

## `aivi.set`

<<< ../../snippets/from_md/stdlib/core/collections/aivi_set.aivi{aivi}


| Function | Explanation |
| --- | --- |
| **empty**<br><code>Set a</code> | Creates an empty set. |
| **size** s<br><code>Set a -> Int</code> | Returns the number of elements. |
| **has** value s<br><code>a -> Set a -> Bool</code> | Returns whether `value` is present. |
| **insert** value s<br><code>a -> Set a -> Set a</code> | Returns a new set with `value` inserted. |
| **remove** value s<br><code>a -> Set a -> Set a</code> | Returns a new set without `value`. |
| **union** left right<br><code>Set a -> Set a -> Set a</code> | Returns the union of two sets. |
| **intersection** left right<br><code>Set a -> Set a -> Set a</code> | Returns elements common to both sets. |
| **difference** left right<br><code>Set a -> Set a -> Set a</code> | Returns elements in `left` not in `right`. |
| **fromList** values<br><code>List a -> Set a</code> | Builds a set from a list. |
| **toList** s<br><code>Set a -> List a</code> | Converts a set into a list. |
| **contains** value s<br><code>a -> Set a -> Bool</code> | Alias of `has`. |

## `aivi.queue`

<<< ../../snippets/from_md/stdlib/core/collections/aivi_queue.aivi{aivi}


| Function | Explanation |
| --- | --- |
| **empty**<br><code>Queue a</code> | Creates an empty queue. |
| **enqueue** value q<br><code>a -> Queue a -> Queue a</code> | Adds `value` to the back. |
| **dequeue** q<br><code>Queue a -> Option (a, Queue a)</code> | Removes and returns the front value and remaining queue. |
| **peek** q<br><code>Queue a -> Option a</code> | Returns the front value without removing it. |

### Deque

<<< ../../snippets/from_md/stdlib/core/collections/deque.aivi{aivi}


| Function | Explanation |
| --- | --- |
| **Deque.empty**<br><code>Deque a</code> | Creates an empty deque. |
| **Deque.pushFront** value d<br><code>a -> Deque a -> Deque a</code> | Adds `value` to the front. |
| **Deque.pushBack** value d<br><code>a -> Deque a -> Deque a</code> | Adds `value` to the back. |
| **Deque.popFront** d<br><code>Deque a -> Option (a, Deque a)</code> | Removes and returns the front value and rest. |
| **Deque.popBack** d<br><code>Deque a -> Option (a, Deque a)</code> | Removes and returns the back value and rest. |
| **Deque.peekFront** d<br><code>Deque a -> Option a</code> | Returns the front value without removing it. |
| **Deque.peekBack** d<br><code>Deque a -> Option a</code> | Returns the back value without removing it. |

## `aivi.heap`

<<< ../../snippets/from_md/stdlib/core/collections/aivi_heap.aivi{aivi}


| Function | Explanation |
| --- | --- |
| **empty**<br><code>Heap a</code> | Creates an empty heap. |
| **push** value h<br><code>a -> Heap a -> Heap a</code> | Inserts `value` into the heap. |
| **popMin** h<br><code>Heap a -> Option (a, Heap a)</code> | Removes and returns the smallest value and remaining heap. |
| **peekMin** h<br><code>Heap a -> Option a</code> | Returns the smallest value without removing it. |
| **fromList** xs<br><code>List a -> Heap a</code> | Creates a heap from a list of elements. |
| **size** h<br><code>Heap a -> Int</code> | Returns the number of elements in the heap. |

`Heap` ordering is determined by `Ord` for the element type. Element types must be comparable primitives or tuples of comparable primitives (lexicographic ordering for tuples).
