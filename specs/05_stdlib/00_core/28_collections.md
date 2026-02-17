# Collections Domain

<!-- quick-info: {"kind":"module","name":"aivi.collections"} -->
The `Collections` domain is your toolbox for structured data. While `List` is great for simple sequences, real-world software needs more. Whether you need to look up users by their ID (Map), keep a list of unique tags (Set), or process tasks by priority (Heap), this domain provides persistent data structures designed for functional code.

<!-- /quick-info -->
## Overview

<<< ../../snippets/from_md/05_stdlib/00_core/28_collections/block_01.aivi{aivi}

## Literals and Merging

Collections introduce sigil-based literals for concise construction. These are domain literals and are validated at compile time.

### Map literal

<<< ../../snippets/from_md/05_stdlib/00_core/28_collections/block_02.aivi{aivi}

Rules:
- Entries use `key => value`.
- Keys and values are full expressions.
- `...expr` spreads another map into the literal.
- When duplicate keys exist, the **last** entry wins (right-biased).

<<< ../../snippets/from_md/05_stdlib/00_core/28_collections/block_03.aivi{aivi}

### Set literal

<<< ../../snippets/from_md/05_stdlib/00_core/28_collections/block_04.aivi{aivi}

Rules:
- Elements are expressions.
- `...expr` spreads another set.
- Duplicates are removed (set semantics).

### Merge operator

The `Collections` domain provides `++` as concatenation for `List`, a right-biased merge for `Map`, and union for `Set`.

<<< ../../snippets/from_md/05_stdlib/00_core/28_collections/block_05.aivi{aivi}

## Core API

The following functions are required. Exact module layout may vary, but names and behavior should match.

### List

While `List` is a built-in type, AIVI provides a standard `List` API (like `Map` and `Set`) for
pipeline-friendly functional programming.

| Function | Explanation |
| --- | --- |
| **List.empty**<br><pre><code>`List a`</code></pre> | The empty list `[]`. |
| **List.isEmpty** list<br><pre><code>`List a -> Bool`</code></pre> | Returns `true` when the list has zero length. |
| **List.length** list<br><pre><code>`List a -> Int`</code></pre> | Returns the number of elements. |
| **List.map** f list<br><pre><code>`(a -> b) -> List a -> List b`</code></pre> | Transforms all elements. |
| **List.filter** pred list<br><pre><code>`(a -> Bool) -> List a -> List a`</code></pre> | Keeps only elements where `pred` returns `true`. |
| **List.flatMap** f list<br><pre><code>`(a -> List b) -> List a -> List b`</code></pre> | Maps and concatenates (List monad bind). |
| **List.foldl** f init list<br><pre><code>`(b -> a -> b) -> b -> List a -> b`</code></pre> | Left fold. |
| **List.foldr** f init list<br><pre><code>`(a -> b -> b) -> b -> List a -> b`</code></pre> | Right fold. |
| **List.scanl** f init list<br><pre><code>`(b -> a -> b) -> b -> List a -> List b`</code></pre> | Like `foldl`, but returns all intermediate accumulators (including `init`). |
| **List.take** n list<br><pre><code>`Int -> List a -> List a`</code></pre> | Takes up to `n` elements. For `n <= 0`, returns `[]`. |
| **List.drop** n list<br><pre><code>`Int -> List a -> List a`</code></pre> | Drops up to `n` elements. For `n <= 0`, returns the original list. |
| **List.takeWhile** pred list<br><pre><code>`(a -> Bool) -> List a -> List a`</code></pre> | Takes the longest prefix where `pred` holds. |
| **List.dropWhile** pred list<br><pre><code>`(a -> Bool) -> List a -> List a`</code></pre> | Drops the longest prefix where `pred` holds. |
| **List.partition** pred list<br><pre><code>`(a -> Bool) -> List a -> (List a, List a)`</code></pre> | Stable partition into `(yes, no)`. |
| **List.find** pred list<br><pre><code>`(a -> Bool) -> List a -> Option a`</code></pre> | Returns the first matching element (or `None`). |
| **List.findMap** f list<br><pre><code>`(a -> Option b) -> List a -> Option b`</code></pre> | Returns the first `Some` produced by `f` (or `None`). |
| **List.at** index list<br><pre><code>`Int -> List a -> Option a`</code></pre> | Returns `Some element` at `index`, or `None` (supports only `index >= 0`). |
| **List.indexOf** needle list<br><pre><code>`a -> List a -> Option Int`</code></pre> | Returns the first index of `needle` (or `None`). |
| **List.zip** left right<br><pre><code>`List a -> List b -> List (a, b)`</code></pre> | Zips two lists, truncating to the shorter length. |
| **List.zipWith** f left right<br><pre><code>`(a -> b -> c) -> List a -> List b -> List c`</code></pre> | Zips with a combining function, truncating to the shorter length. |
| **List.unzip** pairs<br><pre><code>`List (a, b) -> (List a, List b)`</code></pre> | Unzips a list of pairs. |
| **List.intersperse** sep list<br><pre><code>`a -> List a -> List a`</code></pre> | Inserts `sep` between elements (no leading/trailing). |
| **List.chunk** size list<br><pre><code>`Int -> List a -> List (List a)`</code></pre> | Chunks into sublists of length `size`. For `size <= 0`, returns `[]`. |
| **List.dedup** list<br><pre><code>`List a -> List a`</code></pre> | Stable consecutive de-duplication (`[a,a,b,b,a] -> [a,b,a]`). |
| **List.uniqueBy** key list<br><pre><code>`(a -> k) -> List a -> List a`</code></pre> | Stable uniqueness by key (keeps first occurrence). Key must be hashable. |

