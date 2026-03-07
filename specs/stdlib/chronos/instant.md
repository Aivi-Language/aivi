# Instant Domain

<!-- quick-info: {"kind":"module","name":"aivi.chronos.instant"} -->
The `Instant` domain represents a specific moment on the timeline, independent of calendars and time zones.

Use it when you care about *when something actually happened* rather than *what the local wall clock said*. That makes it a good fit for timestamps, logs, ordering, and precise scheduling boundaries.

`Instant` corresponds to a UTC timestamp. By contrast, the calendar and timezone domains are about human-facing date and local-time concepts.

**Implementation note:** `Timestamp` is represented as `DateTime` in RFC3339 text form at runtime, which means timestamps look like `2026-01-01T12:30:00Z`. Instant operations parse and format that text representation. Durations use `Span` from `aivi.chronos.duration` with millisecond precision.
<!-- /quick-info -->
<div class="import-badge">use aivi.chronos.instant<span class="domain-badge">domain</span></div>

## Quick chooser

| If you need... | Use... |
| --- | --- |
| a fixed elapsed span like `5m` or `250ms` | [`aivi.chronos.duration`](./duration.md) |
| one exact UTC moment | `aivi.chronos.instant` |
| human date math such as “next month” or “end of month” | [`aivi.chronos.calendar`](./calendar.md) |
| local clock time in a named region | [`aivi.chronos.timezone`](./timezone.md) |
| durable plans, cron rules, or retry schedules | [`aivi.chronos.scheduler`](./scheduler.md) |

## When to use `Instant`

Reach for `Instant` when you need:

- an audit timestamp,
- a stable ordering key for events,
- “run after this exact moment” logic,
- duration math against a point on the UTC timeline.

If you need calendar-friendly dates, use [`aivi.chronos.calendar`](./calendar.md). If you need local time with daylight-saving rules, use [`aivi.chronos.timezone`](./timezone.md).

## Overview

<<< ../../snippets/from_md/stdlib/chronos/instant/overview.aivi{aivi}

## Common operations

These examples show how to create, compare, and offset instants:

<<< ../../snippets/from_md/stdlib/chronos/instant/features.aivi{aivi}

## Domain definition

The domain definition shows the concrete timestamp shape and the operations built around it:

<<< ../../snippets/from_md/stdlib/chronos/instant/domain_definition.aivi{aivi}

## Reading the current time

Effectful wall-clock reads such as `now` require the `clock.now` capability, or the broader `clock` shorthand.

## Usage examples

A practical pattern is to store or compare `Instant` values internally, then format them into calendar or time-zone-aware values for display.

<<< ../../snippets/from_md/stdlib/chronos/instant/usage_examples.aivi{aivi}
