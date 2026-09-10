# aivi.time

Wall-clock and monotonic clock tasks, plus pure millisecond arithmetic. `EpochMs` is an `Int` alias.

| Value | Type | Description |
| --- | --- | --- |
| `nowMs` | `Task Text Int` | Unix epoch milliseconds from the system clock |
| `monotonicMs` | `Task Text Int` | Milliseconds since the first monotonic-clock request in this process |

Use `nowMs` for stored timestamps and `monotonicMs` for elapsed durations. The monotonic origin is process-local and must not be persisted or compared between processes.

```aivi
use aivi.time (
    nowMs
    monotonicMs
)

value savedAt : Task Text Int = nowMs
value stopwatchNow : Task Text Int = monotonicMs
```

Timestamp pattern formatting and parsing are not offered. Calendar records and formatting live in [aivi.date](date.md); epoch-to-calendar conversion is not currently provided.

## Pure millisecond helpers

| Value | Type | Description |
| --- | --- | --- |
| `EpochMs` | `Int` | Type alias for epoch milliseconds |
| `msPerSecond` | `Int` | `1000` |
| `msPerMinute` | `Int` | `60000` |
| `msPerHour` | `Int` | `3600000` |
| `msPerDay` | `Int` | `86400000` |
| `toSeconds` | `Int -> Int` | Convert milliseconds to whole seconds |
| `toMinutes` | `Int -> Int` | Convert milliseconds to whole minutes |
| `toHours` | `Int -> Int` | Convert milliseconds to whole hours |
| `toDays` | `Int -> Int` | Convert milliseconds to whole days |
| `fromSeconds` | `Int -> Int` | Convert seconds to milliseconds |
| `fromMinutes` | `Int -> Int` | Convert minutes to milliseconds |
| `fromHours` | `Int -> Int` | Convert hours to milliseconds |
| `fromDays` | `Int -> Int` | Convert days to milliseconds |
| `elapsed` | `Int -> Int -> Int` | Subtract `start` from `finish` |

```aivi
use aivi.time (
    fromSeconds
    fromMinutes
    elapsed
    toSeconds
)

value timeoutMs : Int = fromSeconds 30
value cacheTtlMs : Int = fromMinutes 5
value requestTimeMs : Int = elapsed 1200 1875
value requestTimeSeconds : Int = toSeconds requestTimeMs
```

## Example — wall clock plus steady clock

```aivi
use aivi.time (
    nowMs
    monotonicMs
    elapsed
)

value createdAt : Task Text Int = nowMs
value timerStart : Int = 1000
value timerNow : Int = 1450
value timerElapsed : Int = elapsed timerStart timerNow
value steadySnapshot : Task Text Int = monotonicMs
```
