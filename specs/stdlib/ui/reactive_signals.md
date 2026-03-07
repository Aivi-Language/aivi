# Reactive Signals

<!-- quick-info: {"kind":"topic","name":"reactive signals"} -->
Reactive signals are pure derived readers over committed model state. Use them to give reusable UI data a name, and promote them to `computed` when the same pure derivation should be memoized. They are not the same thing as GTK signal events.
<!-- /quick-info -->

If you want the big-picture app guide, start with [Native GTK & libadwaita Apps](./native_gtk_apps.md). If you want the detailed semantics, read [Reactive Dataflow](./reactive_dataflow.md). This page focuses on practical use.

## Two different meanings of “signal”

AIVI GTK docs use the word **signal** in two places:

| Term | Meaning |
| --- | --- |
| **GTK signal event** | Input coming from widgets, represented as `GtkSignalEvent` values such as `GtkClicked` or `GtkInputChanged` |
| **reactive signal** | Pure derived data read from the committed model via `signal`, `computed`, and `readSignal` |

They do different jobs:

- GTK signal events tell the app that something happened,
- reactive signals help the app derive what to render from current state.

## What reactive signals are for

Use reactive signals when a GTK app has pure derived values that deserve a name:

- a title derived from the current model,
- a filtered or grouped view of rows,
- a status label,
- a timer interval derived from settings,
- a summary reused in several places.

Reactive signals do **not** perform IO, do **not** mutate state, and do **not** emit messages.

## Start simple: helper first, then `signal`, then `computed`

There are three common levels:

1. plain helper function,
2. `signal` for a named reactive reader,
3. `computed` for a named reader with memoization.

```aivi
headline = state =>
  if state.query == ""
    then "All Projects"
    else "Search · {state.query}"

headlineSignal =
  signal (state =>
    // Same logic, but named explicitly as a reactive reader.
    if state.query == ""
      then "All Projects"
      else "Search · {state.query}"
  )

rowNames =
  computed "projects.rowNames" (state =>
    // Memoize the mapped list when it is reused.
    map (row => row.name) state.rows
  )
```

Use a plain helper when the value is small and local. Use `signal` when a named reader makes the code easier to follow. Use `computed` when the same pure work is read repeatedly and should be cached until its dependencies change.

## How they fit into a GTK app

Reactive signals read from the app model and fit into the normal `gtkApp` loop:

```aivi
use aivi.ui.gtk4

Row = { name: Text }

Model = {
  query: Text
  rows: List Row
  fastMode: Bool
}

Msg = QueryChanged Text | Tick

headline : Model -> Text
headline =
  signal (state =>
    if state.query == ""
      then "All Projects"
      else "Search · {state.query}"
  )

rowNames : Model -> List Text
rowNames =
  computed "projects.rowNames" (state =>
    map (row => row.name) state.rows
  )

tickMillis : Model -> Int
tickMillis =
  signal (state =>
    // Let the timer speed depend on the current model.
    if state.fastMode then 250 else 1000
  )

view : Model -> GtkNode
view = _ =>
  ~<gtk>
    <GtkBox orientation="vertical" spacing="8">
      <GtkLabel label={headline} />
      <each items={rowNames} as={name}>
        <GtkLabel label={name} />
      </each>
    </GtkBox>
  </gtk>

subscriptions : Model -> List (Subscription Msg)
subscriptions = state => [
  subscriptionEvery {
    key: "tick"
    millis: readSignal tickMillis state
    tag: Tick
  }
]
```

This shows the usual pattern:

- `view` reads `headline` and `rowNames`,
- `rowNames` is memoized because it may be reused,
- `subscriptions` uses `readSignal` explicitly outside the GTK sigil.

## GTK sigils auto-read signals

Inside `~<gtk>...</gtk>` hosted by `gtkApp`, signal values are read automatically in common binding positions such as:

- attribute splices like `label={headline}`,
- `<each items={rowNames} as={name}>`.

Outside the sigil, signals stay ordinary function values, so use `readSignal` or plain function application.

## When not to use reactive signals

Reactive signals are the wrong tool when the work is effectful or long-lived. Use:

- `GtkSignalEvent` + `toMsg` for user input,
- commands for post-update effects,
- subscriptions for timers and external feeds,
- model fields for authoritative state.

Signals should stay pure and synchronous.

## Where to go next

- [Reactive Dataflow](./reactive_dataflow.md) — invalidation, memoization, dependency tracking, and semantics
- [Native GTK & libadwaita Apps](./native_gtk_apps.md) — how reactive signals fit into the full app loop
- [`aivi.ui.gtk4`](./gtk4.md) — GTK signal events and widget-side signal bindings
