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

The `++` operator is provided as concatenation for `List`, right-biased merge for `Map`, and union for `Set`.

<<< ../../snippets/from_md/stdlib/core/collections/aivi_queue.aivi{aivi}

## `aivi.list`

While `List` is a built-in type, AIVI provides a standard `List` API for pipeline-friendly functional programming.

<<< ../../snippets/from_md/stdlib/core/collections/aivi_list.aivi{aivi}


| Function | Explanation |
| --- | --- |
| **empty**<br><pre><code>`List a`</code></pre> | The empty list `[]`. |
| **isEmpty** list<br><pre><code>`List a -> Bool`</code></pre> | Returns `true` when the list has zero length. |
| **length** list<br><pre><code>`List a -> Int`</code></pre> | Returns the number of elements. |
| **map** f list<br><pre><code>`(a -> b) -> List a -> List b`</code></pre> | Transforms all elements. |
| **filter** pred list<br><pre><code>`(a -> Bool) -> List a -> List a`</code></pre> | Keeps only elements where `pred` returns `true`. |
| **flatMap** f list<br><pre><code>`(a -> List b) -> List a -> List b`</code></pre> | Maps and concatenates (List monad bind). |
| **foldl** f init list<br><pre><code>`(b -> a -> b) -> b -> List a -> b`</code></pre> | Left fold. |
| **foldr** f init list<br><pre><code>`(a -> b -> b) -> b -> List a -> b`</code></pre> | Right fold. |
| **scanl** f init list<br><pre><code>`(b -> a -> b) -> b -> List a -> List b`</code></pre> | Like `foldl`, but returns all intermediate accumulators (including `init`). |
| **take** n list<br><pre><code>`Int -> List a -> List a`</code></pre> | Takes up to `n` elements. For `n <= 0`, returns `[]`. |
| **drop** n list<br><pre><code>`Int -> List a -> List a`</code></pre> | Drops up to `n` elements. For `n <= 0`, returns the original list. |
| **takeWhile** pred list<br><pre><code>`(a -> Bool) -> List a -> List a`</code></pre> | Takes the longest prefix where `pred` holds. |
| **dropWhile** pred list<br><pre><code>`(a -> Bool) -> List a -> List a`</code></pre> | Drops the longest prefix where `pred` holds. |
| **partition** pred list<br><pre><code>`(a -> Bool) -> List a -> (List a, List a)`</code></pre> | Stable partition into `(yes, no)`. |
| **find** pred list<br><pre><code>`(a -> Bool) -> List a -> Option a`</code></pre> | Returns the first matching element (or `None`). |
| **findMap** f list<br><pre><code>`(a -> Option b) -> List a -> Option b`</code></pre> | Returns the first `Some` produced by `f` (or `None`). |
| **at** index list<br><pre><code>`Int -> List a -> Option a`</code></pre> | Returns `Some element` at `index`, or `None` (supports only `index >= 0`). |
| **indexOf** needle list<br><pre><code>`a -> List a -> Option Int`</code></pre> | Returns the first index of `needle` (or `None`). |
| **zip** left right<br><pre><code>`List a -> List b -> List (a, b)`</code></pre> | Zips two lists, truncating to the shorter length. |
| **zipWith** f left right<br><pre><code>`(a -> b -> c) -> List a -> List b -> List c`</code></pre> | Zips with a combining function, truncating to the shorter length. |
| **unzip** pairs<br><pre><code>`List (a, b) -> (List a, List b)`</code></pre> | Unzips a list of pairs. |
| **intersperse** sep list<br><pre><code>`a -> List a -> List a`</code></pre> | Inserts `sep` between elements (no leading/trailing). |
| **chunk** size list<br><pre><code>`Int -> List a -> List (List a)`</code></pre> | Chunks into sublists of length `size`. For `size <= 0`, returns `[]`. |
| **dedup** list<br><pre><code>`List a -> List a`</code></pre> | Stable consecutive de-duplication (`[a,a,b,b,a] -> [a,b,a]`). |
| **uniqueBy** key list<br><pre><code>`(a -> k) -> List a -> List a`</code></pre> | Stable uniqueness by key (keeps first occurrence). Key must be hashable. |

## `aivi.map`

<<< ../../snippets/from_md/stdlib/core/collections/aivi_map.aivi{aivi}


| Function | Explanation |
| --- | --- |
| **empty**<br><pre><code>`Map k v`</code></pre> | Creates an empty map. |
| **size** map<br><pre><code>`Map k v -> Int`</code></pre> | Returns the number of entries. |
| **has** key map<br><pre><code>`k -> Map k v -> Bool`</code></pre> | Returns whether `key` is present. |
| **get** key map<br><pre><code>`k -> Map k v -> Option v`</code></pre> | Returns `Some value` or `None`. |
| **insert** key value map<br><pre><code>`k -> v -> Map k v -> Map k v`</code></pre> | Returns a new map with the entry inserted. |
| **update** key f map<br><pre><code>`k -> (v -> v) -> Map k v -> Map k v`</code></pre> | Applies `f` when `key` exists; otherwise no-op. |
| **remove** key map<br><pre><code>`k -> Map k v -> Map k v`</code></pre> | Returns a new map without `key`. |
| **map** f m<br><pre><code>`(v -> v2) -> Map k v -> Map k v2`</code></pre> | Transforms all values with `f`. |
| **mapWithKey** f m<br><pre><code>`(k -> v -> v2) -> Map k v -> Map k v2`</code></pre> | Transforms values with access to keys. |
| **keys** m<br><pre><code>`Map k v -> List k`</code></pre> | Returns all keys as a list. |
| **values** m<br><pre><code>`Map k v -> List v`</code></pre> | Returns all values as a list. |
| **entries** m<br><pre><code>`Map k v -> List (k, v)`</code></pre> | Returns all entries as key/value pairs. |
| **fromList** entries<br><pre><code>`List (k, v) -> Map k v`</code></pre> | Builds a map from key/value pairs. |
| **toList** m<br><pre><code>`Map k v -> List (k, v)`</code></pre> | Converts a map into key/value pairs. |
| **union** left right<br><pre><code>`Map k v -> Map k v -> Map k v`</code></pre> | Merges maps with right-biased keys. |
| **getOrElse** key default m<br><pre><code>`k -> v -> Map k v -> v`</code></pre> | Returns the value for `key`, or `default` when missing. |
| **alter** key f m<br><pre><code>`k -> (Option v -> Option v) -> Map k v -> Map k v`</code></pre> | Inserts/updates/removes by transforming the existing `Option`. |
| **mergeWith** combine left right<br><pre><code>`(k -> v -> v -> v) -> Map k v -> Map k v -> Map k v`</code></pre> | Merges, resolving conflicts with `combine` (only for keys present in both). |
| **filterWithKey** pred m<br><pre><code>`(k -> v -> Bool) -> Map k v -> Map k v`</code></pre> | Keeps entries where `pred key value` returns `true`. |
| **foldWithKey** f init m<br><pre><code>`(b -> k -> v -> b) -> b -> Map k v -> b`</code></pre> | Folds over entries (iteration order is unspecified). |

