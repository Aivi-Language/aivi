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

```aivi
effect {
  concurrent.spawnDetached logger.run
}
```


## 20.2 Communication: Channels

AIVI uses typed CSP-style channels for communication between concurrent tasks.

### Types

```aivi
Send A // Capability to send values of type A
Recv A // Capability to receive values of type A
```

### Channel Operations

```aivi
effect {
  (tx, rx) = channel.make ()
  
  // Sending
  channel.send tx "hello"
  
  // Receiving (returns Result for closed channels)
  msg <- channel.recv rx or "Channel closed"
  
  // Closing
  channel.close tx
}
```


## 20.3 Non-deterministic Selection (select)

Selecting across multiple concurrent operations is essential for channel-based code.

```aivi
// Proposed surface syntax (future):
// next = select {
//   rx1.recv () => msg => handle1 msg
//   rx2.recv () => msg => handle2 msg
//   timeout 1s  => _   => handleTimeout ()
// }
```

The first operation to succeed is chosen; all other pending operations in the block are cancelled.
