# Reactive Dataflow

<!-- quick-info: {"kind":"topic","name":"reactive dataflow"} -->
AIVI reactive dataflow is a runtime-owned graph of source signals, derived signals, event-handle state channels, watchers, and mounted widget bindings. Signal writes propagate through that graph in batches and update only the exact bound props, text nodes, and structural scopes that depend on them.
<!-- /quick-info -->

If you want the gentler introduction first, read [Signals](./reactive_signals.md) and [`aivi.ui.gtk4`](./gtk4.md). This page explains the semantic contract behind the signal-first model.

## Why this exists

GTK apps need more than “store a value and redraw everything.” They need:

- reusable derived values,
- dependency-aware updates,
- efficient direct widget mutation,
- structural child bindings,
- cleanup when mounted scopes disappear,
- async workflows whose status can be observed reactively.

AIVI's answer is a signal graph, not a reducer loop over committed model snapshots.

## Core runtime vocabulary

| Term | Meaning |
| --- | --- |
| **source signal** | Writable cell created with `signal`, `set`, and `update`. |
| **derived signal** | Read-only signal created with `derive` or `combineAll`. |
| **watcher** | Callback/effect subscribed to a signal. Mounted widget bindings are watchers owned by the host. |
| **batch** | One propagation pass that coalesces several writes before notifying observers. |
| **mounted scope** | Lifetime bucket for widget bindings, structural binders, GTK handlers, and cleanup callbacks. |
| **event handle** | Effectful runtime value whose fields (`result`, `error`, `done`, `running`) are themselves signals. |

## What happens when a signal changes

The runtime contract is:

1. a callback, watcher, or event body writes to one or more source signals,
2. those source signals are marked changed inside the current batch,
3. dependent derived signals become dirty,
4. the runtime recomputes dirty derived signals before notifying consumers,
5. each derived signal recomputes at most once per batch,
6. mounted widget bindings and other watchers receive the settled value,
7. structural bindings patch only the scopes whose guard/items signals changed.

The important goal is precision: changing one signal should only touch the exact GTK state that depends on it.

## Derived signals are the only computed-state model

AIVI does not split “computed state” into a separate subsystem. If a value is reactive and read-only, it is still a `Signal`.

That means:

- `derive` is the normal one-input computed form,
- `combineAll` is the record-based multi-input computed form,
- if a convenience `computed` helper exists, it is only sugar for producing another `Signal`.

Example:

```aivi
visibleCount = derive projects length
saveEnabled = combineAll { dirty: dirty, running: saveEvent.running } (vals => vals.dirty and not vals.running)
```

## Batching

`batch` groups several writes into one propagation cycle:

```aivi
batch (_ =>
  do Effect {
    update state (patch { saving: True })
    set lastError None
    pure Unit
  }
)
```

Within one batch:

- downstream derived signals should settle from the final written source values,
- watchers should not observe intermediate states unless they explicitly ask for them,
- mounted GTK bindings should update after the batch has a coherent result.

## Tracking and untracking

Most dependencies are explicit because `derive` and `combineAll` name their source signals directly. For watcher code and lower-level helpers, the runtime still needs tracking rules:

- reading with `get` inside a watcher or derived callback records a dependency,
- `peek` reads without subscribing.

This matters when a watcher wants to read auxiliary state without turning that extra value into a trigger.

## Structural bindings are scoped reactive nodes

Structural bindings are not “rerender the whole tree.” They are dedicated reactive nodes with their own scope semantics.

### `<show>`

- subscribes to one boolean signal,
- mounts the child scope when `True`,
- disposes that scope when `False`.

### `<each>`

- subscribes to a list signal,
- uses stable keys to preserve child scopes,
- moves existing mounted children when keys reorder,
- creates new scopes for new keys,
- disposes removed keys deterministically.

The GTK host owns the actual insert/move/remove logic because container semantics differ by widget.

## Mounted scope cleanup

Every mounted scope owns:

- signal watchers,
- GTK callback connection ids,
- child scopes,
- cleanup callbacks installed by the renderer or a library.

When a scope is disposed, the runtime must:

- remove watchers,
- disconnect GTK handlers,
- dispose child scopes,
- unparent or destroy widgets as required by the host widget.

This is the boundary that keeps long-lived reactive graphs from leaking after a subtree disappears.

## Event handles participate in the same graph

`Event` handles are not a separate async architecture. They are graph nodes with lifecycle signals.

```aivi
refreshData = do Event {
  fetchRows
}

rows = combineAll { cached: cachedRows, fresh: refreshData.result } (vals =>
  vals.fresh match
    | Some rows => rows
    | None      => vals.cached
)
```

The runtime should treat `refreshData.result`, `refreshData.error`, `refreshData.done`, and `refreshData.running` like any other signal for batching, invalidation, and watcher notification.

## Errors and cycles

Reactive dependency cycles are invalid.

The public contract is:

- obvious static self-cycles should be rejected early,
- dynamic cycles must surface as a runtime error instead of looping forever,
- event-handle failures update the event's `error` signal and clear `running`.

## Design constraints that follow from this model

The runtime should preserve these rules:

- signals are first-class runtime values, so imported signals remain shared,
- derived values are regular signals, not model-snapshot readers,
- widget updates are direct and precise rather than full-tree rerenders,
- structural bindings own the only structure-aware patching logic,
- event handles expose async lifecycle as signals,
- cleanup is scope-based and deterministic.

## Where to go next

- [Signals](./reactive_signals.md) — the day-to-day API surface for source and derived signals
- [`aivi.ui.gtk4`](./gtk4.md) — how the graph reaches GTK widgets, mounted bindings, and low-level runtime helpers
