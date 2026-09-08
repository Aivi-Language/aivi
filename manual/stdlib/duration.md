# aivi.duration

Typed time spans.

`aivi.duration` gives you a `Duration` domain instead of passing around plain `Int` values.
That makes time-related code easier to read: `5sec` says more than `5000`.

A `Duration` is a domain over `Int`. Its hoisted suffix literals are the currently usable public
construction surface.

## Import

```aivi
use aivi.duration (
    Duration
    DurationError
)
```

Because `aivi.duration` declares `hoist`, the suffix constructors (`ms`, `sec`, `min`, `hr`, `dy`)
and type names are available project-wide without a `use` statement. The domain also declares
named constructors, conversions, and operators internally, but those members are not currently
exported through module imports. Treat them as implementation details until that boundary is
implemented.

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
| `(+)` | `Duration -> Duration -> Duration` | Add two durations |
| `(-)` | `Duration -> Duration -> Duration` | Subtract one duration from another |
| `(*)` | `Duration -> Int -> Duration` | Multiply a duration by a whole number |
| `(<)` | `Duration -> Duration -> Bool` | Compare two durations |

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

## Declared domain members

### `millis`

```text
millis : Int -> Duration
```

Build a duration from a raw millisecond count.

Use the equivalent `ms` suffix in public code: `150ms`.

### `trySeconds`

```text
trySeconds : Int -> Result DurationError Duration
```

A safe constructor for whole seconds. Use this when you want construction to report a
`DurationError` instead of assuming the input is valid.

`trySeconds` is declared by the domain but is not exported today. For a known non-negative
literal, use `10sec`; validate dynamic input in application code before applying the suffix.

## Conversion

The implementation declares `toMillis`, `toSeconds`, `toMinutes`, `toHours`, and `toDays`, but
these domain members are not exported today. Imported domain values intentionally do not expose a
`.carrier` projection, so public code cannot currently unwrap a `Duration`.

## Operators

The `Duration` domain declares arithmetic and comparison operators internally. Imported code does
not resolve those domain operators yet, so the following signatures describe planned public
behavior rather than callable APIs:

```text
(+) : Duration -> Duration -> Duration
(-) : Duration -> Duration -> Duration
(*) : Duration -> Int -> Duration
(<) : Duration -> Duration -> Bool
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
