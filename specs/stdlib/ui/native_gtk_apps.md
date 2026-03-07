# Native GTK & libadwaita Apps

<!-- quick-info: {"kind":"topic","name":"native gtk apps"} -->
AIVI's main desktop-app story is native GTK4 and libadwaita software. Use `gtkApp` for the app loop, `~<gtk>...</gtk>` for the widget tree, GTK signals for user input, commands and subscriptions for effects, and `signal` or `computed` for reusable pure view logic.
<!-- /quick-info -->

This page is the broad guide. If you want the details afterward, follow up with [`aivi.ui.gtk4`](./gtk4.md), [GTK App Architecture](./app_architecture.md), [Reactive Signals](./reactive_signals.md), [Reactive Dataflow](./reactive_dataflow.md), and [`aivi.ui.forms`](./forms.md).

## What GTK and libadwaita mean in AIVI

GTK4 is the native widget toolkit: windows, buttons, entries, lists, signals, accessibility, drawing, and the event loop. libadwaita builds on GTK and adds GNOME-style application structure and adaptive widgets such as clamps, header bars, and preference rows.

In AIVI, both are used through the same runtime module and the same GTK sigil:

- `Gtk*` tags map to GTK widgets,
- `Adw*` tags map to libadwaita widgets,
- both render inside `~<gtk>...</gtk>`,
- both participate in the same `gtkApp` loop.

So you do not have to choose between “GTK apps” and “libadwaita apps”. AIVI apps can mix both in the same tree.

## The recommended app shape

Most native apps in AIVI use this structure:

| Piece | Role |
| --- | --- |
| `Model` | The complete state needed to render the current UI. |
| `view : Model -> GtkNode` | Pure function that describes GTK and libadwaita widgets. |
| `Msg` | Closed set of app events that matter to the domain. |
| `toMsg : GtkSignalEvent -> Option Msg` | Maps low-level GTK events into app messages. |
| `update : Msg -> Model -> Effect GtkError (AppStep Model Msg)` | Computes the next model and optional follow-up work. |
| `subscriptions : Model -> List (Subscription Msg)` | Describes long-lived event sources such as timers or streams. |
| `gtkApp` | Hosts startup, rendering, signal ingestion, reconciliation, commands, and subscriptions. |

The core flow is:

```text
widget
  -> GtkSignalEvent
  -> toMsg
  -> Msg
  -> update
  -> Model
  -> view
  -> reconcileNode
```

Commands and subscriptions feed the same loop by producing more `Msg` values. Reactive helpers such as `signal` and `computed` stay inside that model-driven flow.

## Putting the pieces together

### 1. Build the UI with GTK and libadwaita widgets

Use `~<gtk>` and prefer shorthand widget tags:

```aivi
~<gtk>
  <AdwClamp maximumSize="480">
    <GtkBox orientation="vertical" spacing="12">
      <GtkLabel label="Project Settings" cssClass="title-2" />
      <GtkEntry id="nameInput" placeholderText="Project name" />
      <GtkButton id="saveBtn" label="Save" />
    </GtkBox>
  </AdwClamp>
</gtk>
```

`Gtk*` and `Adw*` tags are both first-class here. The sigil lowers to a `GtkNode` tree, and `gtkApp` takes care of mounting and patching that tree for standard apps.

### 2. Turn GTK signals into domain messages

Signal sugar such as `onClick={ Save }` or `onInput={ NameChanged }` is the clearest way to bind widget events. Those events arrive as typed `GtkSignalEvent` values, and `toMsg` decides which ones matter to your app.

This separation keeps the code readable:

- GTK knows about clicks, focus, and text edits,
- your app knows about `Save`, `NameChanged`, `SearchFinished`, and other domain messages.

### 3. Keep authoritative state in the model

The model is the single source of truth. Widgets display state and emit input events; they do not own the real application data.

This is especially important for:

- text entry values,
- selection state,
- loading and error state,
- timer-driven UI,
- data coming from background work,
- form validation state.

### 4. Use commands and subscriptions for effectful work

Use commands for follow-up work triggered by a message:

- save a file,
- schedule a one-shot delay,
- emit another message,
- start background work.

Use subscriptions for long-lived sources that should stay active while the model says they are needed:

- repeating timers,
- extra signal streams,
- file watchers,
- network streams,
- device or IPC feeds.

### 5. Know the two meanings of “signal”

In GTK docs, “signal” can mean two different things:

- **GTK signals** are widget events such as clicks, input changes, and focus changes,
- **reactive signals** are pure derived readers created with `signal` or `computed`.

The first kind flows **into** your app as `GtkSignalEvent`. The second kind is read **inside** your app from the committed model.

### 6. Use reactive dataflow for pure derived values

Keep authoritative state in the model, then derive reusable data with ordinary helpers, `signal`, or `computed`.

- use a plain helper when the logic is simple,
- use `signal` when a named reader makes the code clearer,
- use `computed` when the same pure derivation should be memoized and reused.

Good examples include filtered lists, derived labels, grouped rows, and expensive view-only projections.

### 7. Use app-local timers for live UI, use the scheduler for durable plans

Use `commandAfter` and `subscriptionEvery` when the timing only matters while the app is running.

Use [`aivi.chronos.scheduler`](/stdlib/chronos/scheduler) when the work should survive restarts, be coordinated with workers, or follow durable rules such as cron, retry, lease, and tenant concurrency limits.

