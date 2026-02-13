# Concurrency Domain

The `Concurrency` domain unlocks the power of doing multiple things at once.

It provides **Fibers** (lightweight threads) and **Channels** for safe communication. Whether you're fetching two APIs in parallel or building a background worker, this domain gives you the high-level tools (`par`, `scope`) to write concurrent code that doesn't melt your brain.

<<< ../../snippets/from_md/05_stdlib/03_system/30_concurrency/block_01.aivi{aivi}

## Types

<<< ../../snippets/from_md/05_stdlib/03_system/30_concurrency/block_02.aivi{aivi}

## Functions

| Function | Explanation |
| --- | --- |
| **par** left right<br><pre><code>`Effect E A -> Effect E B -> Effect E (A, B)`</code></pre> | Runs both effects concurrently and returns both results; fails if either fails. |
| **scope** run<br><pre><code>`(Scope -> Effect E A) -> Effect E A`</code></pre> | Creates a structured concurrency scope and runs `run` (current `Scope` is `Unit`). |

## Channels

Channels provide a mechanism for synchronization and communication between concurrent fibers.

### `make`

| Function | Explanation |
| --- | --- |
| **make** sample<br><pre><code>`A -> Effect E (Sender A, Receiver A)`</code></pre> | Creates a new channel and returns `(Sender, Receiver)`. |

### `send`

| Function | Explanation |
| --- | --- |
| **send** sender value<br><pre><code>`Sender A -> A -> Effect E Unit`</code></pre> | Sends `value` to the channel; may block if buffered and full or no receiver is ready. |

### `recv`

| Function | Explanation |
| --- | --- |
| **recv** receiver<br><pre><code>`Receiver A -> Effect E (Result A ChannelError)`</code></pre> | Waits for the next value; returns `Ok value` or `Err Closed`. |

### `close`

| Function | Explanation |
| --- | --- |
| **close** sender<br><pre><code>`Sender A -> Effect E Unit`</code></pre> | Closes the channel from the sender side; receivers observe `Err Closed`. |
