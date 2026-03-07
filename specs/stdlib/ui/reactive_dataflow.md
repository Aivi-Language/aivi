# Reactive Dataflow

> **Status: Phase 4 semantic model specified, `computed` runtime slice landed**  
> This page defines the reactive semantics that sit on top of `gtkApp`: source snapshots, pure derived values, memoized computed signals, and invalidation rules. It does **not** create a second effect system or a hidden observer runtime.

<!-- quick-info: {"kind":"topic","name":"reactive dataflow"} -->
AIVI reactive dataflow is a pure memoized derivation graph over committed model snapshots. External change still enters through `Msg` via commands and subscriptions; reactive values only decide how current state is derived and reused.
<!-- /quick-info -->

Reactive dataflow is intentionally subordinate to the blessed GTK architecture:

1. authoritative state still lives in `Model`,
2. commands and subscriptions still own IO, timers, and background work,
3. reactive values only derive reusable pure snapshots from that committed state,
4. `view` and `subscriptions` read those snapshots synchronously during a turn.

## Core vocabulary

| Term | Meaning |
| --- | --- |
| **source value** | An authoritative snapshot that may change between app turns. In standard GTK apps, source values are ordinary fields inside the committed `Model` (for example query text, current rows, connection status, or the latest payload received from a subscription). |
| **derived value** | Any pure projection over source values or other derived values. A plain helper function such as `filteredRows = state => ...` is already a derived value. |
| **signal** | A named read-only derived value that can be reused by other reactive definitions or by the host. A signal has no side effects and no capability clauses. |
| **computed value** | A signal with stable identity and memoization. The host tracks which source/signal revisions it read last time and reuses the cached result until one of those dependencies changes. |

A plain derived helper is correct by default; use a computed signal only when the same pure work should be shared across reads or across app turns.

## Purity and turn boundaries

Reactive values are evaluated **inside** an app turn, never in the background.

- They may read only committed source snapshots and other signals.
- They may not perform `Effect`, acquire `Resource`, spawn tasks, sleep, or emit `Msg`.
- They may not mutate GTK widgets or the model directly.
- They are synchronous: if a value might fail or wait for IO, model that uncertainty explicitly in the source snapshot with `Option`, `Result`, `LoadState`, or a similar domain type.

This preserves AIVI's explicit-effects rule: change from the outside world still enters only through messages, commands, and subscriptions.

## Derived values, signals, and computed values

The spec distinguishes three levels of reuse:

1. **inline derived value** — local pure expression inside `view` or `subscriptions`;
2. **named signal** — extracted pure helper shared by multiple readers;
3. **computed signal** — named signal that the host memoizes.

Conceptual examples:

```aivi
visibleRows = state =>
  state.rows
    |> filter (matchesQuery state.query)
    |> filter (matchesTags state.selectedTags)

searchSummary = state =>
  if state.loading
    then "Searching..."
    else "Showing {length (visibleRows state)} rows"
```

Both helpers above are derived values. They stay pure and may be recomputed whenever read.

When the same derivation is expensive or fan-out-heavy, promote it to a computed signal with stable identity:

```aivi
visibleRows =
  computed "visibleRows" (state =>
    state.rows
      |> filter (matchesQuery state.query)
      |> filter (matchesTags state.selectedTags)
  )
```

`computed "visibleRows" ...` is the shipped Phase 4 helper: the stable key names the memoized node in the reactive graph.

The current public surface is:

```aivi
signal : (model -> a) -> model -> a
computed : Text -> (model -> a) -> model -> a
readSignal : (model -> a) -> model -> a
```

- `signal` marks a plain derived reader intended for reactive reuse,
- `computed` marks a memoized reader with a stable key,
- `readSignal` is the explicit non-GTK way to evaluate a signal value.

Inside GTK sigils hosted by `gtkApp`, attribute splices and `<each items={...}>` auto-read `signal`/`computed` helpers against the current committed model. Outside the sigil, signals remain explicit function values.

## Memoization and invalidation

Each committed source snapshot has a logical revision. A computed signal cache entry stores:

- its stable key,
- the set of source/signal dependencies it read during the last successful evaluation,
- the dependency revisions seen during that evaluation,
- the cached result.

Invalidation follows these rules:

