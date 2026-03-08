# Concurrency Domain

<!-- quick-info: {"kind":"module","name":"aivi.concurrency"} -->
The `Concurrency` domain gives you the tools to run independent work at the same time without sharing mutable state.

It provides lightweight tasks, channels for communication, and structured concurrency helpers so background work stays tied to the part of your program that created it.

<!-- /quick-info -->
<div class="import-badge">use aivi.concurrency</div>

<<< ../../snippets/from_md/stdlib/system/concurrency/concurrency_domain.aivi{aivi}

## When to reach for it

Use `aivi.concurrency` when a program needs to:

- wait for several slow operations at once,
- race two strategies and keep whichever finishes first,
- communicate safely between tasks,
- or stop child work automatically when a parent operation ends.

If you are new to functional concurrency, the key idea is simple: instead of sharing mutable variables between threads, start tasks and let them talk through typed channels.

## Types

<<< ../../snippets/from_md/stdlib/system/concurrency/types.aivi{aivi}

Two other shapes matter when you read the API below:

- `Send A` and `Recv A` are the two ends of a channel that carries values of type `A`.
- `spawn` returns a **task handle record**, not a separate named `Task` type. Its fields are `join`, `cancel`, and `isCancelled`.
- `make sample` uses `sample` only to fix the channel element type. It does **not** put that sample value into the channel.

## Running work concurrently

| Function | What it does | When it helps |
| --- | --- | --- |
| **par** left right<br><code>Effect E A -> Effect E B -> Effect E (A, B)</code> | Runs both effects at the same time and returns both results. If either side fails, the combined effect fails. | Fetching two independent resources in parallel. |
| **race** left right<br><code>Effect E A -> Effect E A -> Effect E A</code> | Starts both effects and keeps the one that completes first. The other one is cancelled. | Trying two mirrors or fallback services and taking the fastest response. |
| **scope** run<br><code>(Scope -> Effect E A) -> Effect E A</code> | Creates a structured scope for child tasks. When the scope ends, child work is cancelled with it instead of silently outliving the parent operation. | Any operation that starts background work you do not want to leak. |
| **spawn** effect<br><code>Effect Text A -> Effect Text { join : Effect Text A, cancel : Effect Text Unit, isCancelled : Effect Text Bool }</code> | Starts an effect in the background and returns a task handle record. You can wait for the result with `join`, stop it with `cancel`, or inspect its state with `isCancelled`. | Long-running work such as polling, indexing, or background imports. |
| **timeoutWith** ms timeoutError effect<br><code>Int -> E -> Effect E A -> Effect E A</code> | Runs `effect` with a time limit and fails with `timeoutError` if the timer wins. | Network calls or external processes that should not hang forever. |
| **retry** attempts effect<br><code>Int -> Effect E A -> Effect E A</code> | Re-runs a failing effect up to `attempts` times. Use a positive attempt count. | Temporary failures such as flaky I/O. |
| **sleep** millis<br><code>Int -> Effect Text Unit</code> | Pauses the current effect for a number of milliseconds. | Backoff, scheduling, and simple polling loops. |

### The task handle returned by `spawn`

This module uses an ordinary record as the task handle:

| Field | Type | What it does |
| --- | --- | --- |
| `join` | `Effect Text A` | Waits for the task to finish and returns its value, or fails with the same error as the task. |
| `cancel` | `Effect Text Unit` | Requests cancellation. Cleanup code inside the task still runs. |
| `isCancelled` | `Effect Text Bool` | Reports whether cancellation has been requested. |

## Channels

Channels let tasks pass values to each other safely.
One side sends values, the other side receives them, and the type system keeps both sides aligned.

### Creating channels

| Function | What it does |
| --- | --- |
| **make** sample<br><code>A -> Effect E (Send A, Recv A)</code> | Creates a new channel and returns a send/receive pair. The `sample` value only fixes the element type. |
| **makeBounded** capacity<br><code>Int -> Effect E (Send A, Recv A)</code> | Creates a channel with a bounded buffer. When the buffer is full, `send` waits until a receiver consumes a value. `capacity` must be greater than `0`. |

### Sending and receiving

| Function | What it does |
| --- | --- |
| **send** sender value<br><code>Send A -> A -> Effect E Unit</code> | Sends `value` into the channel. |
| **recv** receiver<br><code>Recv A -> Effect E (Result ChannelError A)</code> | Waits for the next value. Returns `Ok value` for each message, or `Err Closed` once the channel is closed and drained. In this module, `ChannelError` only has the `Closed` case. |
| **close** sender<br><code>Send A -> Effect E Unit</code> | Closes the sending side so receivers can finish cleanly. |

### Consuming a stream of values

| Function | What it does | Why you might prefer it |
| --- | --- | --- |
| **fold** init fn receiver<br><code>S -> (S -> A -> Effect E S) -> Recv A -> Effect E S</code> | Reads values until the channel closes, threading state through each step, and returns the final state. | Good for reducers, aggregations, and event loops. |
| **forEach** receiver fn<br><code>Recv A -> (A -> Effect E Unit) -> Effect E Unit</code> | Reads every value and runs an effectful action on it until the channel closes. | Good for consumers that only perform side effects. |

## Structured concurrency in practice

Structured concurrency means a child task belongs to the scope that created it.
That gives you predictable cleanup:

- when the scope finishes, its child tasks receive cancellation instead of silently running on,
- if a child fails, the surrounding operation can fail in a controlled way,
- and cleanup code still runs when work is cancelled.

Use `scope` when a task should live only as long as the surrounding operation.
Use `spawn` inside that scope when you need overlapping work and still want one clear owner for its lifetime.

Today `aivi.concurrency` documents the structured path (`scope` + `spawn`) rather than a stable detached-task helper.
If work truly must outlive the current operation, hand it off to a longer-lived owner such as an app host, service process, or subscription system.

A tiny mental model:

- `spawn` starts background work,
- the returned task handle record lets you `join` or `cancel` it,
- `scope` ties child work to its parent operation.

## Waiting on whichever event happens first

Some workflows need to react to the first available result, message, or timeout.
Today the stable surface uses `race` when two effects compete and `timeoutWith` when one of the competitors is simply the clock.

```aivi
do Effect {
  slow = _ => do Effect {
    sleep 50
    pure "left"
  }
  fast = _ => do Effect {
    sleep 0
    pure "right"
  }

  fastest <- race (slow Unit) (fast Unit)
  guarded <- timeoutWith 100 "timeout" (pure 9)
  pure (fastest, guarded)
}
```

In the example above, `race` keeps only the faster branch, and `timeoutWith` cancels the protected effect if the timer wins.
A dedicated `select { ... }` surface form is not part of `aivi.concurrency` today.

## Capabilities

- `sleep` uses clock access.
- Task creation, racing, cancellation, timeouts, and retries rely on concurrency and cancellation support in the runtime.
- Channel creation and delivery rely on runtime channel support.
- Resource cleanup still runs when a task is cancelled.

## How this fits GTK apps

The same concurrency ideas show up in [GTK App Architecture](../ui/app_architecture.md) and [`aivi.ui.gtk4`](../ui/gtk4.md).
Commands and subscriptions are built on ordinary effects and channels, which means UI code gets the same guarantees around cancellation, ordering, and cleanup as non-UI code.
