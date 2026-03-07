# GTK App Architecture

<!-- quick-info: {"kind":"topic","name":"gtk app architecture"} -->
AIVI GTK applications work best with one clear loop: keep authoritative state in a `Model`, turn widget events into `Msg`, update the model in one place, and let `gtkApp` render the next `GtkNode` tree. Commands and subscriptions fit into that same loop so timers, background work, and external feeds stay predictable.
<!-- /quick-info -->

If you want the broad overview first, read [Native GTK & libadwaita Apps](./native_gtk_apps.md). This page is the detailed guide to the moving parts behind `gtkApp`.

## The recommended shape

Most single-window GTK apps can be organized around these pieces:

1. **`Model`** — the complete state needed to render the current window.
2. **`view : Model -> GtkNode`** — a pure function that describes the widget tree for the current state.
3. **`Msg`** — a closed set of events your app cares about.
4. **`toMsg : GtkSignalEvent -> Option Msg`** — the adapter from raw GTK events to your app's messages. For common constructor bindings, `toMsg: auto` can derive this from the current view tree.
5. **`update : Msg -> Model -> Effect GtkError (AppStep Model Msg)`** — the function that decides the next model and any follow-up work.
6. **`subscriptions : Model -> List (Subscription Msg)`** — long-lived event sources that should stay active while the current model says they are needed.
7. **`gtkApp`** — the host that wires startup, rendering, event ingestion, reconciliation, commands, and subscriptions together.

If you are familiar with Elm, Redux-style reducers, or unidirectional UI architectures, the mental model is similar: input becomes a message, the message updates state, and the UI is redrawn from that state.

## Start here

Before reading the full API surface, keep this short checklist in mind:

1. put all authoritative screen state in `Model`
2. turn widget input into a small `Msg` type
3. let `update` compute the next `Model`
4. let `view` redraw from that model
5. add commands or subscriptions only when the screen needs effects

Everything else on this page explains how `gtkApp` hosts that loop.

## Core types

The architecture is built around these conceptual types:

```aivi
CommandKey = Text
SubscriptionKey = Text

AppStep model msg = {
  model: model,
  commands: List (Command msg)
}
```

`AppStep` means:

- `model` is the next committed model,
- `commands` are follow-up effects to start **after** that model becomes current,
- an empty command list means “render only; no extra work right now”.

Some apps do not need explicit commands yet. In that case, the simpler update shape still works as shorthand:

```aivi
update : msg -> model -> Effect GtkError model
```

Conceptually, that is equivalent to:

```aivi
update : msg -> model -> Effect GtkError (AppStep model msg)
update = msg => model => pure {
  model,
  commands: []
}
```

`appStep` and `appStepWith` are just shorthand constructors for that same record shape. Use them when they help; direct `{ model, commands }` records are equally valid.

Likewise, if an app has no long-lived external feeds, `subscriptions = noSubscriptions` is equivalent to `subscriptions = _ => []`.

## `gtkApp` in one sentence

`gtkApp` is the host that keeps everything in sync:

- it starts GTK,
- mounts the initial widget tree,
- listens for GTK signal events,
- turns those events into `Msg`,
- runs `update`,
- re-renders and reconciles the UI,
- refreshes subscriptions,
- launches any commands returned by `update`.

The specified public shape is:

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

For older code or very simple examples, the host can also lift `update : msg -> s -> Effect GtkError s` into an `AppStep` automatically.

For common constructor-style signal bindings such as `onInput={ ProjectNameChanged }` and `onClick={ Save }`, `gtkApp` also ships `toMsg: auto`. `auto` works best when a signal is either unique in the current view or attached to a widget with an `id="..."` name. Keep an explicit `toMsg` when routing depends on richer event matching or when several unnamed widgets emit the same signal.

## How one app turn works

A normal turn through `gtkApp` looks like this:

1. initialize GTK and create the application/window,
2. run `onStart` once,
3. build and attach the initial `view model`,
4. open the primary `signalStream`,
5. start the initial `subscriptions model`,
6. translate each incoming event into `Msg`,
7. call `update`,
8. commit the returned `model`,
9. invalidate any reactive values affected by changed source snapshots,
10. evaluate `view` against the new committed model,
11. reconcile the new `view`,
12. diff subscriptions against the new model,
13. launch the returned commands.

The important idea is that there is still **one official loop**:

- GTK signals, timers, and external feeds produce `Msg`,
- `update` computes the next `Model`,
- `view` redraws from that committed model,
- `gtkApp` handles the side effects after the state transition.

## Reactive dataflow fits inside the same loop

Reactive helpers such as `signal` and `computed` are useful when you want named pure derived values. They do **not** replace the app loop.

- source snapshots still live in the model,
- commands and subscriptions still own IO and timers,
- reactive helpers only derive reusable pure values from committed model state.

A small example:

