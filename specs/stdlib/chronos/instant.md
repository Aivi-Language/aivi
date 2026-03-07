# Instant Domain

<!-- quick-info: {"kind":"module","name":"aivi.chronos.instant"} -->
The `Instant` domain represents a specific moment on the timeline, independent of calendars and time zones.

Use it when you care about *when something actually happened* rather than *what the local wall clock said*. That makes it a good fit for timestamps, logs, ordering, and precise scheduling boundaries.

`Instant` corresponds to a UTC timestamp. By contrast, the calendar and timezone domains are about human-facing date and local-time concepts.

**Implementation note:** `Timestamp` is represented as `DateTime` in RFC3339 text form at runtime, which means timestamps look like `2026-01-01T12:30:00Z`. Instant operations parse and format that text representation. Durations use `Span` from `aivi.chronos.duration` with millisecond precision.
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
- duration math against a point on the UTC timeline.

If you need calendar-friendly dates, use [`aivi.chronos.calendar`](./calendar.md). If you need local time with daylight-saving rules, use [`aivi.chronos.timezone`](./timezone.md).

## Mental model

`Instant` answers **“exactly when?”**

A good rule of thumb is:

- store `Instant` values in databases and event logs,
- compare `Instant` values when ordering matters,
- convert to calendar or time-zone values only when you need a human-facing display.

## Overview

<<< ../../snippets/from_md/stdlib/chronos/instant/overview.aivi{aivi}

## Common operations

These examples show how to create, compare, and offset instants. Read them as a three-step story: create an exact timestamp, compare it with another timestamp, then combine it with a duration when you need a deadline or timeout boundary.

<<< ../../snippets/from_md/stdlib/chronos/instant/features.aivi{aivi}

## Domain definition

The domain definition shows the concrete timestamp shape and the operations built around it:

<<< ../../snippets/from_md/stdlib/chronos/instant/domain_definition.aivi{aivi}

## Reading the current time

Effectful wall-clock reads such as `now` require the `clock.now` capability, or the broader `clock` shorthand.

## Usage examples

A practical pattern is to store or compare `Instant` values internally, then format them into calendar or time-zone-aware values for display.

<<< ../../snippets/from_md/stdlib/chronos/instant/usage_examples.aivi{aivi}
