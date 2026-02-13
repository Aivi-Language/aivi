# Memory Management

AIVI uses **Reference Counting (RC)** as its primary memory management strategy. This decision aligns with the language's immutable-by-default nature and the desire for deterministic resource cleanup.

## Core Strategy

*   **Immutable Values**: Most values in AIVI (`List`, `Record`, `String`, `BigInt`) are immutable and shared via reference counting (`Arc<T>`).
*   **No Tracing GC**: There is no "stop-the-world" tracing garbage collector. Memory is reclaimed immediately when the last reference is dropped.
*   **Determinism**: Resource cleanup (file handles, network sockets) is deterministic and tied to the scope of the value owning the resource.

## Cycle Handling

While strict immutability prevents cycles in data structures, cycles can still arise in:
1.  **Recursive Closures/Thunks**: A function that refers to itself needs access to its own environment.
2.  **Mutable Cells** (if added): Explicit mutable references could form cycles.

### Recursive Definitions

To handle recursive definitions without leaking memory, the runtime allows for "weakening" or manually breaking the cycle in the environment of recursive thunks.

1.  **Knotted Environments**: Recursive thunks hold a reference to an environment that eventually points back to them.
2.  **Breaking**: When a recursive scope exits, the runtime may explicitly break these cycles to ensure the `Env` and its contained `Thunk`s are dropped.

### Future Work

*   **Weak References**: We may introduce explicit weak references for advanced users implementing cyclic data structures.
*   **Cycle Detection**: A lightweight cycle detector (e.g., "purple" coloring) might be added if manual cycle breaking proves insufficient for complex cases.
