# Duration Domain

A type-safe way to represent **Spans of Time**.

## What is this?

In many systems, a timeout is just an integer like `500`. But is that 500 milliseconds? 500 seconds?
`Duration` solves this ambiguity. It wraps the raw number in a type that knows its unit. `500` becomes `500ms` or `0.5s`.

## Why this exists

Ambiguous units cause outages. (e.g., A developer sets a timeout to `30`, thinking seconds, but the system reads it as 30 milliseconds, causing instant failure). This domain forces clarity: you must specify the unit, and the compiler ensures you don't compare Seconds to Apples.

## Overview

```aivi
import aivi.std.chronos.duration use { Duration }

// Clear, unambiguous literals
let timeout = 500`ms`
let delay = 2`seconds`

// Type-safe comparison
if delay > timeout {
    // ...
}
```

## Features

```aivi
Span = { millis: Int }
```

## Domain Definition

```aivi
domain Duration over Span = {
  type Delta = Millisecond Int | Second Int | Minute Int | Hour Int
  
  (+) : Span -> Delta -> Span
  (+) span (Millisecond n) = { millis: span.millis + n }
  (+) span (Second n)      = { millis: span.millis + n * 1000 }
  (+) span (Minute n)      = { millis: span.millis + n * 60000 }
  (+) span (Hour n)        = { millis: span.millis + n * 3600000 }
  
  (-) : Span -> Delta -> Span
  (-) span delta = span + (negateDelta delta)
  
  // Span arithmetic
  (+) : Span -> Span -> Span
  (+) s1 s2 = { millis: s1.millis + s2.millis }
  
  // Delta literals
  1ms = Millisecond 1
  1s = Second 1
  1min = Minute 1
  1h = Hour 1
}
```

## Usage Examples

```aivi
use aivi.std.duration

timeout = { millis: 0 } + 30s
delay = timeout + 500ms
longPoll = { millis: 0 } + 5min
```
