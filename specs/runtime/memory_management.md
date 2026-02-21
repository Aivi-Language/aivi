# Memory Management

AIVI uses **Reference Counting (RC)** as its primary memory management strategy. This decision aligns with the language's immutable-by-default nature and the desire for deterministic resource cleanup.

## Core Strategy

*   **Immutable Values**: Most values in AIVI (`List`, `Record`, `Text`, `BigInt`) are immutable and shared via reference counting.
*   **No Tracing GC**: There is no "stop-the-world" tracing garbage collector. Memory is reclaimed immediately when the last reference is dropped.
*   **Determinism**: Resource cleanup (file handles, network sockets) is deterministic and tied to the scope of the value owning the resource (see [Resources](../02_syntax/15_resources.md)).

## Cycle Handling

While strict immutability prevents cycles in data structures, cycles can still arise in:
1.  **Recursive Closures**: A function that refers to itself needs access to its own environment, creating a reference cycle.

### Recursive Definitions

To handle recursive definitions without leaking memory, the runtime uses one of the following strategies (implementation-defined):

1.  **Weak Back-References**: The self-reference in a recursive closure is held as a weak reference that does not prevent deallocation.
2.  **Scope-Based Cycle Breaking**: When a recursive scope exits, the runtime explicitly breaks the cycle to ensure the closure and its environment are freed.

The choice of strategy is transparent to the programmer   recursive definitions simply work without memory leaks.

### Future Work

*   **Weak References**: Explicit weak references may be introduced for advanced users implementing cyclic data structures.
*   **Cycle Detection**: A lightweight cycle detector may be added if the above strategies prove insufficient for complex cases.
