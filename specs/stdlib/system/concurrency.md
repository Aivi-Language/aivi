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

## Capability mapping (Phase 1 surface)

- `sleep` → `clock.sleep`
- wall-clock reads such as `Instant.now` (see `aivi.chronos.instant`) → `clock.now`
- `scope`, `spawn`, `race`, `select`, explicit task cancellation → `cancellation.propagate`
- `timeoutWith` → `clock.sleep` + `cancellation.propagate`
- ordinary resource cleanup remains cancellation-protected without requiring an explicit `cancellation.mask` clause

## GTK app architecture alignment

The blessed GTK command/subscription model is intentionally layered on this concurrency domain rather than inventing a UI-only scheduler:

- `Command.startTask` is a structured child task hosted by `gtkApp`,
- `Command.cancel` and subscription replacement/removal reuse ordinary task cancellation,
- `Subscription.source` is the declarative UI wrapper over a `Resource` that yields a `Receiver`,
- typed progress reporting is modeled by sending app-defined values over a channel and mapping them back into `Msg`.

This means the same guarantees apply in UI code:

- cancellation is cooperative at effect bind points,
- finalizers still run on task/subscription shutdown,
- progress ordering is preserved per producer channel,
- shutting down the host scope cancels all child work.

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

### `fold`

| Function | Explanation |
| --- | --- |
| **fold** init fn receiver<br><code>S -> (S -> A -> Effect E S) -> Receiver A -> Effect E S</code> | Consumes values from the channel, threading state through each step. Returns the final state when the channel closes. Eliminates the need for manual `recv` + `Err`/`Ok` matching + `recurse`. |

### `forEach`

| Function | Explanation |
| --- | --- |
| **forEach** receiver fn<br><code>Receiver A -> (A -> Effect E Unit) -> Effect E Unit</code> | Consumes all values from the channel, running an effectful action on each. Returns `Unit` when the channel closes. |

## Structural Concurrency Model

Structural concurrency means: concurrent tasks are children of the scope that spawned them. When the scope ends, all children have either completed or been cancelled (with cleanup).

- `scope` bounds task lifetime to a lexical scope.
- Tasks spawned with `spawn` inside a scope are joined before `scope` returns.
- Errors propagate: if any child fails, the scope fails.

### Explicit Detachment

When a task must outlive its creator (e.g., a background daemon), it must be explicitly detached from the structural tree.

<<< ../../snippets/from_md/runtime/concurrency/explicit_detachment.aivi{aivi}

## Non-Deterministic Selection (`select`)

Selecting across multiple concurrent operations is essential for channel-based code.

<<< ../../snippets/from_md/runtime/concurrency/non_deterministic_selection_select.aivi{aivi}

The first operation to succeed is chosen; all other pending operations are cancelled.
