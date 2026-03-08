# TimeZone and ZonedDateTime

<!-- quick-info: {"kind":"module","name":"aivi.chronos.timezone"} -->
The `TimeZone` and `ZonedDateTime` domains handle named geographic zones, daylight-saving transitions, and “what does this UTC timestamp look like on a local clock?” style questions.

They are the right tools when local time matters: meeting times, user-facing schedules, region-specific deadlines, and conversions between UTC storage and local display.

**Implementation note:** time-zone rules come from the IANA time-zone database (the standard global list of zone names such as `Europe/Berlin` and `America/New_York`) via `chrono-tz`. Ambiguous or invalid local times are runtime errors when a wall-clock time cannot be resolved to one instant.
<!-- /quick-info -->
<div class="import-badge">use aivi.chronos.timezone<span class="domain-badge">domain</span></div>

## Quick chooser

| If your main question is... | Use... |
| --- | --- |
| “How long should I wait?” | [`aivi.chronos.duration`](./duration.md) |
| “Exactly when did this happen?” | [`aivi.chronos.instant`](./instant.md) |
| “What calendar date comes next?” | [`aivi.chronos.calendar`](./calendar.md) |
| “What local time is this in a named place?” | `aivi.chronos.timezone` |
| “How should jobs keep happening over time?” | [`aivi.chronos.scheduler`](./scheduler.md) |

## When to use `TimeZone`

Reach for this module when you need to:

- convert a UTC [`Timestamp`](./instant.md) into a local time,
- interpret a local time in a named zone,
- display schedules for users in different regions,
- handle daylight-saving changes correctly.

For raw UTC moments, use [`aivi.chronos.instant`](./instant.md). For human calendar arithmetic without zone conversion, use [`aivi.chronos.calendar`](./calendar.md).

## Mental model

`TimeZone` answers **“what clock time is this in that place?”**

A common workflow is:

1. store or receive a UTC `Timestamp` from [`aivi.chronos.instant`](./instant.md),
2. choose a named zone such as `Europe/Berlin`,
3. convert only when you need a local display or local-time interpretation.

`ZonedDateTime` then keeps three pieces of information together:

- `dateTime`: the local wall-clock reading,
- `zone`: the named IANA zone,
- `offset`: the resolved offset for that local time.

When you need the UTC moment back, call `toInstant zdt`; there is no separate `instant` field on `ZonedDateTime`.

## Overview

<<< ../../snippets/from_md/stdlib/chronos/timezone/overview.aivi{aivi}

## Common operations

These examples show how to work with named zones and zoned local date-times. Pay special attention to where the examples move between UTC storage and local display, because that is the safest default pattern.

<<< ../../snippets/from_md/stdlib/chronos/timezone/features.aivi{aivi}

## Core operations at a glance

| Operation | Type | Use when |
| --- | --- | --- |
| `getOffset zone timestamp` | `TimeZone -> Timestamp -> Span` | You already have a UTC timestamp and need the zone offset that applied at that moment. |
| `toInstant zdt` | `ZonedDateTime -> Timestamp` | You have a local wall-clock time plus a zone and need the corresponding UTC timestamp. |
| `atZone zdt zone` | `ZonedDateTime -> TimeZone -> ZonedDateTime` | You want to keep the same instant but view it in a different named zone. |

<<< ../../snippets/from_md/stdlib/chronos/timezone/block_01.aivi{aivi}


## Domain definition

The domain definition shows the values used for zones, offsets, and zoned date-times. `Timestamp` comes from [`aivi.chronos.instant`](./instant.md), and `DateTime` comes from [`aivi.chronos.calendar`](./calendar.md).

<<< ../../snippets/from_md/stdlib/chronos/timezone/domain_definition.aivi{aivi}

## Usage examples

A common pattern is to store `Timestamp` values internally and convert them into `ZonedDateTime` values only when displaying or interpreting local time.

<<< ../../snippets/from_md/stdlib/chronos/timezone/usage_examples.aivi{aivi}

## Literal forms and verification

- Use `~tz(Europe/Paris)` for a named zone. See also the literal overview in [primitive types](../../syntax/types/primitive_types.md) and [operators](../../syntax/operators.md#sigils-and-literal-forms).
- Use `~zdt(...)` for a zoned local date-time; the runtime resolves the supplied local clock reading against the named zone and fails if that local time is ambiguous or invalid.
- Current behavior is exercised by `integration-tests/stdlib/aivi/chronos/timezone.aivi`.
