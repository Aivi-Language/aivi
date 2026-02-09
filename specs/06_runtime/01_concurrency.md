# Runtime: Concurrency and Communication

AIVI implements a **Structural Concurrency** model by default, ensuring that the lifecycle of concurrent tasks is strictly bound to the lexical scope that created them.


## 20.1 Structural Concurrency

The `scope` block acts as a boundary for concurrent operations. All tasks spawned within a scope must complete or be cancelled before the scope exits.

### Primitives

- `scope { ... }` — Defines a lifetime boundary for child tasks.
- `par { f; g }` — Runs `f` and `g` in parallel, waiting for both. If one fails/cancels, the other is cancelled.
- `race { f; g }` — Runs `f` and `g` in parallel, returning the first result and cancelling the loser.

### Explicit Detachment

When a task must outlive its creator (e.g., a background daemon), it must be explicitly detached from the structural tree.

```aivi
effect {
  spawnDetached (logger.run ())
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
  tx.send "hello"
  
  // Receiving (returns Result for closed channels)
  msg = rx.recv () ? {
    Ok value => value
    Err Closed => "Channel closed"
  }
  
  // Closing
  tx.close ()
}
```


## 20.3 Non-deterministic Selection (select)

The `select` block allows waiting on multiple channel operations simultaneously.

```aivi
next = select {
  rx1.recv () => msg => handle1 msg
  rx2.recv () => msg => handle2 msg
  timeout 1s  => _   => handleTimeout ()
}
```

The first operation to succeed is chosen; all other pending operations in the block are cancelled.
