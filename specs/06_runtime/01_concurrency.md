# Runtime: Concurrency and Communication

AIVI implements a **Structural Concurrency** model by default, ensuring that the lifecycle of concurrent tasks is strictly bound to the lexical scope that created them.


## 20.1 Structural Concurrency

Structural concurrency means: concurrent tasks are children of the scope that spawned them. When the scope ends, all children have either completed or are cancelled (with cleanup).

### Primitives

For parser simplicity in v0.1, these are described as **standard library APIs** (taking thunks / effects), even if future surface syntax adds dedicated blocks:

- `concurrent.scope : Effect E A -> Effect E A`
- `concurrent.par   : Effect E A -> Effect E B -> Effect E (A, B)`
- `concurrent.race  : Effect E A -> Effect E A -> Effect E A`

### Explicit Detachment

When a task must outlive its creator (e.g., a background daemon), it must be explicitly detached from the structural tree.

<<< ../snippets/from_md/06_runtime/01_concurrency/block_01.aivi{aivi}


## 20.2 Communication: Channels

AIVI uses typed CSP-style channels for communication between concurrent tasks.

### Types

<<< ../snippets/from_md/06_runtime/01_concurrency/block_02.aivi{aivi}

### Channel Operations

<<< ../snippets/from_md/06_runtime/01_concurrency/block_03.aivi{aivi}


## 20.3 Non-deterministic Selection (select)

Selecting across multiple concurrent operations is essential for channel-based code.

<<< ../snippets/from_md/06_runtime/01_concurrency/block_04.aivi{aivi}

The first operation to succeed is chosen; all other pending operations in the block are cancelled.
