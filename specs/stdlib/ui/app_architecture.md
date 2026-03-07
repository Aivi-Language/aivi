# GTK App Architecture

> **Status: Phase 2 command/subscription surface specified, runtime subset landed**  
> `gtkApp` remains the single blessed host for standard GTK applications. The runtime now hosts `AppStep`, `subscriptions`, timer subscriptions, direct-message source subscriptions, the memoized `computed` helper for reactive reads over committed models, and the command subset needed for the blessed loop via the concrete helpers `commandNone`, `commandBatch`, `commandEmit`, `commandPerform`, `commandAfter`, and `commandCancel`. The richer mapper-based `Command.perform` / `Command.startTask` target shape below is still the direction of travel. Forms and validation layer on top of the same host via [`aivi.ui.forms`](./forms.md), rather than introducing a second UI architecture.

<!-- quick-info: {"kind":"topic","name":"gtk app architecture"} -->
AIVI GTK applications have one public architecture: `Model` / `View` / `Msg` / `Update` hosted by `gtkApp`, extended with typed commands and subscriptions, paired with lightweight form helpers from `aivi.ui.forms`, and compatible with the pure reactive layer defined in [Reactive Dataflow](./reactive_dataflow.md). Lower-level primitives such as `signalStream`, `buildFromNode`, and `reconcileNode` remain escape hatches and implementation building blocks, not competing top-level app patterns.
<!-- /quick-info -->

## Blessed shape

Every standard single-window GTK app should be organized around these parts:

1. **`Model`** — the full state needed to render the current window.
2. **`view : Model -> GtkNode`** — a pure projection from state to GTK node tree.
3. **`Msg`** — a closed ADT describing domain events that matter to the app.
4. **`toMsg : GtkSignalEvent -> Option Msg`** — the adapter for the app's primary GTK signal stream.
5. **`update : Msg -> Model -> Effect GtkError (AppStep Model Msg)`** — the steady-state transition function.
6. **`subscriptions : Model -> List (Subscription Msg)`** — long-lived event sources derived from the current model.
7. **`gtkApp`** — the runtime host that wires startup, rendering, event ingestion, commands, subscriptions, and reconciliation together.

The command/subscription extension is additive: it keeps the same mental model and adds an official place for post-update work and non-GTK event feeds.
Reactive values are additive in the same way: they are pure memoized helpers layered between committed model snapshots and code that reads them, not a second update loop.

## Core types

The Phase 2 architecture introduces these conceptual types:

```aivi
CommandKey = Text
SubscriptionKey = Text

AppStep model msg = {
  model: model,
  commands: List (Command msg)
}
```

`AppStep` means:

- `model` is the next committed model that will be rendered,
- `commands` are post-update effects to start **after** that model becomes current,
- an empty command list means "render only; do no extra work".

The existing runtime shape:

```aivi
update : msg -> model -> Effect GtkError model
```

is the compatibility shorthand for:

```aivi
update : msg -> model -> Effect GtkError (AppStep model msg)
update = msg => model => pure { model, commands: [] }
```

Likewise, code that does not need long-lived external feeds can still use the implemented shorthand `subscriptions = noSubscriptions` (equivalent to `subscriptions = _ => []`).

## `gtkApp` host surface

The specified public shape of `gtkApp` is:

```aivi
gtkApp : {
  id:            Text,
  title:         Text,
  size:          (Int, Int),
  model:         s,
  onStart:       AppId -> WindowId -> Effect GtkError Unit,
  subscriptions: s -> List (Subscription msg),
  view:          s -> GtkNode,
  toMsg:         GtkSignalEvent -> Option msg,
  update:        msg -> s -> Effect GtkError (AppStep s msg)
} -> Effect GtkError Unit
```

Two compatibility notes are part of this spec:

- code already written against the implemented subset may keep `update : msg -> s -> Effect GtkError s`; the host lifts it to `AppStep` with no commands,
- `gtkAppFull` remains a deprecated compatibility shim for advanced window flags and update-time handles; new docs and new apps should continue to teach only `gtkApp`.

## Runtime flow

`gtkApp` executes the architecture in this order:

