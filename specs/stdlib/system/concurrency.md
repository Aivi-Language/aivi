# Concurrency Domain

<!-- quick-info: {"kind":"module","name":"aivi.concurrency"} -->
The `Concurrency` domain unlocks the power of doing multiple things at once.

It provides **Fibers** (lightweight threads) and **Channels** for safe communication. Whether you're fetching two APIs in parallel or building a background worker, this domain gives you the high-level tools (`par`, `scope`) to write concurrent code that doesn't melt your brain.

<!-- /quick-info -->
<div class="import-badge">use aivi.concurrency</div>

<<< ../../snippets/from_md/stdlib/system/concurrency/concurrency_domain.aivi{aivi}

## Types

<<< ../../snippets/from_md/stdlib/system/concurrency/types.aivi{aivi}

## Functions

| Function | Explanation |
| --- | --- |
| **par** left right<br><code>Effect E A -> Effect E B -> Effect E (A, B)</code> | Runs both effects concurrently and returns both results; fails if either fails. |
| **race** left right<br><code>Effect E A -> Effect E A -> Effect E A</code> | Runs two effects and resolves with the first completion; cancels the loser. |
| **scope** run<br><code>(Scope -> Effect E A) -> Effect E A</code> | Creates a structured concurrency scope. The `Scope` handle is passed to `run` and can be used to spawn child tasks that are guaranteed to complete (or be cancelled) before `scope` returns. |
| **spawn** effect<br><code>Effect Text A -> Effect Text (Task A)</code> | Starts an effect in the background and returns a `Task` handle with `join`, `cancel`, and `isCancelled`. |
| **timeoutWith** ms timeoutError effect<br><code>Int -> E -> Effect E A -> Effect E A</code> | Races an effect with a timer and fails with `timeoutError` when the timer wins. |
| **retry** attempts effect<br><code>Int -> Effect E A -> Effect E A</code> | Retries a failing effect up to `attempts` times. |
| **sleep** millis<br><code>Int -> Effect Text Unit</code> | Suspends the current effect for `millis` milliseconds. |

Code reference: `crates/aivi/src/stdlib/concurrency.rs` — `aivi.concurrency` exports `par`, `scope`, `make`, `send`, `recv`, `close`

## Channels

Channels provide a mechanism for synchronization and communication between concurrent fibers.

### `make`

| Function | Explanation |
| --- | --- |
| **make** sample<br><code>A -> Effect E (Sender A, Receiver A)</code> | Creates a new channel and returns `(Sender, Receiver)`. |
| **makeBounded** capacity<br><code>Int -> Effect E (Sender A, Receiver A)</code> | Creates a bounded channel with backpressure when the buffer is full. |

### `send`

| Function | Explanation |
| --- | --- |
| **send** sender value<br><code>Sender A -> A -> Effect E Unit</code> | Sends `value` to the channel; may block if buffered and full or no receiver is ready. |

### `recv`

| Function | Explanation |
| --- | --- |
| **recv** receiver<br><code>Receiver A -> Effect E (Result ChannelError A)</code> | Waits for the next value; returns `Ok value` or `Err Closed`. |

### `close`

| Function | Explanation |
| --- | --- |
| **close** sender<br><code>Sender A -> Effect E Unit</code> | Closes the channel from the sender side; receivers observe `Err Closed`. |
