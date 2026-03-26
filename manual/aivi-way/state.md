# State

In AIVI, state lives in signal graphs and recurrence plans. Avoid thinking in terms of mutable boxes or component-local setters.

## Fold external wakeups into a signal

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

## Use explicit recurrence when the next step is scheduled

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

Here the scheduler owns the wakeups. `@|>` turns the seed into state, `?|>` decides whether another tick should advance, and `<|@` computes the next state.

Once you have a state signal, derive more signals from it. Do not treat `sig` as an imperative variable slot.