1. initialize GTK and create the application/window,
2. run `onStart` once,
3. build and attach the initial `view model`,
4. open the primary `signalStream`,
5. start the initial `subscriptions model`,
6. translate each incoming event into `Msg`,
7. call `update`,
8. commit the returned `model`,
9. assign fresh revisions to changed source snapshots and invalidate affected computed values,
10. evaluate the new `view` against the committed model (dirty computed values recalculate lazily on first read),
11. reconcile the new `view`,
12. diff `subscriptions` against the new model and update them,
13. launch the returned `commands`.

This preserves one official mental model:

- **signals, timers, and external feeds produce `Msg`**
- **`update` computes the next `Model` and requested work**
- **reactive invalidation happens from that committed `Model`, never from ambient observers**
- **`view` re-renders from that committed `Model` and pulls derived data synchronously**
- **`gtkApp` hosts the side effects after the state transition**

Commands and subscriptions never replace `Msg`; they only decide **where messages come from** and **what work starts after a message**.

## Reactive dataflow layer

Phase 4 adds a pure reactive layer on top of the committed model described above; see [Reactive Dataflow](./reactive_dataflow.md) for the full semantics.

- authoritative source snapshots remain ordinary model fields,
- plain helper functions are derived values and may recompute whenever read,
- named computed signals are memoized host-tracked projections with stable identity,
- commands and subscriptions still own all effectful work and can influence reactive values only by emitting `Msg` that update model sources.

This keeps the architecture single-loop: reactive values help reuse pure work inside `view`, `subscriptions`, and command construction, but they never mutate the model on their own and they never start effects implicitly.

In the current shipped milestone, GTK sigils may read reactive helpers directly in common binding positions:

```aivi
titleText = computed "counter.title" (state => "Count: {toText state.count}")
visibleRows = signal (state => state.rows)

view = _ =>
  ~<gtk>
    <GtkBox orientation="vertical">
      <GtkLabel label={titleText} />
      <each items={visibleRows} as={row}>
        <GtkLabel label={row.name} />
      </each>
    </GtkBox>
  </gtk>
```

Those bindings are evaluated against the committed model inside `gtkApp`; outside the sigil, use `readSignal` or ordinary function application.

## Forms and validation

Form-heavy screens stay inside the same GTK app loop:

- keep editable input in `Field A` values from [`aivi.ui.forms`](./forms.md),
- map `GtkInputChanged` messages to `setValue`,
- map `GtkFocusOut` messages to `touch`,
- flip a `submitted: Bool` flag in your model on submit,
- render inline errors with `visibleErrors`,
- build the typed submit payload with the existing `Validation` applicative.

This keeps field state, validation, commands, and subscriptions in one vocabulary: GTK signals still become `Msg`, `update` still owns the state transition, and later command work consumes the validated result instead of bypassing the app architecture.

## Commands

`Command msg` is a pure description of work that `gtkApp` interprets after a successful `update`.

### Standard command constructors

| Constructor | Meaning | Capability alignment |
| --- | --- | --- |
| `Command.none` | No post-update work. | none |
| `Command.batch cmds` | Run several commands as one step. | union of nested commands |
| `Command.emit msg` | Enqueue a synthetic message immediately after the current update commits. | none |
| `Command.perform { run, onOk, onError }` | Run a one-shot effect and map its result back into `Msg`. | whatever capabilities `run` requires |
| `Command.after { key, millis, msg }` | One-shot timer that emits `msg` after `millis`. | `clock.sleep`; keyed so it can be cancelled/replaced |
| `Command.startTask { key, run, onProgress, onOk, onError, onCancelled }` | Start cancellable background work with typed progress events. | capabilities of `run`, plus `cancellation.propagate` for keyed cancellation |
| `Command.cancel key` | Cancel the currently running keyed task or keyed one-shot timer. | `cancellation.propagate` |

`Command.perform` is for work with one terminal outcome. `Command.startTask` is for work that may outlive the current turn and report progress before finishing.

### Background task shape

`Command.startTask` is specified around a typed progress channel:

```aivi
Command.startTask {
  key:         "search",
  run:         progress => searchCatalog progress state.query,
  onProgress:  SearchProgress,
  onOk:        SearchFinished,
  onError:     SearchFailed,
  onCancelled: Some SearchCancelled
}
```

Where `run` has the conceptual shape:

```aivi
run : Sender progress -> Effect err a with { ... }
```

Semantics:

