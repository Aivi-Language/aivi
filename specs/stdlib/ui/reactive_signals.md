# Derived Values

<!-- quick-info: {"kind":"topic","name":"derived values"} -->
Derived values are pure readers over committed model state. Use `derive` to give reusable UI data a name, and promote it to `memo` when the same pure derivation should be cached. They are not the same thing as GTK signal events.
<!-- /quick-info -->

If you want the big-picture app guide, start with [Native GTK & libadwaita Apps](./native_gtk_apps.md). If you want the turn-by-turn host flow around these helpers, read [GTK App Architecture](./app_architecture.md). If you want the detailed semantics, read [Derived Dataflow](./reactive_dataflow.md). This page focuses on practical use.

## GTK signals vs derived values

AIVI now reserves the word **signal** for GTK widget events. Pure model-derived UI helpers use different names:

| Term | Meaning |
| --- | --- |
| **GTK signal event** | Input coming from widgets, represented as `GtkSignalEvent` values such as `GtkClicked` or `GtkInputChanged` |
| **derived value** | Pure derived data read from the committed model via `derive`, `memo`, and `readDerived` |

They do different jobs:

- GTK signal events tell the app that something happened,
- derived values help the app derive what to render from current state.

## What derived values are for

Use derived values when a GTK app has pure derived values that deserve a name:

- a heading derived from the current model,
- a filtered or grouped view of rows,
- a status label,
- a timer interval derived from settings,
- a summary reused in several places.

Derived values do **not** perform IO, do **not** mutate state, and do **not** emit messages.

A good mental model is: a derived value is a named read of the current model, not a background listener.

## Start simple: helper first, then `derive`, then `memo`

There are three common levels:

1. plain helper function,
2. `derive` for a named derived reader,
3. `memo` for a named reader with memoization.

For `memo`, the first argument is a stable descriptive key that identifies the cached derivation across app turns.

<<< ../../snippets/from_md/stdlib/ui/reactive_signals/block_01.aivi{aivi}


Use a plain helper when the value is small and local. Use `derive` when a named reader makes the code easier to follow. Use `memo` when the same pure work is read repeatedly and should be cached until its dependencies change.

### Decision guide

| Reach for... | When it is usually the best choice |
| --- | --- |
| plain helper | the derivation is local, short, and only read in one place |
| `derive` | the same derived value deserves a name or is reused in a couple of places |
| `memo` | the derivation is reused heavily or expensive enough that caching clearly helps |

## How they fit into a GTK app

Derived values read from the app model and fit into the normal `gtkApp` loop:

The snippet below shows only the pure derived-data slice. The surrounding `toMsg`, `update`, and `gtkApp` wiring stays the same as in the normal app architecture.

<<< ../../snippets/from_md/stdlib/ui/reactive_signals/block_02.aivi{aivi}


This shows the usual pattern:

- `view` reads `heading` and `projectNames`,
- `projectNames` is memoized because it may be reused and carries a stable cache key (`"projects.names"`),
- `subscriptions` uses `readDerived` explicitly outside the GTK sigil.

## GTK sigils auto-read derived values

Inside `~<gtk>...</gtk>` hosted by `gtkApp`, derived values are read automatically in common binding positions such as:

- attribute splices like `label={heading}`,
- `<each items={projectNames} as={projectName}>`.

Outside the sigil, derived values stay ordinary function values of shape `Model -> A`, so use `readDerived` or plain function application.

For example, `readDerived refreshMillis model` and `refreshMillis model` evaluate the same pure reader; `readDerived` simply makes that boundary explicit in examples and app code.

## When not to use derived values

Derived values are the wrong tool when the work is effectful or long-lived. Use:

- `GtkSignalEvent` + `toMsg` for user input,
- commands for post-update effects,
- subscriptions for timers and external feeds,
- model fields for authoritative state.

Derived values should stay pure and synchronous.

## Where to go next

- [Derived Dataflow](./reactive_dataflow.md) — invalidation, memoization, dependency tracking, and semantics
- [GTK App Architecture](./app_architecture.md) — where `view`, `subscriptions`, commands, and turn boundaries fit together
- [Native GTK & libadwaita Apps](./native_gtk_apps.md) — how derived values fit into the full app loop
- [`aivi.ui.gtk4`](./gtk4.md) — GTK signal events and widget-side signal bindings
