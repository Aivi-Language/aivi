# Standard Library: Duration Domain

## Module

```aivi
module aivi/std/duration = {
  export domain Duration
  export Span, Millisecond, Second, Minute, Hour
  export toMillis, fromMillis
}
```

## Types

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
  
  -- Span arithmetic
  (+) : Span -> Span -> Span
  (+) s1 s2 = { millis: s1.millis + s2.millis }
  
  -- Delta literals
  1ms = Millisecond 1
  1s = Second 1
  1min = Minute 1
  1h = Hour 1
}
```

## Usage Examples

```aivi
use aivi/std/duration

timeout = { millis: 0 } + 30s
delay = timeout + 500ms
longPoll = { millis: 0 } + 5min
```