```aivi
titleText = computed "counter.title" (state =>
  // Memoize a title that may be read more than once in the same render.
  "Count: {toText state.count}"
)

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

Inside the GTK sigil, `gtkApp` reads those helpers against the current committed model for you. Outside the sigil, use `readSignal` or ordinary function application. If you want the beginner-friendly introduction to these helpers first, read [Reactive Signals](./reactive_signals.md#start-simple-helper-first-then-signal-then-computed).

## Forms and validation stay in the same architecture

Form-heavy screens do not need a second UI framework. The normal pattern is:

- keep editable input in `Field A` values from [`aivi.ui.forms`](./forms.md),
- map `GtkInputChanged` to `setValue`,
- map `GtkFocusOut` to `touch`,
- keep a `submitted: Bool` flag in the model,
- render inline errors with `visibleErrors`,
- build the final typed submit payload with `Validation`.

This keeps form state, validation, and effects in one place instead of splitting logic between widgets and ad-hoc callbacks.

## Commands

`Command msg` is a pure description of work that `gtkApp` interprets after a successful update.

### Standard command constructors

| Constructor | Meaning | When to reach for it |
| --- | --- | --- |
| `Command.none` | No follow-up work. | The message only changes local state. |
| `Command.batch cmds` | Run several commands as one step. | A single update should trigger multiple follow-ups. |
| `Command.emit msg` | Enqueue another message immediately after the current update commits. | A message should expand into another app-level message. |
| `Command.perform { run, onOk, onError }` | Run a one-shot effect and map its result back into `Msg`. | Saving, loading, or any single-result task. |
| `Command.after { key, millis, msg }` | Emit `msg` once after a delay. | Short in-process delays such as clearing a toast. |
| `Command.startTask { key, run, onProgress, onOk, onError, onCancelled }` | Start cancellable background work with typed progress events. | Search, sync, imports, exports, uploads. |
| `Command.cancel key` | Cancel a keyed task or keyed one-shot timer. | The model says earlier work is no longer relevant. |

`Command.perform` is for work with one terminal outcome. `Command.startTask` is for work that may report progress before it finishes.

### Background task shape

```aivi
Command.startTask {
  key: "search",
  run: progress => searchCatalog progress state.query,
  onProgress: SearchProgress,
  onOk: SearchFinished,
  onError: SearchFailed,
  onCancelled: Some SearchCancelled
}
```

Conceptually, `run` has this shape:

```aivi
run : Sender progress -> Effect err a with { ... }
```

Semantics:

- `run` executes as a child task hosted by `gtkApp`,
- each `send progress value` becomes `onProgress value`,
- exactly one terminal path wins: `onOk`, `onError`, or `onCancelled`,
- after a terminal message, later progress for that task key is ignored,
- starting a new task with the same key replaces the old one.

Commands are always **post-update**. The new model becomes current before a command can feed a new `Msg` back into the app.

## Subscriptions

`Subscription msg` describes a long-lived event source derived from the current model.

### Standard subscription constructors

| Constructor | Meaning | Typical use |
| --- | --- | --- |
| `Subscription.none` | No external feed. | Screens with only direct user input. |
| `Subscription.batch subs` | Merge several subscriptions. | Combine timers, streams, and watchers. |
| `Subscription.every { key, millis, tag }` | Repeating timer that emits `tag`. | Clocks, heartbeats, auto-refresh. |
| `Subscription.source { key, open, onEvent, onError, onClosed }` | Acquire a typed receiver/resource and forward values into `Msg`. | File watchers, network streams, device feeds. |

`Subscription.source` is the general bridge for any long-lived producer that can be opened, cleaned up, and read over time.

```aivi
Subscription.source {
  key: "file-watch",
  open: watchConfigFile "./config.json",
  onEvent: ConfigChanged,
  onError: Some ConfigWatchFailed,
  onClosed: None
}
```

Conceptually, `open` has this shape:

```aivi
open : Resource err (Receiver event) with { ... }
```

That `Resource` boundary matters because replacing or removing a subscription should trigger structured cleanup automatically.

### Subscription diffing

`gtkApp` evaluates `subscriptions` after the initial render and again after every committed update. It compares the old and new sets by `SubscriptionKey`:

- **same key, same shape** → keep the existing subscription running,
- **same key, changed shape** → cancel and replace it,
- **removed key** → cancel and clean it up,
- **new key** → start it.

In practice, that means subscriptions are a function of state, not imperative setup code scattered across your app.

## Choosing between timers, signals, background work, and the scheduler

### Timers

- Use `Command.after` for a **one-shot** delayed message requested by `update`.
- Use `Subscription.every` for a **repeating** tick that should stay alive while the current model needs it.
- Use [`aivi.chronos.scheduler`](/stdlib/chronos/scheduler) when the timing must survive app shutdown, be coordinated with workers, or follow durable plans such as cron and retry rules.

### GTK signals

Your app's primary GTK signal flow still goes through:

```aivi
toMsg : GtkSignalEvent -> Option Msg
```

That is already hosted by the built-in `signalStream` inside `gtkApp`. If you need extra GTK-driven feeds beyond that primary stream, wrap them in `Subscription.source` instead of starting a second top-level loop.

### Background work

Use `Command.perform` for quick one-shot work and `Command.startTask` for cancellable long-running jobs. Typical examples include:

- search and indexing,
- network fetches with progress,
- database sync,
- image processing,
- any operation that should keep the UI responsive while it works.

### External feeds

Anything that can be represented as `Resource err (Receiver event)` fits the subscription model:

- file-watch APIs,
- long-lived HTTP or event streams,
- database notification channels,
- IPC or device feeds,
- custom channel-based library integrations.

## Cancellation and progress semantics

`gtkApp` owns a structured-concurrency scope for:

- keyed background commands,
- keyed one-shot timers,
- active subscriptions.

When the window or app shuts down, that scope is cancelled. Child tasks and subscription resources therefore follow the same guarantees as the rest of AIVI concurrency:

- cancellation is observed at effect bind points,
- resource finalizers still run,
- cleanup stays cancellation-protected automatically.

A few rules are especially useful in practice:

- `Command.cancel key` is a no-op when nothing with that key is running,
- removing or replacing a subscription key cancels the earlier instance,
- starting a keyed task or keyed timer replaces the previous instance,
- when provided, `onCancelled` fires at most once for a task instance,
- progress ordering is preserved **per command key**, not globally across all tasks.

Apps should model progress explicitly in `Msg` and `Model`, just like any other domain event.

## Startup hooks and advanced options

### `onStart`

`onStart : AppId -> WindowId -> Effect GtkError Unit` is the place for one-time setup such as:

- registering application CSS with `appSetCss`,
- registering actions or shortcuts,
- applying initial window configuration that is not state-driven,
- bridging temporary low-level setup before the rest of the app is running.

`onStart` is not a second steady-state update loop. Repeating timers, ongoing background feeds, and normal application work should still live in subscriptions or commands.

### Advanced window setup

`gtkApp` is the only high-level host API. Closing the primary window ends the host loop by default, while `windowSetHideOnClose win True` keeps the loop alive and hides the window instead. When an app needs extra one-time window configuration such as `windowSetDecorated` or `windowSetHideOnClose`, do that work in `onStart`.

## Relation to lower-level primitives

AIVI still exposes the lower-level building blocks behind this architecture:

- `signalStream` — the raw GTK signal receiver,
- `reconcileNode` — the widget-tree patching primitive,
- `buildFromNode` — the initial mount primitive,
- scopes, tasks, channels, and receivers — the concurrency tools commands and subscriptions are built on.

Reach for them when you need custom hosting, experiments, or multi-window flows. For standard single-window apps, let `gtkApp` own the event loop.

## Example 1: local state plus a repeating timer

This first example keeps one query string in the model and adds a repeating timer only while polling is enabled:

```aivi
Model = {
  query: Text
  pollingEnabled: Bool
  secondsVisible: Int
}

