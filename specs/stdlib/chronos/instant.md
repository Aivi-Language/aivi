# Instant Domain

<!-- quick-info: {"kind":"module","name":"aivi.chronos.instant"} -->
The `Instant` domain models exact UTC moments on the timeline. Use it when you care about *when something actually happened* rather than *what a local wall clock said*.

That makes it the right fit for audit timestamps, event ordering, deadlines, and duration math against a precise point in time.

`Instant` values are written as ISO-8601 UTC literals such as `2024-05-21T12:00:00Z`. By contrast, [`aivi.chronos.calendar`](./calendar.md) and [`aivi.chronos.timezone`](./timezone.md) focus on human-facing dates, local times, and regional clock rules.

**Implementation note:** the current runtime stores `Timestamp` as `DateTime` text, compares and offsets it by parsing RFC3339 values, and formats results back to RFC3339 text with up to nanosecond precision. `Instant` arithmetic with [`Span`](./duration.md) is still expressed in whole milliseconds.
<!-- /quick-info -->
<div class="import-badge">use aivi.chronos.instant<span class="domain-badge">domain</span></div>

## Quick chooser

| If your main question is... | Use... |
| --- | --- |
| “How long should I wait?” | [`aivi.chronos.duration`](./duration.md) |
| “Exactly when did this happen?” | `aivi.chronos.instant` |
| “What calendar date should users see?” | [`aivi.chronos.calendar`](./calendar.md) |
| “What local time is this in Berlin or New York?” | [`aivi.chronos.timezone`](./timezone.md) |
| “How should jobs keep happening over time?” | [`aivi.chronos.scheduler`](./scheduler.md) |

## When to use `Instant`

Reach for `Instant` when you need:

- an audit timestamp,
- a stable ordering key for events,
- “run after this exact moment” logic,
- elapsed-time math between two UTC moments.

If you need human calendar concepts such as months or end-of-month handling, use [`aivi.chronos.calendar`](./calendar.md). If you need local time with daylight-saving rules, use [`aivi.chronos.timezone`](./timezone.md).

## Mental model

`Instant` answers **“exactly when?”**

A good rule of thumb is:

- store `Instant` values in databases and event logs,
- compare `Instant` values when ordering matters,
- convert to calendar or time-zone values only at display or interpretation boundaries.

## Overview

This is the smallest useful pattern: create two instants, compare them, then measure the elapsed span between them.

<<< ../../snippets/from_md/stdlib/chronos/instant/block_01.aivi{aivi}


`elapsed` is a `Span`, so you can inspect `elapsed.millis` or combine it with other duration logic from [`aivi.chronos.duration`](./duration.md).

## Common operations

The `Instant` domain is intentionally small. Most day-to-day code uses just these operators:

| Operation | Type | Use it for |
| --- | --- | --- |
| `<`, `<=`, `>`, `>=` | `Timestamp -> Timestamp -> Bool` | Ordering two exact moments on the UTC timeline |
| `instant + span` | `Timestamp -> Span -> Timestamp` | Moving forward by a fixed elapsed duration |
| `instant - span` | `Timestamp -> Span -> Timestamp` | Moving backward by a fixed elapsed duration |
| `left - right` | `Timestamp -> Timestamp -> Span` | Measuring elapsed time between two instants |

<<< ../../snippets/from_md/stdlib/chronos/instant/block_02.aivi{aivi}


The `retryDelay` record is the concrete `Span` shape. If you want named duration literals such as `500ms` or `30s`, use [`aivi.chronos.duration`](./duration.md) to construct the span first and then combine it with an `Instant`.

## Domain definition

`Timestamp` is the public type alias for `Instant` — they refer to the same type.

The public surface is a `Timestamp` alias plus comparison and span arithmetic:

<<< ../../snippets/from_md/stdlib/chronos/instant/block_03.aivi{aivi}


The overloaded `-` either subtracts a `Span` from an instant or measures the `Span` between two instants, depending on the right-hand operand.

## Getting the current time

This module itself is pure: it defines comparisons and arithmetic once you already have a `Timestamp`. When another API performs a wall-clock read to produce a current instant, that read is an ordinary runtime effect.

## Usage examples

A practical pattern is to store or compare `Instant` values internally, then hand them off to calendar or time-zone code only when you need human-facing interpretation.

<<< ../../snippets/from_md/stdlib/chronos/instant/block_04.aivi{aivi}


In real applications, keep `receivedAt`, `deadline`, and `expiredAt` in UTC `Instant` form for storage and comparison, then convert to [`aivi.chronos.timezone`](./timezone.md) only when you need to show local clock time.

## See also

- [`aivi.chronos.duration`](./duration.md) for fixed elapsed spans such as timeouts and retry delays
- [`aivi.chronos.calendar`](./calendar.md) for month/day/year arithmetic
- [`aivi.chronos.timezone`](./timezone.md) for local time and daylight-saving conversion
- [`Effects`](../../syntax/effects.md) for effectful clock access
