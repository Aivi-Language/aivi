# Reactive Signals

<!-- quick-info: {"kind":"topic","name":"reactive signals"} -->
Reactive signals are pure derived readers over committed model state. Use them to give reusable UI data a name, and promote them to `computed` when the same pure derivation should be memoized. They are not the same thing as GTK signal events.
<!-- /quick-info -->

If you want the big-picture app guide, start with [Native GTK & libadwaita Apps](./native_gtk_apps.md). If you want the turn-by-turn host flow around these helpers, read [GTK App Architecture](./app_architecture.md). If you want the detailed semantics, read [Reactive Dataflow](./reactive_dataflow.md). This page focuses on practical use.

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

- a heading derived from the current model,
- a filtered or grouped view of rows,
- a status label,
- a timer interval derived from settings,
- a summary reused in several places.

Reactive signals do **not** perform IO, do **not** mutate state, and do **not** emit messages.

A good mental model is: a reactive signal is a named read of the current model, not a background listener.

## Start simple: helper first, then `signal`, then `computed`

There are three common levels:

1. plain helper function,
2. `signal` for a named reactive reader,
3. `computed` for a named reader with memoization.

For `computed`, the first argument is a stable descriptive key that identifies the cached derivation across app turns.

<<< ../../snippets/from_md/stdlib/ui/reactive_signals/block_01.aivi{aivi}


Use a plain helper when the value is small and local. Use `signal` when a named reader makes the code easier to follow. Use `computed` when the same pure work is read repeatedly and should be cached until its dependencies change.

### Decision guide

| Reach for... | When it is usually the best choice |
| --- | --- |
| plain helper | the derivation is local, short, and only read in one place |
| `signal` | the same derived value deserves a name or is reused in a couple of places |
| `computed` | the derivation is reused heavily or expensive enough that caching clearly helps |

## How they fit into a GTK app

Reactive signals read from the app model and fit into the normal `gtkApp` loop:

The snippet below shows only the pure derived-data slice. The surrounding `toMsg`, `update`, and `gtkApp` wiring stays the same as in the normal app architecture.

<<< ../../snippets/from_md/stdlib/ui/reactive_signals/block_02.aivi{aivi}


This shows the usual pattern:

- `view` reads `heading` and `projectNames`,
- `projectNames` is memoized because it may be reused and carries a stable cache key (`"projects.names"`),
- `subscriptions` uses `readSignal` explicitly outside the GTK sigil.

## GTK sigils auto-read signals

Inside `~<gtk>...</gtk>` hosted by `gtkApp`, signal values are read automatically in common binding positions such as:

- attribute splices like `label={heading}`,
- `<each items={projectNames} as={projectName}>`.

Outside the sigil, signals stay ordinary function values of shape `Model -> A`, so use `readSignal` or plain function application.

For example, `readSignal refreshMillis model` and `refreshMillis model` evaluate the same pure reader; `readSignal` simply makes that boundary explicit in examples and app code.

## When not to use reactive signals

Reactive signals are the wrong tool when the work is effectful or long-lived. Use:

- `GtkSignalEvent` + `toMsg` for user input,
- commands for post-update effects,
- subscriptions for timers and external feeds,
- model fields for authoritative state.

Signals should stay pure and synchronous.

## Where to go next

- [Reactive Dataflow](./reactive_dataflow.md) — invalidation, memoization, dependency tracking, and semantics
- [GTK App Architecture](./app_architecture.md) — where `view`, `subscriptions`, commands, and turn boundaries fit together
- [Native GTK & libadwaita Apps](./native_gtk_apps.md) — how reactive signals fit into the full app loop
- [`aivi.ui.gtk4`](./gtk4.md) — GTK signal events and widget-side signal bindings
