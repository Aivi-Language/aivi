# TimeZone and ZonedDateTime

<!-- quick-info: {"kind":"module","name":"aivi.chronos.timezone"} -->
The `TimeZone` and `ZonedDateTime` domains handle geographic time offsets, daylight-saving transitions, and “what does this instant look like in this place?” style questions.

They are the right tools when local time matters: meeting times, user-facing schedules, region-specific deadlines, and conversions between UTC storage and local display.

**Implementation note:** time zone rules come from the IANA database (via `chrono-tz`); offsets include DST, ambiguous or invalid local times are runtime errors, and `ZonedDateTime` literals use millisecond precision.
<!-- /quick-info -->
<div class="import-badge">use aivi.chronos.timezone<span class="domain-badge">domain</span></div>

## When to use `TimeZone`

Reach for this module when you need to:

- convert a UTC instant into a local time,
- interpret a local time in a named zone,
- display schedules for users in different regions,
- handle daylight-saving changes correctly.

For raw UTC moments, use [`aivi.chronos.instant`](./instant.md). For human calendar arithmetic without zone conversion, use [`aivi.chronos.calendar`](./calendar.md).

## Overview

<<< ../../snippets/from_md/stdlib/chronos/timezone/overview.aivi{aivi}

## Common operations

These examples show how to work with named zones and zoned local date-times:

<<< ../../snippets/from_md/stdlib/chronos/timezone/features.aivi{aivi}

## Domain definition

The domain definition shows the values used for zones, offsets, and zoned date-times:

<<< ../../snippets/from_md/stdlib/chronos/timezone/domain_definition.aivi{aivi}

## Usage examples

A common pattern is to store `Instant` values internally and convert them into `ZonedDateTime` values only when displaying or interpreting local time.

<<< ../../snippets/from_md/stdlib/chronos/timezone/usage_examples.aivi{aivi}
