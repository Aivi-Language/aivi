# Collections Domain

The `Collections` domain expands the core data structures beyond `List` and `Vector`.

## Why this exists

While `List` is great for simple sequences, efficient software often requires more sophisticated data structures like HashMaps, Sets, or Priority Queues for performance and semantic clarity.

## Overview

```aivi
import aivi.std.core.collections use { Map, Set }

let scores = Map.empty()
    |> Map.insert("Alice", 100)
    |> Map.insert("Bob", 95)

if scores |> Map.has("Alice") {
    print("Alice is present")
}
```

## Goals for v1.0

- **Map/Dict**: Persistent ordered maps (AVL or Red-Black Tree) and/or HashMaps (HAMT).
- **Set**: Persistent sets corresponding to map types.
- **Queue/Deque**: Efficient FIFO/LIFO structures.
- **Heap/PriorityQueue**.