Msg = QueryChanged Text | Tick

subscriptions : Model -> List (Subscription Msg)
subscriptions = model =>
  if model.pollingEnabled
    then [
      Subscription.every {
        key: "clock",
        millis: 1000,
        tag: Tick
      }
    ]
    else []

update : Msg -> Model -> Effect GtkError (AppStep Model Msg)
update = msg => model =>
  pure (
    msg match
      | QueryChanged newQuery =>
          {
            model: model <| { query: newQuery }
            commands: []
          }
      | Tick =>
          {
            model: model <| { secondsVisible: model.secondsVisible + 1 }
            commands: []
          }
  )
```

## Example 2: add background work without changing the loop

When a message should launch asynchronous work, return a command from `update`:

```aivi
Model = {
  query: Text
  searching: Bool
  progress: Int
  results: List Text
  error: Option Text
}

Msg
  = QueryChanged Text
  | SearchProgress Int
  | SearchFinished (List Text)
  | SearchFailed Text
  | SearchCancelled

update = msg => model =>
  pure (
    msg match
      | QueryChanged newQuery =>
          {
            model: model <| { query: newQuery, searching: True, progress: 0 }
            commands: [
              Command.startTask {
                key: "search"
                run: progress => searchCatalog progress newQuery
                onProgress: SearchProgress
                onOk: SearchFinished
                onError: SearchFailed
                onCancelled: Some SearchCancelled
              }
            ]
          }
      | SearchProgress n =>
          {
            model: model <| { progress: n }
            commands: []
          }
      | SearchFinished results =>
          {
            model: model <| { searching: False, results }
            commands: []
          }
      | SearchFailed err =>
          {
            model: model <| { searching: False, error: Some err }
            commands: []
          }
      | SearchCancelled =>
          {
            model: model <| { searching: False }
            commands: []
          }
  )
```

The important part is that nothing creates a second event loop. Even background work still feeds results back as ordinary `Msg` values.

## See it in action

If you want to compare this guide with real project code, these are useful follow-ups:

- `demos/snake.aivi` for `gtkApp` with a repeating subscription,
- `integration-tests/stdlib/aivi/ui/gtk4/gtk4.aivi` for `AppStep`, command, and subscription helpers,
- `integration-tests/stdlib/aivi/ui/forms/` for form state and validation flows.
