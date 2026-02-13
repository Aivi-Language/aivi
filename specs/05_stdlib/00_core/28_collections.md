# Collections Domain

The `Collections` domain is your toolbox for structured data. While `List` is great for simple sequences, real-world software needs more. Whether you need to look up users by their ID (Map), keep a list of unique tags (Set), or process tasks by priority (Heap), this domain provides persistent data structures designed for functional code.

## Overview

<<< ../../snippets/from_md/05_stdlib/00_core/28_collections/block_01.aivi{aivi}

## v1.0 Scope

- **Map/Dict**: persistent ordered maps (AVL/Red-Black) and/or HashMaps (HAMT).
- **Set**: persistent sets corresponding to map types.
- **Queue/Deque**: efficient FIFO/LIFO structures.
- **Heap/PriorityQueue**.

## Literals and Merging (v1.0)

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

The `Collections` domain provides `++` as a right-biased merge for `Map` and union for `Set`.

<<< ../../snippets/from_md/05_stdlib/00_core/28_collections/block_05.aivi{aivi}

## Core API (v1.0)

The following functions are required for v1.0 implementations. Exact module layout may vary, but names and behavior should match.

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