### Map

| Function | Explanation |
| --- | --- |
| **Map.empty**<br><pre><code>`Map k v`</code></pre> | Creates an empty map. |
| **Map.size** map<br><pre><code>`Map k v -> Int`</code></pre> | Returns the number of entries. |
| **Map.has** key map<br><pre><code>`k -> Map k v -> Bool`</code></pre> | Returns whether `key` is present. |
| **Map.get** key map<br><pre><code>`k -> Map k v -> Option v`</code></pre> | Returns `Some value` or `None`. |
| **Map.insert** key value map<br><pre><code>`k -> v -> Map k v -> Map k v`</code></pre> | Returns a new map with the entry inserted. |
| **Map.update** key f map<br><pre><code>`k -> (v -> v) -> Map k v -> Map k v`</code></pre> | Applies `f` when `key` exists; otherwise no-op. |
| **Map.remove** key map<br><pre><code>`k -> Map k v -> Map k v`</code></pre> | Returns a new map without `key`. |
| **Map.map** f map<br><pre><code>`(v -> v2) -> Map k v -> Map k v2`</code></pre> | Transforms all values with `f`. |
| **Map.mapWithKey** f map<br><pre><code>`(k -> v -> v2) -> Map k v -> Map k v2`</code></pre> | Transforms values with access to keys. |
| **Map.keys** map<br><pre><code>`Map k v -> List k`</code></pre> | Returns all keys as a list. |
| **Map.values** map<br><pre><code>`Map k v -> List v`</code></pre> | Returns all values as a list. |
| **Map.entries** map<br><pre><code>`Map k v -> List (k, v)`</code></pre> | Returns all entries as key/value pairs. |
| **Map.fromList** entries<br><pre><code>`List (k, v) -> Map k v`</code></pre> | Builds a map from key/value pairs. |
| **Map.toList** map<br><pre><code>`Map k v -> List (k, v)`</code></pre> | Converts a map into key/value pairs. |
| **Map.union** left right<br><pre><code>`Map k v -> Map k v -> Map k v`</code></pre> | Merges maps with right-biased keys. |
| **Map.getOrElse** key default map<br><pre><code>`k -> v -> Map k v -> v`</code></pre> | Returns the value for `key`, or `default` when missing. |
| **Map.alter** key f map<br><pre><code>`k -> (Option v -> Option v) -> Map k v -> Map k v`</code></pre> | Inserts/updates/removes by transforming the existing `Option`. |
| **Map.mergeWith** combine left right<br><pre><code>`(k -> v -> v -> v) -> Map k v -> Map k v -> Map k v`</code></pre> | Merges, resolving conflicts with `combine` (only for keys present in both). |
| **Map.filterWithKey** pred map<br><pre><code>`(k -> v -> Bool) -> Map k v -> Map k v`</code></pre> | Keeps entries where `pred key value` returns `true`. |
| **Map.foldWithKey** f init map<br><pre><code>`(b -> k -> v -> b) -> b -> Map k v -> b`</code></pre> | Folds over entries (iteration order is unspecified). |

