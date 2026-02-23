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
| **par** left right<br><pre><code>`Effect E A -> Effect E B -> Effect E (A, B)`</code></pre> | Runs both effects concurrently and returns both results; fails if either fails. |
| **race** left right<br><pre><code>`Effect E A -> Effect E A -> Effect E A`</code></pre> | Runs two effects and resolves with the first completion; cancels the loser. |
| **scope** run<br><pre><code>`(Scope -> Effect E A) -> Effect E A`</code></pre> | Creates a structured concurrency scope. The `Scope` handle is passed to `run` and can be used to spawn child tasks that are guaranteed to complete (or be cancelled) before `scope` returns. |
| **spawn** effect<br><pre><code>`Effect Text A -> Effect Text (Task A)`</code></pre> | Starts an effect in the background and returns a `Task` handle with `join`, `cancel`, and `isCancelled`. |
| **timeoutWith** ms timeoutError effect<br><pre><code>`Int -> E -> Effect E A -> Effect E A`</code></pre> | Races an effect with a timer and fails with `timeoutError` when the timer wins. |
| **retry** attempts effect<br><pre><code>`Int -> Effect E A -> Effect E A`</code></pre> | Retries a failing effect up to `attempts` times. |
| **sleep** millis<br><pre><code>`Int -> Effect Text Unit`</code></pre> | Suspends the current effect for `millis` milliseconds. |

Code reference: `crates/aivi/src/stdlib/concurrency.rs` — `aivi.concurrency` exports `par`, `scope`, `make`, `send`, `recv`, `close`

## Channels

Channels provide a mechanism for synchronization and communication between concurrent fibers.

### `make`

| Function | Explanation |
| --- | --- |
| **make** sample<br><pre><code>`A -> Effect E (Sender A, Receiver A)`</code></pre> | Creates a new channel and returns `(Sender, Receiver)`. |
| **makeBounded** capacity<br><pre><code>`Int -> Effect E (Sender A, Receiver A)`</code></pre> | Creates a bounded channel with backpressure when the buffer is full. |

### `send`

| Function | Explanation |
| --- | --- |
| **send** sender value<br><pre><code>`Sender A -> A -> Effect E Unit`</code></pre> | Sends `value` to the channel; may block if buffered and full or no receiver is ready. |

### `recv`

| Function | Explanation |
| --- | --- |
| **recv** receiver<br><pre><code>`Receiver A -> Effect E (Result ChannelError A)`</code></pre> | Waits for the next value; returns `Ok value` or `Err Closed`. |

### `close`

| Function | Explanation |
| --- | --- |
| **close** sender<br><pre><code>`Sender A -> Effect E Unit`</code></pre> | Closes the channel from the sender side; receivers observe `Err Closed`. |
