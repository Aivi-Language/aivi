# Concurrency Domain

The `Concurrency` domain provides primitives for concurrent execution and communication.

```aivi
use std.Concurrency
```

## Functions

### `par`

```aivi
par : Effect E A -> Effect E B -> Effect E (A, B)
```

Executes two effects concurrently and returns their results as a tuple. If either effect fails, the entire operation fails.

```aivi
(left, right) <- concurrent.par (print "left") (print "right")
```

### `scope`

```aivi
scope : (Scope -> Effect E A) -> Effect E A
```

Creates a structured concurrency scope. Spawning fibers within this scope ensures they are joined or cancelled when the scope exits. (Note: `par` is often a higher-level convenience over `scope`).

## Channels

Channels provide a mechanism for synchronization and communication between concurrent fibers.

### `make`

```aivi
make : A -> Effect E (Sender A, Receiver A)
```

Creates a new channel for values of type `A`. Returns a pair of `Sender` and `Receiver`.

### `send`

```aivi
send : Sender A -> A -> Effect E Unit
```

Sends a value to the channel. This operation may block if the channel is full (if buffered) or until a receiver is ready (if unbuffered).

### `recv`

```aivi
recv : Receiver A -> Effect E (Result A ChannelError)
```

Receives a value from the channel. Returns `Ok value` if successful, or `Err Closed` if the channel is closed.
