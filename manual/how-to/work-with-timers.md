# How to work with timers

Use a timer source when you need recurring or delayed external input. The timer itself is the
outside-world boundary; everything downstream stays pure.

## Example

```aivi
type Unit -> Int -> Int
func countTicks = tick total =>
    total + 1

@source timer.every 1sec with {
    immediate: True,
    coalesce: True
}
signal tick : Signal Unit

signal elapsed = tick
 +|> 0 countTicks

signal label = elapsed
  |> "Elapsed: {.}s"

value main =
    <Window title="Timer">
        <Label text={label} />
    </Window>

export main
```

## Why this shape works

- `timer.every` creates a typed stream of events.
- `+|>` accumulates those events into state over time.
- `coalesce: True` avoids a backlog if the UI is briefly busy.

## Common variations

- Use `timer.after` for one-shot delays.
- Feed a timer into `refreshOn` when polling an API.
- Keep timer logic small and move real state transitions into a named `step` function.