- `run` executes as a child task hosted by `gtkApp`,
- each `send progress value` becomes `onProgress value`,
- exactly one terminal path wins: `onOk`, `onError`, or `onCancelled`,
- after a terminal message is emitted, later progress from that task key is ignored,
- starting a new task with the same `key` is equivalent to `Command.cancel key` followed by the new start.

Commands are **post-update**: the model returned by `update` becomes current before any command result can enqueue its own `Msg`.

## Subscriptions

`Subscription msg` describes a long-lived event source derived from the current model.

### Standard subscription constructors

| Constructor | Meaning | Capability alignment |
| --- | --- | --- |
| `Subscription.none` | No external feed. | none |
| `Subscription.batch subs` | Merge several subscriptions. | union of nested subscriptions |
| `Subscription.every { key, millis, tag }` | Repeating timer; each tick emits `tag`. | `clock.schedule` |
| `Subscription.source { key, open, onEvent, onError, onClosed }` | Acquire a typed receiver/resource and forward values into `Msg`. | capabilities required by `open` |

`Subscription.source` is the general bridge for:

- additional GTK signal streams,
- background daemons that expose a receiver,
- file watchers or network/event-stream clients,
- any external source that has setup/teardown and pushes typed values over time.

The conceptual shape is:

```aivi
Subscription.source {
  key:      "file-watch",
  open:     watchConfigFile "./config.json",
  onEvent:  ConfigChanged,
  onError:  Some ConfigWatchFailed,
  onClosed: None
}
```

Where `open` has the conceptual shape:

```aivi
open : Resource err (Receiver event) with { ... }
```

Using `Resource` here is deliberate: replacing or removing a subscription must trigger structured cleanup automatically.

### Subscription diffing

`gtkApp` evaluates `subscriptions` after the initial render and after every committed update. It diffs the old and new sets by `SubscriptionKey`:

- **same key, same shape** → keep the existing subscription running,
- **same key, changed shape** → cancel and clean up the old subscription, then start the new one,
- **removed key** → cancel and clean up the old subscription,
- **new key** → acquire and start the new subscription.

This makes subscriptions a function of the model rather than imperative setup code.

## Timers, signals, background work, and external feeds

### Timers

- Use `Command.after` for a **one-shot** delayed message that was requested by `update`.
- Use `Subscription.every` for a **repeating** tick that should remain active while the model says it is needed.

This split matches intent: one-shot timers are transient post-update work, while repeating timers are long-lived event sources.

### Signals

The app's primary GTK signal ingestion still flows through:

```aivi
toMsg : GtkSignalEvent -> Option Msg
```

hosted by the built-in `signalStream` inside `gtkApp`. In other words, the standard signal flow is already an implicit subscription.

When an app needs extra GTK-driven feeds beyond that primary stream, it should use `Subscription.source` around a lower-level signal receiver rather than introduce a second event loop.

### Background work

Use `Command.perform` for quick one-shot work and `Command.startTask` for cancellable long-running jobs. The latter is the blessed way to integrate:

- search/indexing,
- network fetches with progress,
- database sync,
- image processing,
- any work that should keep the UI responsive while reporting intermediate state.

### External event sources

Anything that can be represented as `Resource err (Receiver event)` fits the subscription model:

- file-watch APIs,
- long-lived HTTP/event streams,
- database notification channels,
- IPC or device feeds,
- custom library abstractions built on channels.

This keeps `signalStream` and `channel.fold` as the low-level primitives while giving `gtkApp` one declarative home for hosted feeds.

## Cancellation and progress semantics

The command/subscription model reuses the language-wide capability and cancellation story; it does **not** invent a UI-only variant.

### Hosting model

`gtkApp` owns a structured concurrency scope for:

- keyed background commands,
- keyed one-shot timers,
- active subscriptions.

When the window/app shuts down, `gtkApp` cancels that scope. All child tasks and subscription resources must therefore honor the same guarantees as ordinary AIVI concurrency:

- cancellation is observed at effect bind points,
- resource finalizers still run,
- cleanup stays cancellation-protected automatically.

### Keyed cancellation

- `Command.cancel key` is a no-op when no matching command is running.
- Removing or replacing a subscription key is defined as cancellation of the old subscription instance.
- Starting a keyed task or keyed timer replaces the previous instance of that key.
- An `onCancelled` message, when provided, fires at most once for a given task instance and only after cancellation has been observed and cleanup has begun.

