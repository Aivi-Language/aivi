# Signals

A signal is a value that changes over time.

If `val` is a snapshot — a value fixed at declaration time — then `sig` is a live value that
is always current. When a signal's dependencies change, the signal recomputes automatically.

Think of it like a spreadsheet cell: when a cell it depends on changes, it updates immediately.

## Declaring a signal

```aivi
sig count : Signal Int = 0
```

This declares a signal named `count` that holds an `Int`. Its initial value is `0`.

A signal that derives from another signal uses `\|>`:

```aivi
sig doubled : Signal Int =
    count
     |> \n => n * 2
```

`doubled` is always `count * 2`. You do not manually update it; the runtime maintains the
dependency.

## Signals from signals

Any pipe chain that starts with a signal produces a new signal:

```aivi
sig scoreLine : Signal Text =
    game
     |> .score
     |> \n => "Score: {n}"
```

`scoreLine` recomputes whenever `game` changes. The `\|>` pipes you already know work
identically on signals.

## Recurrence: @\|>...<\|@

The recurrence pattern is how signals accumulate state over time.
It is the signal equivalent of a fold:

```aivi
sig count : Signal Int =
    0
    @|> add increment
    <|@ add decrement
```

Reading this:

- `0` — the initial value.
- `@\|>` — recur start: on the first event from `increment`, apply `add increment` to `0`.
- `<\|@` — recur step: on subsequent events from `increment` or `decrement`, apply the
  function to the **current accumulated value**.

The full form is:

```
initialValue
@|> stepFn sourceSignal1
<|@ stepFn sourceSignal2
<|@ stepFn sourceSignal3
...
```

Each `<\|@` introduces one more source that can trigger an update.

## Example: direction signal in Snake

```aivi
sig direction : Signal Direction =
    Right
    @|> keepDirection keyDown
    <|@ keepDirection keyDown
```

- Starts as `Right`.
- Every time `keyDown` fires, applies `keepDirection keyDown` to the current direction.
- The second `<\|@` line feeds from the same source — in practice, this wires both the
  initial and subsequent events.

## Example: game state signal

```aivi
sig game : Signal Game =
    initialGame
    @|> stepGame boardSize direction
    <|@ stepGame boardSize direction
```

On every timer tick (wired via `@source`), `stepGame` is applied to the current `game` value
with the current `direction`. The output becomes the new `game`.
The entire game loop is two lines.

## Signals are values, not variables

A key distinction: `sig count` does not declare a mutable variable. It declares a node in the
signal dependency graph. The runtime owns the actual storage; AIVI code only describes the
relationships.

You cannot write to a signal from user code. Only declared sources (`@source`, `@recur.timer`)
can drive a recurrence.

## Derived signals vs recurrent signals

| Form | Meaning |
|---|---|
| `sig x = someSignal \|> f` | Derives from another signal; no local state |
| `sig x = init @\|> step src <\|@ step src` | Accumulates state over time |

A derived signal has no memory — it is a pure transformation.
A recurrent signal has memory — it folds over a stream of events.

## Multiple sources

A recurrent signal can listen to multiple independent sources:

```aivi
sig counter : Signal Int =
    0
    @|> \_ => \n => n + 1
    <|@ \_ => \n => n + 1
```

(Here both sources do the same thing; in real programs they would do different things.)

## Summary

- `sig name : Signal T = initialValue` declares a time-varying value.
- Derived signals use `\|>` chains; they recompute automatically.
- Recurrent signals use `@\|>...<\|@` to fold a stream of events into accumulated state.
- Signals form a dependency graph maintained by the runtime.
- You never write to a signal; you only declare how it is computed.

[Next: Sources →](/tour/06-sources)