1. when `update` commits a new model, every changed source snapshot gets a new revision,
2. every computed signal that depended on one of those changed revisions becomes **dirty**,
3. dirtiness propagates transitively through dependent computed signals,
4. dirty signals do **not** run immediately; they recompute lazily on the next synchronous read,
5. the first read of a dirty computed signal in a turn reevaluates it against the current committed sources, records a fresh dependency set, and caches the new result,
6. later reads of that computed signal in the same turn reuse that cache.

Consequences:

- repeated reads of the same computed signal within one `view` pass run the underlying pure computation at most once,
- if a computed signal's dependency set is data-dependent, the host replaces the old dependency edges with the ones observed during the latest successful evaluation,
- invalidation is about dependency correctness first; implementations may add equality-based short-circuiting later, but correctness must not rely on it.

Reactive dependency cycles are invalid. Obvious self-recursion should be rejected statically; dynamic cycles must surface as a host error rather than loop forever silently.

## Source-driven UI updates

Reactive dataflow does not bypass `Msg` or `update`. External sources still update the UI through the blessed event loop:

1. a GTK signal, timer, command result, or `Subscription.source` event produces a `Msg`,
2. `update` stores the new authoritative snapshot in the model,
3. `gtkApp` commits the new model and invalidates affected computed signals,
4. `view` reads any required signals against that committed model,
5. dirty computed signals recalculate lazily,
6. `reconcileNode` patches the live widget tree.

That means subscription-fed data such as search results, file-watch payloads, sensor readings, or database notifications become **source values only after `update` commits them**. No subscription or effect may write into a signal cache directly.

In the current milestone, the host still reevaluates the `view` function after each committed update; signal-aware GTK bindings make those reads ergonomic and allow computed memoization to remove duplicate pure work inside a turn. Finer-grained widget-slot invalidation remains a follow-up optimization.

## Boundaries: reactive values vs effects vs subscriptions

| Concern | Reactive values | Effects / commands | Subscriptions |
| --- | --- | --- | --- |
| Owns authoritative state? | No. Reads committed source snapshots. | No. Reads captured values and produces later `Msg`. | No. Produces later `Msg` from long-lived resources. |
| Can perform IO or use capabilities? | No. | Yes. Capabilities come from the enclosed `Effect` / `Resource`. | Yes. Acquisition and cleanup live in `Resource`; event forwarding lives in `Effect`. |
| Runs when? | Synchronously when `view`, `subscriptions`, or command construction reads it. | After `update` commits the returned model. | While installed by `gtkApp`; diffed/cancelled by subscription key. |
| May mutate model or widgets directly? | No. | No; must emit `Msg` and let `update` commit the next model. | No; must emit `Msg` and let `update` commit the next model. |
| Lifetime | Current turn plus memo cache for computed nodes. | One-shot or keyed background task lifetime. | Long-lived until removed, replaced, or host shutdown. |

Two boundary rules are especially important:

- **effects capture snapshots, not live signals** — if a command needs derived data, evaluate the reactive value first and pass the resulting plain value into `run`;
- **subscriptions describe producers, not observers** — a subscription may decide whether it should exist from the current model (or from computed values derived from it), but once running it can only influence the app by emitting `Msg`.

## Interaction with subscriptions

`subscriptions : Model -> List (Subscription Msg)` may use derived or computed values to decide:

- which subscriptions should be active,
- which keys/configuration they should use,
- which plain snapshot values should be captured when opening them.

Those decisions are still recomputed only at commit boundaries. A reactive signal does **not** keep a subscription live by itself, and reading a signal does **not** open new resources.

## Interaction with forms and local view state

Form helpers from [`aivi.ui.forms`](./forms.md) remain ordinary source values inside the model:

- `Field.value`, `Field.touched`, and `Field.dirty` are source snapshots,
- inline error lists and submit enablement are ideal derived/computed values,
- asynchronous validation, debounced search, or remote suggestions remain commands/subscriptions.

So the reactive layer improves reuse of pure UI logic without introducing hidden mutable form state or a second validation loop.

## Design constraints

The Phase 4 reactive model intentionally keeps these guardrails:

- no ambient observer graph outside `gtkApp`,
- no hidden mutation from subscriptions/effects into computed caches,
- no second scheduler beyond the existing command/subscription host,
- no separate capability model for reactive code,
- no replacement for `Model -> View -> Msg -> Update`; reactive dataflow is an optimization and structuring layer inside that architecture.