Notes:
- `union` is right-biased (keys from the right map override).
- `update` applies only when the key exists; otherwise it is a no-op.

## `aivi.set`

<<< ../../snippets/from_md/stdlib/core/collections/aivi_set.aivi{aivi}


| Function | Explanation |
| --- | --- |
| **empty**<br><pre><code>`Set a`</code></pre> | Creates an empty set. |
| **size** s<br><pre><code>`Set a -> Int`</code></pre> | Returns the number of elements. |
| **has** value s<br><pre><code>`a -> Set a -> Bool`</code></pre> | Returns whether `value` is present. |
| **insert** value s<br><pre><code>`a -> Set a -> Set a`</code></pre> | Returns a new set with `value` inserted. |
| **remove** value s<br><pre><code>`a -> Set a -> Set a`</code></pre> | Returns a new set without `value`. |
| **union** left right<br><pre><code>`Set a -> Set a -> Set a`</code></pre> | Returns the union of two sets. |
| **intersection** left right<br><pre><code>`Set a -> Set a -> Set a`</code></pre> | Returns elements common to both sets. |
| **difference** left right<br><pre><code>`Set a -> Set a -> Set a`</code></pre> | Returns elements in `left` not in `right`. |
| **fromList** values<br><pre><code>`List a -> Set a`</code></pre> | Builds a set from a list. |
| **toList** s<br><pre><code>`Set a -> List a`</code></pre> | Converts a set into a list. |
| **contains** value s<br><pre><code>`a -> Set a -> Bool`</code></pre> | Alias of `has`. |

## `aivi.queue`

<<< ../../snippets/from_md/stdlib/core/collections/aivi_queue.aivi{aivi}


| Function | Explanation |
| --- | --- |
| **empty**<br><pre><code>`Queue a`</code></pre> | Creates an empty queue. |
| **enqueue** value q<br><pre><code>`a -> Queue a -> Queue a`</code></pre> | Adds `value` to the back. |
| **dequeue** q<br><pre><code>`Queue a -> Option (a, Queue a)`</code></pre> | Removes and returns the front value and remaining queue. |
| **peek** q<br><pre><code>`Queue a -> Option a`</code></pre> | Returns the front value without removing it. |

### Deque

<<< ../../snippets/from_md/stdlib/core/collections/deque.aivi{aivi}


| Function | Explanation |
| --- | --- |
| **Deque.empty**<br><pre><code>`Deque a`</code></pre> | Creates an empty deque. |
| **Deque.pushFront** value d<br><pre><code>`a -> Deque a -> Deque a`</code></pre> | Adds `value` to the front. |
| **Deque.pushBack** value d<br><pre><code>`a -> Deque a -> Deque a`</code></pre> | Adds `value` to the back. |
| **Deque.popFront** d<br><pre><code>`Deque a -> Option (a, Deque a)`</code></pre> | Removes and returns the front value and rest. |
| **Deque.popBack** d<br><pre><code>`Deque a -> Option (a, Deque a)`</code></pre> | Removes and returns the back value and rest. |
| **Deque.peekFront** d<br><pre><code>`Deque a -> Option a`</code></pre> | Returns the front value without removing it. |
| **Deque.peekBack** d<br><pre><code>`Deque a -> Option a`</code></pre> | Returns the back value without removing it. |

## `aivi.heap`

<<< ../../snippets/from_md/stdlib/core/collections/aivi_heap.aivi{aivi}


| Function | Explanation |
| --- | --- |
| **empty**<br><pre><code>`Heap a`</code></pre> | Creates an empty heap. |
| **push** value h<br><pre><code>`a -> Heap a -> Heap a`</code></pre> | Inserts `value` into the heap. |
| **popMin** h<br><pre><code>`Heap a -> Option (a, Heap a)`</code></pre> | Removes and returns the smallest value and remaining heap. |
| **peekMin** h<br><pre><code>`Heap a -> Option a`</code></pre> | Returns the smallest value without removing it. |
| **fromList** xs<br><pre><code>`List a -> Heap a`</code></pre> | Creates a heap from a list of elements. |
| **size** h<br><pre><code>`Heap a -> Int`</code></pre> | Returns the number of elements in the heap. |

`Heap` ordering is determined by `Ord` for the element type. Element types must be comparable primitives or tuples of comparable primitives (lexicographic ordering for tuples).
