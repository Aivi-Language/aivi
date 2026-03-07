# Reactive Dataflow

<!-- quick-info: {"kind":"topic","name":"reactive dataflow"} -->
AIVI reactive dataflow is a way to name and cache pure calculations over the current committed `Model`. A committed model snapshot is simply the full `Model` after one successful app turn. `computed` values memoize—that is, cache—their last pure result until one of their inputs changes—while external change still enters through `Msg` via commands and subscriptions.
<!-- /quick-info -->

If you want the gentler introduction first, read [Reactive Signals](./reactive_signals.md) and [Native GTK & libadwaita Apps](./native_gtk_apps.md). This page explains the underlying rules.

## Why this exists

Many UI values are derived from other state:

- filtered rows,
- grouped sections,
- labels such as “Showing 24 results”,
- visibility flags,
- expensive view-only projections.

You can always compute those values inline, and often that is the best choice. Reactive dataflow becomes useful when a derived value deserves a name, is reused in more than one place, or is expensive enough that memoization matters.

The important boundary is this: reactive dataflow stays on the **pure** side of the app. It does not fetch data, spawn work, or mutate state.

## Quick mental model

If the app architecture is a message loop, reactive dataflow is the spreadsheet layer inside that loop:

- model fields are the input cells,
- `signal` values are named formulas,
- `computed` values are named formulas with caching,
- commands and subscriptions are everything that touches the outside world.

Use reactive dataflow when:

- the app already has the right source data in its `Model`,
- a derived value is reused, expensive, or easier to understand when named,
- you want caching for pure calculations, not a second effect system.

If all you need is one short expression inside `view`, an ordinary helper is still the simplest choice.

## Choose between a helper, `signal`, and `computed`

| If the value is... | Reach for... | Why |
| --- | --- | --- |
| short, local, and read once | a plain helper or inline expression | the simplest thing stays the clearest |
| reused in a few places or worth naming | `signal` | gives the derivation a readable name |
| reused and expensive enough to cache | `computed` | keeps the value pure while avoiding repeated work |

## Core vocabulary

| Term | Meaning | Example |
| --- | --- | --- |
| **source value** | An authoritative snapshot that may change between app turns. In a standard GTK app, source values are ordinary fields inside the committed `Model`. | `model.projects`, `model.query`, `model.loading` |
| **derived value** | Any pure projection over source values or other derived values. | `length model.projects`, `filter isVisible model.projects` |
| **signal** | A named read-only derived value that can be reused by other definitions or by the host. It has no side effects and no capability clauses. | `headerText = signal (model => ...)` |
| **computed value** | A signal with stable identity and memoization. The host tracks what it read last time and reuses the cached result until one of those dependencies changes. | `visibleProjects = computed "projects.visible" (model => ...)` |
| **dirty** | Marked for recomputation because one of the inputs changed. Dirty values recompute only when read. | `visibleProjects` after `model.query` changes |

A plain helper is correct by default. Promote it to `signal` or `computed` only when the extra structure helps.

## Purity and turn boundaries

Reactive values are evaluated **inside** an app turn, never in the background.

The practical reason for these rules is predictability: reactive values should be as easy to test and reason about as ordinary pure helper functions.

- They may read only committed source snapshots and other signals.
- They may not perform `Effect`, acquire `Resource`, spawn tasks, sleep, or emit `Msg`.
- They may not mutate GTK widgets or the model directly.
- They are synchronous.

If a value might fail or wait for IO, model that uncertainty explicitly in your source snapshot with `Option`, `Result`, `LoadState`, or a similar type.

## Derived values, signals, and computed values

There are three common levels of reuse:

1. **inline derived value** — a local pure expression inside `view` or `subscriptions`,
2. **named signal** — an extracted pure reader that improves reuse or readability,
3. **computed signal** — a named reader whose result is memoized.

Conceptual examples:

```aivi
visibleProjects = model =>
  model.projects
    |> filter (matchesQuery model.searchQuery)
    |> filter (matchesTags model.selectedTags)

resultsSummary = model =>
  if model.loading
    then "Searching..."
    else "Showing {length (visibleProjects model)} projects"
```

Both helpers above are already derived values. They stay pure and may be recomputed whenever read.

When the same derivation is expensive or widely reused, promote it:

```aivi
visibleProjects =
  computed "projects.visible" (model =>
    model.projects
      |> filter (matchesQuery model.searchQuery)
      |> filter (matchesTags model.selectedTags)
  )
```

The current public surface is:

```aivi
signal : (model -> a) -> model -> a
computed : Text -> (model -> a) -> model -> a
readSignal : (model -> a) -> model -> a
```