Notes:
- `Map.union` is right-biased (keys from the right map override).
- `Map.update` applies only when the key exists; otherwise it is a no-op.

### Set

| Function | Explanation |
| --- | --- |
| **Set.empty**<br><pre><code>`Set a`</code></pre> | Creates an empty set. |
| **Set.size** set<br><pre><code>`Set a -> Int`</code></pre> | Returns the number of elements. |
| **Set.has** value set<br><pre><code>`a -> Set a -> Bool`</code></pre> | Returns whether `value` is present. |
| **Set.insert** value set<br><pre><code>`a -> Set a -> Set a`</code></pre> | Returns a new set with `value` inserted. |
| **Set.remove** value set<br><pre><code>`a -> Set a -> Set a`</code></pre> | Returns a new set without `value`. |
| **Set.union** left right<br><pre><code>`Set a -> Set a -> Set a`</code></pre> | Returns the union of two sets. |
| **Set.intersection** left right<br><pre><code>`Set a -> Set a -> Set a`</code></pre> | Returns elements common to both sets. |
| **Set.difference** left right<br><pre><code>`Set a -> Set a -> Set a`</code></pre> | Returns elements in `left` not in `right`. |
| **Set.fromList** values<br><pre><code>`List a -> Set a`</code></pre> | Builds a set from a list. |
| **Set.toList** set<br><pre><code>`Set a -> List a`</code></pre> | Converts a set into a list. |
| **Set.contains** value set<br><pre><code>`a -> Set a -> Bool`</code></pre> | Alias of `Set.has`. |

### Queue / Deque

| Function | Explanation |
| --- | --- |
| **Queue.empty**<br><pre><code>`Queue a`</code></pre> | Creates an empty queue. |
| **Queue.enqueue** value queue<br><pre><code>`a -> Queue a -> Queue a`</code></pre> | Adds `value` to the back. |
| **Queue.dequeue** queue<br><pre><code>`Queue a -> Option (a, Queue a)`</code></pre> | Removes and returns the front value and remaining queue. |
| **Queue.peek** queue<br><pre><code>`Queue a -> Option a`</code></pre> | Returns the front value without removing it. |
| **Deque.empty**<br><pre><code>`Deque a`</code></pre> | Creates an empty deque. |
| **Deque.pushFront** value deque<br><pre><code>`a -> Deque a -> Deque a`</code></pre> | Adds `value` to the front. |
| **Deque.pushBack** value deque<br><pre><code>`a -> Deque a -> Deque a`</code></pre> | Adds `value` to the back. |
| **Deque.popFront** deque<br><pre><code>`Deque a -> Option (a, Deque a)`</code></pre> | Removes and returns the front value and rest. |
| **Deque.popBack** deque<br><pre><code>`Deque a -> Option (a, Deque a)`</code></pre> | Removes and returns the back value and rest. |
| **Deque.peekFront** deque<br><pre><code>`Deque a -> Option a`</code></pre> | Returns the front value without removing it. |
| **Deque.peekBack** deque<br><pre><code>`Deque a -> Option a`</code></pre> | Returns the back value without removing it. |

### Heap / PriorityQueue

| Function | Explanation |
| --- | --- |
| **Heap.empty**<br><pre><code>`Heap a`</code></pre> | Creates an empty heap. |
| **Heap.push** value heap<br><pre><code>`a -> Heap a -> Heap a`</code></pre> | Inserts `value` into the heap. |
| **Heap.popMin** heap<br><pre><code>`Heap a -> Option (a, Heap a)`</code></pre> | Removes and returns the smallest value and remaining heap. |
| **Heap.peekMin** heap<br><pre><code>`Heap a -> Option a`</code></pre> | Returns the smallest value without removing it. |

`Heap` ordering is determined by `Ord` for the element type.