### Progress delivery

Progress is intentionally message-driven rather than ambient:

- progress values are app-defined and statically typed,
- order is preserved **per command key** in the order `run` sent them,
- no total ordering is promised across different keys or between different subscriptions,
- once a terminal outcome wins, later progress for that key is ignored.

Apps should therefore model progress explicitly in `Msg` and `Model`, just like any other domain event.

## Startup hooks and options

### `onStart`

`onStart : AppId -> WindowId -> Effect GtkError Unit` remains the blessed startup boundary for one-time host setup such as:

- registering application CSS with `appSetCss`,
- registering actions or shortcuts,
- applying initial window configuration that is not state-driven,
- bridging temporary escape hatches while the command/subscription runtime surface is landing.

`onStart` is **not** a second steady-state update loop. Repeating timers, background feeds, and most runtime work should live in subscriptions or commands once that surface is available.

### Advanced options

The public architecture still does **not** define a second blessed top-level API for uncommon window flags or update-time access to GTK handles. `gtkAppFull` remains a deprecated compatibility shim rather than a second path.

Use `gtkAppFull` only when blocked by functionality the blessed architecture does not yet surface, such as:

- uncommon window flags like `decorated` / `hideOnClose`,
- legacy code that still requires `AppId` or `WindowId` inside `update`.

## Relation to lower-level primitives

### `signalStream`

`signalStream` remains the low-level signal receiver used underneath `gtkApp`. `Subscription.source` is the declarative bridge back up to the blessed architecture whenever an app needs more than the built-in primary signal flow.

### `reconcileNode`

`reconcileNode` is still the rendering primitive used after each successful update. Commands and subscriptions do not change its role; they only affect when new messages enter the loop.

### `buildFromNode`

`buildFromNode` remains the initial mount primitive for low-level/manual hosting code. Standard apps should continue to let `gtkApp` own mounting and reconciliation.

### `scope`, `spawn`, and channels

The blessed architecture is deliberately layered on the same lower-level primitives exposed elsewhere:

- `Command.startTask` is the structured UI-facing wrapper over hosted background work,
- `Subscription.source` is the declarative wrapper over `Receiver`-driven feeds,
- `signalStream` stays available for custom loops, experiments, and advanced multi-window flows.

## Example

```aivi
Msg
  = QueryChanged Text
  | SearchProgress Int
  | SearchFinished (List Text)
  | SearchFailed Text
  | SearchCancelled
  | Tick

subscriptions : Model -> List (Subscription Msg)
subscriptions = state =>
  if state.pollingEnabled
    then [
      Subscription.every {
        key: "clock",
        millis: 1000,
        tag: Tick
      }
    ]
    else []

update : Msg -> Model -> Effect GtkError (AppStep Model Msg)
update = msg => state =>
  msg match
    | QueryChanged txt =>
        pure {
          model: state <| { query: txt, searching: True, progress: 0 },
          commands: [
            Command.startTask {
              key: "search",
              run: progress => searchCatalog progress txt,
              onProgress: SearchProgress,
              onOk: SearchFinished,
              onError: SearchFailed,
              onCancelled: Some SearchCancelled
            }
          ]
        }
    | Tick =>
        pure {
          model: state,
          commands: []
        }
    | SearchProgress n =>
        pure {
          model: state <| { progress: n },
          commands: []
        }
    | SearchFinished results =>
        pure {
          model: state <| { searching: False, results },
          commands: []
        }
    | SearchFailed err =>
        pure {
          model: state <| { searching: False, error: Some err },
          commands: []
        }
    | SearchCancelled =>
        pure {
          model: state <| { searching: False },
          commands: []
        }
```

## Proof surfaces

The quick-reference layer is now propagated in `AIVI_LANGUAGE.md`, and the flagship proof surfaces for this architecture are:

- `demos/snake.aivi` for the blessed `gtkApp` + `Subscription.every` story,
- `integration-tests/stdlib/aivi/ui/gtk4/gtk4.aivi` for `AppStep`, command, and subscription helpers,
- `integration-tests/stdlib/aivi/ui/forms/` for the lightweight forms and validation flow.