```aivi
use aivi.chronos.scheduler

invoiceRollup = {
  // This plan value can be stored or handed to a worker later.
  key: planKey "invoice-rollup" 2026-01-01T00:00:00Z
  tenantId: "tenant-apac"
  trigger: once 2026-01-01T00:00:00Z
  scheduledAt: 2026-01-01T00:00:00Z
  attempt: 0
  status: Planned
}
```

A GTK app will often create or inspect scheduler values as part of normal app logic, while a worker or backend process executes them later.

### 8. Add forms on top of the app loop, not beside it

For form-heavy screens, `aivi.ui.forms` keeps the same architecture:

- store each editable value as `Field A` in the model,
- map `GtkInputChanged` to `setValue`,
- map `GtkFocusOut` to `touch`,
- render inline errors with `visibleErrors`,
- build the final typed payload with `Validation`.

There is no separate form runtime and no hidden widget-owned field state.

## Guided example

The example below shows the pieces working together:

- `AdwClamp` provides a simple libadwaita layout container,
- `GtkEntry` and `GtkButton` emit signals,
- `toMsg: auto` derives the simple signal routing from the current view,
- `SubscriptionEvery` keeps a timer alive,
- `CommandAfter` schedules a delayed follow-up message,
- direct `{ model, commands }` records keep `update` readable.

```aivi
use aivi
use aivi.text
use aivi.ui.gtk4

Model = {
  projectName: Text
  secondsSinceSave: Int
  status: Text
}

Msg
  = ProjectNameChanged Text
  | Save
  | Tick
  | ClearStatus

initialModel : Model
initialModel = {
  projectName: ""
  secondsSinceSave: 0
  status: "Waiting for changes"
}

headline : Model -> Text
headline = state =>
  if state.projectName == ""
    then "Project Settings"
    else "Project Settings · {state.projectName}"

statusText : Model -> Text
statusText = state => state.status

view : Model -> GtkNode
view = state =>
  ~<gtk>
    <AdwClamp maximumSize="480">
      <GtkBox
        orientation="vertical"
        spacing="12"
        marginTop="24"
        marginBottom="24"
        marginStart="24"
        marginEnd="24"
      >
        <GtkLabel label={headline state} cssClass="title-2" />
        <GtkEntry
          text={state.projectName}
          placeholderText="Project name"
          onInput={ ProjectNameChanged }
        />
        <GtkButton label="Save" onClick={ Save } />
        <GtkLabel label={statusText state} />
        <GtkLabel label={"Seconds since save: {toText state.secondsSinceSave}"} />
      </GtkBox>
    </AdwClamp>
  </gtk>

subscriptions : Model -> List (Subscription Msg)
subscriptions = _ => [
  SubscriptionEvery {
    key: "clock"
    millis: 1000
    tag: Tick
  }
]

renameProject : Text -> Model -> AppStep Model Msg
renameProject = txt state => {
  model: state <| { projectName: txt }
  commands: []
}

saveProject : Model -> AppStep Model Msg
saveProject = state => {
  model: state <| { secondsSinceSave: 0, status: "Saved" }
  commands: [
    CommandAfter {
      key: "clear-status"
      millis: 2000
      msg: ClearStatus
    }
  ]
}

tick : Model -> AppStep Model Msg
tick = state => {
  model: state <| { secondsSinceSave: state.secondsSinceSave + 1 }
  commands: []
}

clearStatus : Model -> AppStep Model Msg
clearStatus = state => {
  model: state <| { status: "Waiting for changes" }
  commands: []
}

update : Msg -> Model -> Effect GtkError (AppStep Model Msg)
update = msg => state =>
  pure (
    msg match
      | ProjectNameChanged txt => renameProject txt state
      | Save                   => saveProject state
      | Tick                   => tick state
      | ClearStatus            => clearStatus state
  )

main : Effect GtkError Unit
main = gtkApp {
  id: "docs.projectSettings"
  title: "Project Settings"
  size: (640, 480)
  model: initialModel
  onStart: _ _ => pure Unit
  subscriptions: subscriptions
  view: view
  toMsg: auto
  update: update
}
```

When a screen has several unnamed widgets producing the same signal, either give them `id="..."` names or keep an explicit `toMsg`. `auto` is meant for the straightforward constructor-routing cases, not for every possible GTK event workflow.

## When to reach for lower-level primitives

`gtkApp` is the normal choice, but AIVI still exposes lower-level GTK tools when you need custom hosting:

| Need | Preferred tool |
| --- | --- |
| Standard single-window app | `gtkApp` |
| Pure derived UI data | plain helper, `signal`, or `computed` |
| Form state and validation | `aivi.ui.forms` |
| One-shot delayed or effectful follow-up work | commands such as `commandAfter` or `commandPerform` |
| Repeating or long-lived event source | subscriptions such as `subscriptionEvery` or `subscriptionSource` |
| Manual/custom event loop, experiments, or multi-window hosting | `signalStream`, `buildFromNode`, `reconcileNode` |
| Tests or one-off queue inspection | `signalPoll` |

For most app code, you can think in terms of: **model, messages, `gtkApp`, and a GTK/libadwaita view tree**.

## Where to go next

- [`aivi.ui.gtk4`](./gtk4.md) — runtime API reference, GTK sigil details, typed signal events, and low-level primitives
- [GTK App Architecture](./app_architecture.md) — deeper detail on `gtkApp`, commands, and subscriptions
- [Reactive Dataflow](./reactive_dataflow.md) — `signal`, `computed`, invalidation, and memoization
- [`aivi.ui.forms`](./forms.md) — field state, validation helpers, and form-focused examples
