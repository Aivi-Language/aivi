# aivi.duration

Typed time spans.

`aivi.duration` gives you a `Duration` domain instead of passing around plain `Int` values.
That makes time-related code easier to read: `5sec` says more than `5000`.

A `Duration` is a domain over `Int`. Use suffix literals for constants and `millis` or `trySeconds` for dynamic values.

## Import

```aivi
use aivi.duration (
    Duration
    DurationError
    millis
    trySeconds
    toMillis
    toSeconds
    toMinutes
    toHours
    toDays
)
```

Because `aivi.duration` declares `hoist`, the suffix constructors (`ms`, `sec`, `min`, `hr`, `dy`)
and type names are available project-wide without a `use` statement. Named constructors and conversions are explicitly importable.

## Overview

| Member | Type | Description |
| --- | --- | --- |
| `ms` | `Int -> Duration` | Suffix constructor for milliseconds, as in `250ms` |
| `sec` | `Int -> Duration` | Suffix constructor for seconds, as in `5sec` |
| `min` | `Int -> Duration` | Suffix constructor for minutes, as in `2min` |
| `hr` | `Int -> Duration` | Suffix constructor for hours, as in `1hr` |
| `dy` | `Int -> Duration` | Suffix constructor for days, as in `7dy` |
| `millis` | `Int -> Duration` | Build a duration from a raw millisecond count |
| `trySeconds` | `Int -> Result DurationError Duration` | Smart constructor that can fail |

## Suffix constructors

The shortest way to make a duration is with a suffix literal:

```aivi
value debounce : Duration = 250ms
value retryDelay : Duration = 5sec
value sessionLength : Duration = 30min
value backupWindow : Duration = 1hr
value trialPeriod : Duration = 14dy
```

These values stay typed as `Duration`, so they are harder to confuse with unrelated `Int`
values elsewhere in your program.

## Constructors

### `millis`

```text
millis : Int -> Duration
```

Build a duration from a raw millisecond count.

The `ms` suffix is equivalent for literals: `150ms`.

### `trySeconds`

```text
trySeconds : Int -> Result DurationError Duration
```

A safe constructor for whole seconds. Use this when you want construction to report a
`DurationError` instead of assuming the input is valid.

`trySeconds` rejects negative seconds and values whose millisecond conversion would overflow `Int`. Ordinary duration values can be signed.

## Conversion

`toMillis`, `toSeconds`, `toMinutes`, `toHours`, and `toDays` return `Int`. Whole-unit conversions use integer division, truncating toward zero.

```aivi
use aivi.duration (
    millis
    toMillis
)

value raw : Int = toMillis (millis 250)
```

## Error type

```aivi
type DurationError = Text
```

When a smart constructor fails, the module reports a plain text message.

## Example — readable scheduling values

```aivi
value animationFrame : Duration = 16ms
value autosaveEvery : Duration = 30sec
value timeout : Duration = 2min
```
