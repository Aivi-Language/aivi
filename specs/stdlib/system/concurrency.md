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

## Running work concurrently

| Function | What it does | When it helps |
| --- | --- | --- |
| **par** left right<br><code>Effect E A -> Effect E B -> Effect E (A, B)</code> | Runs both effects at the same time and returns both results. If either side fails, the combined effect fails. | Fetching two independent resources in parallel. |
| **race** left right<br><code>Effect E A -> Effect E A -> Effect E A</code> | Starts both effects and keeps the one that completes first. The other one is cancelled. | Trying two mirrors or fallback services and taking the fastest response. |
| **scope** run<br><code>(Scope -> Effect E A) -> Effect E A</code> | Creates a structured scope for child tasks. Work started inside the scope is guaranteed to finish or be cancelled before the scope returns. | Any operation that starts background work you do not want to leak. |
| **spawn** effect<br><code>Effect Text A -> Effect Text (Task A)</code> | Starts an effect in the background and gives you a `Task` handle that can be joined or cancelled. | Long-running work such as polling, indexing, or background imports. |
| **timeoutWith** ms timeoutError effect<br><code>Int -> E -> Effect E A -> Effect E A</code> | Runs `effect` with a time limit and fails with `timeoutError` if the timer wins. | Network calls or external processes that should not hang forever. |
| **retry** attempts effect<br><code>Int -> Effect E A -> Effect E A</code> | Re-runs a failing effect up to `attempts` times. | Temporary failures such as flaky I/O. |
| **sleep** millis<br><code>Int -> Effect Text Unit</code> | Pauses the current effect for a number of milliseconds. | Backoff, scheduling, and simple polling loops. |

## Channels

Channels let tasks pass values to each other safely.
One side sends values, the other side receives them, and the type system keeps both sides aligned.

### Creating channels

| Function | What it does |
| --- | --- |
| **make** sample<br><code>A -> Effect E (Sender A, Receiver A)</code> | Creates a new channel and returns a sender/receiver pair. |
| **makeBounded** capacity<br><code>Int -> Effect E (Sender A, Receiver A)</code> | Creates a channel with a bounded buffer. When the buffer is full, sends wait until space becomes available. |

### Sending and receiving

| Function | What it does |
| --- | --- |
| **send** sender value<br><code>Sender A -> A -> Effect E Unit</code> | Sends `value` into the channel. |
| **recv** receiver<br><code>Receiver A -> Effect E (Result ChannelError A)</code> | Waits for the next value. Returns `Ok value`, or `Err Closed` after the channel is closed and drained. |
| **close** sender<br><code>Sender A -> Effect E Unit</code> | Closes the sending side so receivers can finish cleanly. |

### Consuming a stream of values

| Function | What it does | Why you might prefer it |
| --- | --- | --- |
| **fold** init fn receiver<br><code>S -> (S -> A -> Effect E S) -> Receiver A -> Effect E S</code> | Reads values until the channel closes, threading state through each step, and returns the final state. | Good for reducers, aggregations, and event loops. |
| **forEach** receiver fn<br><code>Receiver A -> (A -> Effect E Unit) -> Effect E Unit</code> | Reads every value and runs an effectful action on it until the channel closes. | Good for consumers that only perform side effects. |

## Structured concurrency in practice

Structured concurrency means a child task belongs to the scope that created it.
That gives you predictable cleanup:

- when the scope finishes, its child tasks are not left running accidentally,
- if a child fails, the surrounding operation can fail in a controlled way,
- and cleanup code still runs when work is cancelled.

Use `scope` when a task should live only as long as the surrounding operation.
Use explicit detachment only when a task truly must outlive its creator.

A tiny mental model:

- `spawn` starts background work,
- `Task` lets you join or cancel it,
- `scope` makes sure child work is cleaned up with its parent operation.

### Explicit detachment

<<< ../../snippets/from_md/runtime/concurrency/explicit_detachment.aivi{aivi}

## Waiting on whichever event happens first

Some workflows need to react to the first available result, message, or timeout.
Selection helpers model that pattern directly. “Select” here means “wait for whichever event happens first, then continue with that winner”.

<<< ../../snippets/from_md/runtime/concurrency/non_deterministic_selection_select.aivi{aivi}

The first successful operation wins, and the remaining pending work is cancelled.

## Capabilities

- `sleep` uses clock access.
- Task creation, racing, cancellation, and timeouts rely on concurrency and cancellation support in the runtime.
- Resource cleanup still runs when a task is cancelled.

## How this fits GTK apps

The same concurrency model is used by the GTK app architecture.
Commands and subscriptions are built on ordinary tasks and channels, which means UI code gets the same guarantees around cancellation, ordering, and cleanup as non-UI code.
