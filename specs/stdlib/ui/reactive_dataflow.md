# Derived Dataflow

<!-- quick-info: {"kind":"topic","name":"derived dataflow"} -->
AIVI derived dataflow lets you name pure calculations over the current committed `Model` and, when needed, cache them. A committed model snapshot is the full `Model` after one successful app turn. Use `derive` for a named pure reader and `memo` when that same reader needs memoization; outside change still enters only through `Msg`, commands, and subscriptions.
<!-- /quick-info -->

If you want the gentler introduction first, read [Derived Values](./reactive_signals.md) and [Native GTK & libadwaita Apps](./native_gtk_apps.md). This page explains the underlying rules.

> **Terminology note:** On this page, `derive` means a named pure reader and `memo` means the memoized form of that same idea. GTK signal **events** such as clicks and text changes remain a separate concept.

## Why this exists

Many UI values are derived from other state:

- filtered rows,
- grouped sections,
- labels such as “Showing 24 results”,
- visibility flags,
- expensive view-only projections.

You can always compute those values inline, and often that is the best choice. Derived dataflow becomes useful when a derived value deserves a name, is reused in more than one place, or is expensive enough that memoization matters.

The important boundary is this: derived dataflow stays on the **pure** side of the app. It does not fetch data, spawn work, or mutate state.

## Quick mental model

If the app architecture is a message loop, derived dataflow is the spreadsheet layer inside that loop:

- model fields are the input cells,
- `derive` values are reusable named formulas without caching,
- `memo` values are reusable named formulas with a stable key and caching,
- commands and subscriptions are everything that touches the outside world.

Use derived dataflow when:

- the app already has the right source data in its `Model`,
- a derived value is reused, expensive, or easier to understand when named,
- you want caching for pure calculations, not a second effect system.

If all you need is one short expression inside `view`, an ordinary helper is still the simplest choice.

## Choose between a helper, `derive`, and `memo`

| If the value is... | Reach for... | Why |
| --- | --- | --- |
| short, local, and read once | a plain helper or inline expression | the simplest thing stays the clearest |
| reused in a few places or worth naming | `derive` | gives the derivation a readable name |
| reused and expensive enough to cache | `memo` | keeps the value pure while avoiding repeated work |

## Core vocabulary

| Term | Meaning | Example |
| --- | --- | --- |
| **source value** | An authoritative snapshot that may change between app turns. In a standard GTK app, source values are ordinary fields inside the committed `Model`. | `model.projects`, `model.query`, `model.loading` |
| **derived value** | Any pure projection over source values or other derived values. | `length model.projects`, `filter isVisible model.projects` |
| **derive** | A named read-only derived value that can be reused by other definitions or by the host. It has no side effects. | `headerText = derive (model => ...)` |
| **memo value** | A derived reader with a stable key and memoization. The host tracks what it read last time and reuses the cached result until one of those dependencies changes. | `visibleProjects = memo "projects.visible" (model => ...)` |
| **dirty** | Marked for recomputation because one of the inputs changed. Dirty values recompute only when read. | `visibleProjects` after `model.query` changes |

A plain helper is correct by default. Promote it to `derive` or `memo` only when the extra structure helps.

## Purity and turn boundaries

Derived values are evaluated **inside** an app turn, never in the background.

The practical reason for these rules is predictability: derived values should be as easy to test and reason about as ordinary pure helper functions.

- They may read only committed source snapshots and other derived values.
- They may not perform `Effect`, acquire `Resource`, spawn tasks, sleep, or emit `Msg`.
- They may not mutate GTK widgets or the model directly.
- They are synchronous.

If a value might fail or wait for IO, model that uncertainty explicitly in your source snapshot with `Option`, `Result`, `LoadState`, or a similar type.

## Inline values, `derive`, and `memo`

There are three common levels of reuse:

1. **inline derived value** — a local pure expression inside `view` or `subscriptions`,
2. **`derive`** — an extracted pure reader that improves reuse or readability,
3. **`memo`** — an extracted pure reader whose result is memoized.

Conceptual examples:

<<< ../../snippets/from_md/stdlib/ui/reactive_dataflow/block_01.aivi{aivi}


Both helpers above are already derived values. They stay pure and may be recomputed whenever read.

When the same derivation is expensive or widely reused, promote it:

<<< ../../snippets/from_md/stdlib/ui/reactive_dataflow/block_02.aivi{aivi}


The snippets above show the two endpoints: plain helpers first, then the memoized `memo` form. `derive` sits in the middle and has the same reader shape as a helper, just with a clearer derived-state name:

<<< ../../snippets/from_md/stdlib/ui/reactive_dataflow/block_01.aivi{aivi}


For `memo`, the first argument is a stable descriptive key that identifies the cached derivation across app turns.

The current public surface is:

<<< ../../snippets/from_md/stdlib/ui/reactive_dataflow/block_02.aivi{aivi}


- `derive` marks a plain derived reader intended for reuse,
- `memo` marks a memoized reader with a stable key,
- `readDerived` is the explicit way to evaluate one of these readers outside GTK sigils; `readDerived title model` and `title model` are equivalent.