- `signal` marks a plain derived reader intended for reactive reuse,
- `computed` marks a memoized reader with a stable key,
- `readSignal` is the explicit non-GTK way to evaluate a signal.

Inside GTK sigils hosted by `gtkApp`, attribute splices and `<each items={...}>` auto-read `signal` and `computed` helpers against the current committed model. Outside the sigil, signals remain explicit function values.

## How reactive dataflow fits the normal app loop

Reactive dataflow does not bypass `Msg` or `update`. It sits between the committed model and the next render:

1. a GTK signal, timer, command result, or subscription event produces a `Msg`
2. `update` commits the next authoritative model
3. `view` reads any `signal` or `computed` helpers it needs
4. dirty computed values recalculate lazily
5. `reconcileNode` patches the live widget tree

That means data from a watcher, search task, or network stream becomes reactive source data **only after `update` commits it to the model**. For the full event-loop picture, see [GTK App Architecture](./app_architecture.md#how-one-app-turn-works).

## Memoization and invalidation

Memoization just means caching the result of a pure calculation so repeated reads do not redo the same work unnecessarily.

### What `computed` remembers

Each committed source snapshot has a logical revision. A computed cache entry stores:

- its stable key,
- the source and signal dependencies it read during the last successful evaluation,
- the dependency revisions seen during that evaluation,
- the cached result.

In practice, the host remembers “which inputs did I read last time?” and “what result did I get?” If those inputs still match, the cached result can be reused safely.

### Advanced invalidation rules

Invalidation follows this checklist:

- when `update` commits a new model, every changed source snapshot gets a new revision,
- every computed signal that depended on one of those changed revisions becomes **dirty**,
- dirtiness propagates through dependent computed signals,
- dirty signals do **not** rerun immediately; they recompute on the next synchronous read,
- the first read of a dirty computed signal records a fresh dependency set and caches the new result,
- later reads in the same turn reuse that cache.

Consequences:

- repeated reads of the same computed signal within one render run the underlying pure computation at most once,
- data-dependent dependency sets are recalculated from the latest successful evaluation,
- correctness comes from dependency tracking first; memoization shortcuts must not change behavior.

Reactive dependency cycles are invalid. Obvious self-recursion should be rejected statically, and dynamic cycles must surface as a host error instead of looping forever.

## Boundaries: reactive values vs effects vs subscriptions

| Concern | Reactive values | Effects / commands | Subscriptions |
| --- | --- | --- | --- |
| Owns authoritative state? | No. Reads committed source snapshots. | No. Reads captured values and produces later `Msg`. | No. Produces later `Msg` from long-lived resources. |
| Can perform IO or use capabilities? | No. | Yes. | Yes. |
| Runs when? | Synchronously when `view`, `subscriptions`, or command construction reads it. | After `update` commits the returned model. | While installed by `gtkApp`; diffed and cancelled by key. |
| May mutate model or widgets directly? | No. | No; they must emit `Msg` and let `update` commit the next model. | No; they must emit `Msg` and let `update` commit the next model. |
| Lifetime | Current turn plus memo cache for computed nodes. | One-shot or keyed background task lifetime. | Long-lived until removed, replaced, or host shutdown. |

Two rules are especially useful in practice:

- **effects capture snapshots, not live signals** — if a command needs derived data, evaluate the reactive value first and pass the plain result into the command,
- **subscriptions describe producers, not observers** — a subscription may decide whether it should exist from the current model, but once running it can only influence the app by emitting `Msg`.

## Interaction with subscriptions

`subscriptions : Model -> List (Subscription Msg)` may use derived or computed values to decide:

- which subscriptions should be active,
- which keys or configuration they should use,
- which plain snapshot values should be captured when opening them.

Those decisions are still recomputed only at commit boundaries. Reading a signal does not keep a subscription alive by itself and does not open resources.

## Interaction with forms and local view state

Form helpers from [`aivi.ui.forms`](./forms.md) remain ordinary source values inside the model:

- `Field.value`, `Field.touched`, and `Field.dirty` are source snapshots,
- inline error lists and submit enablement are ideal derived or computed values,
- asynchronous validation, debounced search, or remote suggestions still belong to commands or subscriptions.

So the reactive layer improves reuse of pure UI logic without creating hidden mutable form state.

## Design constraints

The reactive model deliberately keeps these guardrails:

- no ambient observer graph outside `gtkApp`,
- no hidden mutation from subscriptions or effects into computed caches,
- no second scheduler beyond the existing command/subscription host,
- no separate capability model for reactive code,
- no replacement for `Model -> View -> Msg -> Update`.

In short: reactive dataflow is a tool for expressing and reusing pure derived state, not a second application architecture.
