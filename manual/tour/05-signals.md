# Signals

A `Signal A` is a value of type `A` that changes over time. `val` is stable; `sig` participates in the reactive graph.

## Derived signals

```aivi
type NamePair =
  | NamePair Text Text

sig firstName = "Ada"
sig lastName = "Lovelace"

sig namePair =
  &|> firstName
  &|> lastName
  |> NamePair
```

## Folding wakeups with `scan`

`scan` is the normal way to turn wakeups into evolving state.

```aivi
fun step:Int tick:Unit current:Int =>
    current + 1

@source timer.every 120 with {
    immediate: True
}
sig tick: Signal Unit

sig counter: Signal Int =
    tick
     |> scan 0 step
```

## Explicit recurrence

When the scheduler itself drives the next step, use recurrence decorators and recurrence pipes.

Read recurrence pipes like this:

- the value before `@|>` is the seed
- `@|>` computes the first recurring state
- `?|>` decides whether to keep stepping
- `<|@` computes the next recurring state on each wakeup

```aivi
domain Duration over Int
    literal sec: Int -> Duration

type PollState = {
    attempts: Int,
    keepPolling: Bool
}

fun beginPoll:PollState initialAttempts:Int =>
    {
        attempts: initialAttempts,
        keepPolling: True
    }

fun nextPoll:PollState state:PollState =>
    state.attempts + 1 < 4
     T|> { attempts: state.attempts + 1, keepPolling: True }
     F|> { attempts: state.attempts + 1, keepPolling: False }

@recur.timer 1sec
sig pollState: Signal PollState =
    0
     @|> beginPoll
     ?|> .keepPolling
     <|@ nextPoll
```

`@recur.backoff ...` uses the same recurrence suffix shape, but the wakeups come from a backoff schedule instead of a fixed timer.

Signals are not general-purpose mutable cells. Prefer deriving new signals from existing ones instead of treating them like imperative state variables.
