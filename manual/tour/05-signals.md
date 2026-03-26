# Signals

A signal is a value that changes over time.

If `val` is a snapshot — a value fixed at declaration time — then `sig` is a live value that
is always current. When a signal's dependencies change, the signal recomputes automatically.

Think of it like a spreadsheet cell: when a cell it depends on changes, it updates immediately.

## Declaring a signal

```aivi
// TODO: add a verified AIVI example here
```

This declares a signal named `count` that holds an `Int`. Its initial value is `0`.

A signal that derives from another signal uses `|>`:

```aivi
// TODO: add a verified AIVI example here
```

`doubled` is always `count * 2`. You do not manually update it; the runtime maintains the
dependency.

## Signals from signals

Any pipe chain that starts with a signal produces a new signal:

```aivi
// TODO: add a verified AIVI example here
```

`scoreLine` recomputes whenever `game` changes. The `|>` pipes you already know work
identically on signals.

## Stateful signal folds with `scan`

A signal can keep state by folding another signal's updates with `scan`:

```aivi
fun addTick:Int tick:Unit total:Int =>
    total + 1

@source timer.every 1000
sig tick : Signal Unit

sig count : Signal Int =
    tick
     |> scan 0 addTick
```

Reading this:

- `tick` is the upstream signal that publishes raw events.
- `scan 0 addTick` seeds the state at `0`.
- `addTick` receives the latest event payload first, then the previous state, and returns the next
  state.

`scan` is the normal way to accumulate keyboard events, timers, request completions, and other
source-driven updates into signal state.

## Example: direction signal in Snake

```aivi
sig direction : Signal Direction =
    keyDown
     |> scan Right updateDirection
```

`Right` is the seed. `keyDown` publishes raw key events. `updateDirection` receives the latest
key event plus the previous direction and returns the next direction.

## Example: game state signal

```aivi
sig game : Signal Game =
    tick
     |> scan initialGame stepOnTick
```

Every tick updates `game` by applying `stepOnTick` to the latest timer event and the previous
game state.

## Explicit recurrence: `@|>` and `<|@`

`@|>` and `<|@` are still part of the language, but they mean explicit recurrence/cursor
semantics rather than ordinary source-driven state.

```aivi
// TODO: add a verified AIVI example here
```

Reading this:

- `initial` — the seed value before the recurrence starts.
- `@|> start` — the first recurrence stage.
- `?|> predicate` — an optional guard; the step is skipped when the predicate is false.
- `<|@ step` — a recurrence step stage: receives the current state and returns the next state.

Use explicit recurrence when you want the recurrence itself to be the abstraction. Use `scan`
when you are folding a signal's updates into state.

## Recurrence guards

A `?|>` between `@|>` and `<|@` acts as a guard. If the predicate is false, the current
iteration is skipped:

```aivi
// TODO: add a verified AIVI example here
```

Here `?|> .hasNext` skips the step once the recurrent cursor no longer has a next element.

## Signals are values, not variables

A key distinction: `sig count` does not declare a mutable variable. It declares a node in the
signal dependency graph. The runtime owns the actual storage; AIVI code only describes the
relationships.

You cannot write to a signal from user code. Runtime-owned sources publish raw events, `scan`
folds those events into state, and explicit recurrence stays owned by the scheduler.

## Derived signals vs recurrent signals

A derived signal has no memory — it is a pure transformation.
A recurrent signal has memory — it folds over a stream of events.

## Summary

- `sig name : Signal T = initialValue` declares a time-varying value.
- Derived signals use `|>` chains; they recompute automatically.
- Signals form a dependency graph maintained by the runtime.
- You never write to a signal; you only declare how it is computed.
- `upstream |> scan seed step` is the usual way to build stateful signals from event streams.
- `scan` step functions receive the latest event payload and the previous state.
- `@|>` starts an explicit recurrent suffix: seed on the left, start stage on the right.
- `<|@` advances the explicit recurrence; `?|>` between `@|>` and `<|@` acts as a guard.
- Stateful signals have memory; pure derived signals do not.

[Next: Sources →](/tour/06-sources)
