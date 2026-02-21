# Mutable Map Module

<!-- quick-info: {"kind":"module","name":"aivi.mutableMap"} -->
The `aivi.mutableMap` module exposes the `MutableMap` record for effect-scoped
mutable maps. It is designed for local, controlled mutation inside `do Effect { ... }`
blocks, with `freeze` producing an immutable `Map` snapshot.
<!-- /quick-info -->
<div class="import-badge">use aivi.mutableMap</div>

## Overview

<<< ../../snippets/from_md/stdlib/core/mutable_map/overview.aivi{aivi}

## Type

`MutableMap k v` is a mutable container whose contents are a `Map k v`.

Notes:
- All operations are effectful and must run inside `do Effect { ... }`.
- Keys follow the same constraints as `Map` keys.
- Use `freeze` to snapshot the current contents as an immutable `Map`.

## Core API (v0.1)

| Function | Explanation |
| --- | --- |
| **MutableMap.create** map<br><pre><code>`Map k v -> Effect e (MutableMap k v)`</code></pre> | Creates a mutable map initialized from `map`. |
| **MutableMap.empty** unit<br><pre><code>`Unit -> Effect e (MutableMap k v)`</code></pre> | Creates an empty mutable map. |
| **MutableMap.get** key mmap<br><pre><code>`k -> MutableMap k v -> Effect e (Option v)`</code></pre> | Returns `Some value` for `key`, or `None`. |
| **MutableMap.getOrElse** key default mmap<br><pre><code>`k -> v -> MutableMap k v -> Effect e v`</code></pre> | Returns the value for `key`, or `default` when missing. |
| **MutableMap.insert** key value mmap<br><pre><code>`k -> v -> MutableMap k v -> Effect e Unit`</code></pre> | Inserts or overwrites `key`. |
| **MutableMap.remove** key mmap<br><pre><code>`k -> MutableMap k v -> Effect e Unit`</code></pre> | Removes `key` when present. |
| **MutableMap.has** key mmap<br><pre><code>`k -> MutableMap k v -> Effect e Bool`</code></pre> | Returns whether `key` is present. |
| **MutableMap.size** mmap<br><pre><code>`MutableMap k v -> Effect e Int`</code></pre> | Returns the number of entries. |
| **MutableMap.freeze** mmap<br><pre><code>`MutableMap k v -> Effect e (Map k v)`</code></pre> | Snapshots the mutable map into an immutable `Map`. |
| **MutableMap.keys** mmap<br><pre><code>`MutableMap k v -> Effect e (List k)`</code></pre> | Returns all keys (iteration order is unspecified). |
| **MutableMap.values** mmap<br><pre><code>`MutableMap k v -> Effect e (List v)`</code></pre> | Returns all values (iteration order is unspecified). |
| **MutableMap.modify** f mmap<br><pre><code>`(Map k v -> Map k v) -> MutableMap k v -> Effect e Unit`</code></pre> | Replaces the contents with `f` applied to the current snapshot. |

Notes:
- `keys` and `values` return results in an unspecified order.
- `modify` applies a pure function to a snapshot and replaces the map with the result.
