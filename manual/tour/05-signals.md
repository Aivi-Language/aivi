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
`@\|>` starts the recurrent flow; `<\|@` is the recurrence step.

```aivi
fun add:Int #x:Int #n:Int => n + x

@source button.clicked "inc"
sig count : Signal Int =
    0
    @|> add 1
    <|@ add 1
```

Reading this:

- `0` — the initial value (the seed of the accumulator).
- `@\|>` — recurrent flow start: when the source fires, begin the accumulation.
- `<\|@` — recurrence step: apply the step function to the current accumulated value.
- `add 1` is partially applied — the step function receives the current `count` as its last
  argument each time the source fires.

## Example: direction signal in Snake

```aivi
@source window.keyDown with { repeat: False, focusOnly: True }
sig direction : Signal Direction =
    Right
    @|> keepDirection keyDown
    <|@ keepDirection keyDown
```

On each `keyDown` event, `@\|>` starts the recurrence and `<\|@` applies `keepDirection keyDown`
to the current direction, storing the result as the new direction.

## Example: game state signal

```aivi
@source timer.every 160 with { immediate: True, coalesce: True }
sig game : Signal Game =
    initialGame
    @|> stepGame boardSize direction
    <|@ stepGame boardSize direction
```

Every 160 ms the timer fires. `stepGame` runs with the current `direction`, producing the next
`game` state. The entire game loop is two lines.

## Two independent sources — two recurrences

Each `@source` drives one recurrent signal. To respond to two different events, declare two
signals and derive from both:

```aivi
fun add:Int #x:Int #n:Int => n + x

@source button.clicked "increment"
sig added : Signal Int =
    0
    @|> add 1
    <|@ add 1

@source button.clicked "decrement"
sig removed : Signal Int =
    0
    @|> add 1
    <|@ add 1

sig count : Signal Int = added - removed
```

`count` is a pure derived signal — the difference of two independent accumulators.

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

## Summary

- `sig name : Signal T = initialValue` declares a time-varying value.
- Derived signals use `\|>` chains; they recompute automatically.
- Recurrent signals use `@\|>` (flow start) and `<\|@` (recurrence step) to fold events into accumulated state.
- Signals form a dependency graph maintained by the runtime.
- You never write to a signal; you only declare how it is computed.

[Next: Sources →](/tour/06-sources)
