# Instant Domain

The `Instant` domain represents **a specific moment in time** on the timeline, independent of time zones or calendars.

It corresponds to a UTC timestamp (Unix epoch). While `DateTime` (in `Calendar`) is about "Human Time" (what the clock says on the wall), `Instant` is about "Physics Time" (when the event actually happened).

## Overview

```aivi
use aivi.chronos.instant (Instant)

// ISO-8601 literal syntax
started = 2024-05-21T12:00:00Z

// Instants can be compared
if now > started {
    // ...
}
```

## Features

```aivi
// Wraps a 64-bit integer (nanoseconds since epoch)
Timestamp = { nanos: Int }
```

## Domain Definition

```aivi
domain Instant over Timestamp = {
  // Parsing ISO-8601 literals
  // 2024-05-21T12:00:00Z -> { nanos: ... }
  
  // Comparison (Temporal order)
  (<) : params Timestamp -> Timestamp -> Bool
  (<=) : params Timestamp -> Timestamp -> Bool
  (>) : params Timestamp -> Timestamp -> Bool
  (>=) : params Timestamp -> Timestamp -> Bool
  
  // Duration arithmetic
  (+) : Timestamp -> Duration -> Timestamp
  (-) : Timestamp -> Duration -> Timestamp
  (-) : Timestamp -> Timestamp -> Duration
}
```

## Usage Examples

```aivi
use aivi.chronos.instant

start = 2024-01-01T00:00:00Z
end   = 2024-01-01T00:00:10Z

elapsed = end - start
// elapsed is 10s (Duration)
```