These helpers are still ordinary values of shape `model -> a`. Inside GTK sigils hosted by `gtkApp`, common binding positions such as attribute splices and `<each items={...}>` auto-read them against the current committed model. Outside the sigil, they remain explicit function values.

## How derived dataflow fits the normal app loop

Derived dataflow does not bypass `Msg` or `update`. It sits between the committed model and the next render:

1. a GTK signal event, timer, command result, or subscription event produces a `Msg`
2. `update` commits the next authoritative model
3. `view` reads any `derive` or `memo` helpers it needs
4. dirty memo values recalculate lazily
5. `reconcileNode` patches the live widget tree

That means data from a watcher, search task, or network stream becomes source data for derived values **only after `update` commits it to the model**. For the full event-loop picture, see [GTK App Architecture](./app_architecture.md#how-one-app-turn-works).

## Memoization and invalidation

Memoization just means caching the result of a pure calculation so repeated reads do not redo the same work unnecessarily.

### What `memo` remembers

Each committed source snapshot has a logical revision. A `memo` cache entry stores:

- its stable key,
- the source and derived-value dependencies it read during the last successful evaluation,
- the dependency revisions seen during that evaluation,
- the cached result.

In practice, the host remembers “which inputs did I read last time?” and “what result did I get?” If those inputs still match, the cached result can be reused safely.

### Invalidation rules in practice

Invalidation follows this checklist:

- when `update` commits a new model, every changed source snapshot gets a new revision,
- every memoized derived value that depended on one of those changed revisions becomes **dirty**,
- dirtiness propagates through dependent memoized values,
- dirty memo values do **not** rerun immediately; they recompute on the next synchronous read,
- the first read of a dirty memoized value records a fresh dependency set and caches the new result,
- later reads in the same turn reuse that cache.

Example: if `visibleProjects = memo "projects.visible" (...)` read both `model.projects` and `model.query` last turn, then a new `model.query` value marks it dirty. Nothing reruns yet. The next time `view` or `subscriptions` reads `visibleProjects`, it recomputes once from the new model snapshot and caches that fresh result for later reads in the same turn.

Consequences:

- repeated reads of the same memoized value within one render run the underlying pure computation at most once,
- data-dependent dependency sets are recalculated from the latest successful evaluation,
- correctness comes from dependency tracking first; memoization shortcuts must not change behavior.

Derived dependency cycles are invalid. Obvious self-recursion should be rejected statically, and dynamic cycles must surface as a host error instead of looping forever.

## Boundaries: derived values vs effects vs subscriptions

| Concern | Derived values | Effects / commands | Subscriptions |
| --- | --- | --- | --- |
| Owns authoritative state? | No. Reads committed source snapshots. | No. Reads captured values and produces later `Msg`. | No. Produces later `Msg` from long-lived resources. |
| Can perform IO? | No. | Yes. | Yes. |
| Runs when? | Synchronously when ordinary app code reads it, most often from `view` or `subscriptions`. | After `update` commits the returned model. | While installed by `gtkApp`; diffed and cancelled by key. |
| May mutate model or widgets directly? | No. | No; they must emit `Msg` and let `update` commit the next model. | No; they must emit `Msg` and let `update` commit the next model. |
| Lifetime | Current turn plus memo cache for memo nodes. | One-shot or keyed background task lifetime. | Long-lived until removed, replaced, or host shutdown. |

Two rules are especially useful in practice:

- **effects capture snapshots, not live derived readers** — if a command needs derived data, evaluate the derived value first and pass the plain result into the command,
- **subscriptions describe producers, not observers** — a subscription may decide whether it should exist from the current model, but once running it can only influence the app by emitting `Msg`.

In other words, capture the plain value, not the reader:

<<< ../../snippets/from_md/stdlib/ui/reactive_dataflow/block_03.aivi{aivi}


Avoid capturing `resultsSummary` itself inside a command or subscription closure when what you really need is the current `Text`.

## Interaction with subscriptions

`subscriptions : Model -> List (Subscription Msg)` may use derived or memoized values to decide:

- which subscriptions should be active,
- which keys or configuration they should use,
- which plain snapshot values should be captured when opening them.

Those decisions are still recomputed only at commit boundaries. Reading a derived value does not keep a subscription alive by itself and does not open resources.

## Interaction with forms and local view state

Form helpers from [`aivi.ui.forms`](./forms.md) remain ordinary source values inside the model:

- `Field.value`, `Field.touched`, and `Field.dirty` are source snapshots,
- inline error lists and submit enablement are ideal derived or memoized values,
- asynchronous validation, debounced search, or remote suggestions still belong to commands or subscriptions.

So the derived layer improves reuse of pure UI logic without creating hidden mutable form state.

## Design constraints

The derived model deliberately keeps these guardrails:

- no ambient observer graph outside `gtkApp`,
- no hidden mutation from subscriptions or effects into memo caches,
- no second scheduler beyond the existing command/subscription host,
- no separate effect model for derived code,
- no replacement for `Model -> View -> Msg -> Update`.

In short: derived dataflow is a tool for expressing and reusing pure derived state, not a second application architecture.
