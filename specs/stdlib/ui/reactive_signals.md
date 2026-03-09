# Signals

<!-- quick-info: {"kind":"topic","name":"signals"} -->
AIVI signals are first-class reactive values. Create writable source cells with `signal`, derive more signals with `map` and `combine2`, mutate them with `set` or `update`, and observe them with `watch` or `on`.
<!-- /quick-info -->

If you want the big-picture app guide, start with [`aivi.ui.gtk4`](./gtk4.md). If you want the runtime semantics, read [Reactive Dataflow](./reactive_dataflow.md). This page focuses on the day-to-day API shape.

## What a signal is

Treat `Signal a` like a first-class reactive container.

In practice that means:

- it holds a current value,
- other signals can be derived from it,
- widgets can bind to it directly,
- callbacks and event handles can update it,
- imported signals stay shared across modules because the runtime cell itself is the value.

Derived UI state is no longer a separate architecture. If a value should stay reactive, make it a `Signal`.

## Core surface

| API | Meaning |
| --- | --- |
| `signal initial` | Create a writable source signal. |
| `get s` | Read the current value. Best for callbacks, events, and low-level code. |
| `set s value` | Replace the current value. |
| `update s fn` | Transform the current value. May also accept patch-style record updates. |
| `map s fn` | Derive a new signal from one source signal. |
| `combine2` | Derive one signal from two source signals. |
| `watch s fn` / `on s fn` | Observe changes and run a callback or effect. Returns a disposable. |
| `batch fn` | Group several writes into one propagation batch. |
| `peek s` | Read without recording a dependency. |

The common style is: use signals and combinators in normal UI code, then reach for `get` or `peek` in callbacks and lower-level runtime integrations.

## Start simple

```aivi
count = signal 0
title = map count (value => "Count {value}")

increment = _ => update count (_ + 1)
reset = _ => set count 0
```

`title` is already the “computed” form. There is no need to switch into a separate derived-value API just because the data is read-only.

## Multi-signal composition

AIVI needs multi-signal combinators because real UI state rarely depends on just one source.

```aivi
firstName = signal "Ada"
lastName = signal "Lovelace"
saveBusy = saveProfile.running

fullName = combine2 firstName lastName (first => last => "{first} {last}")
canSaveBase = combine2 firstName lastName (first => last =>
  first != "" and last != ""
)
canSave = combine2 canSaveBase saveBusy (ready => running =>
  ready and not running
)
```

Use `combine2` when one derived value depends on two live sources. If the runtime grows higher-arity combinators later, they should stay ergonomic; until then, compose them explicitly.

## Record-valued signals and patch updates

Signals that hold records should support ergonomic patch-style updates:

```aivi
profile = signal {
  name: ""
  subscribed: False
  saveCount: 0
}

update profile <| { name: "AIVI" }
update profile <| { subscribed: not _ }
update profile <| { saveCount: _ + 1 }
```

When that is not expressive enough, fall back to a normal function:

```aivi
update profile (state =>
  state <| {
    name: normalize state.name
    saveCount: state.saveCount + 1
  }
)
```

## Watching and side effects

`watch` and `on` are for code that should react when a signal changes:

```aivi
dispose <- on query (text => logDebug "search query: {text}")
```

Useful rules:

- mounted UI bindings are just host-managed watchers,
- ordinary application code should keep watchers small and lifecycle-bound,
- use `peek` when a watcher needs a value without subscribing to it.

## Event handles expose signals too

`Event` handles fit the same model. They are effectful runtime values with reactive lifecycle fields:

```aivi
saveDraft : Event GtkError Text
saveDraft = do Event {
  run: do Effect {
    persistDraft (get draft)
    pure "Saved"
  }
}

saveMessage = map saveDraft.result (maybeResult =>
  maybeResult match
    | Some text => text
    | None      => ""
)
```

Important fields:

- `saveDraft.result`
- `saveDraft.error`
- `saveDraft.done`
- `saveDraft.running`

Because those fields are signals, you can combine them with other state through the same `map` and `combine2` APIs.

## When not to introduce a signal

Use an ordinary helper function when all of these are true:

- the value is local,
- it is computed once in one place,
- it does not need reactive lifetime or sharing.

If the value needs to stay live across widget bindings, watchers, or event-handle state, make it a `Signal`.

## Where to go next

- [Reactive Dataflow](./reactive_dataflow.md) — invalidation, batching, dependency tracking, and lifecycle cleanup
- [`aivi.ui.gtk4`](./gtk4.md) — full-app guidance, GTK widget binding rules, and low-level event helpers
