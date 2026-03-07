# Calendar Domain

<!-- quick-info: {"kind":"module","name":"aivi.chronos.calendar"} -->
The `Calendar` domain is for human calendar math: dates, months, years, and “what happens next month?” style questions.

It exists because calendar arithmetic is full of edge cases. Months have different lengths, leap years happen, and a calendar day is not the same thing as a fixed number of seconds.
<!-- /quick-info -->
<div class="import-badge">use aivi.chronos.calendar<span class="domain-badge">domain</span></div>

## Quick chooser

| If your main question is... | Use... |
| --- | --- |
| “How long should I wait?” | [`aivi.chronos.duration`](./duration.md) |
| “Exactly when did this happen?” | [`aivi.chronos.instant`](./instant.md) |
| “What calendar date comes next?” | `aivi.chronos.calendar` |
| “What local time should this show in a region?” | [`aivi.chronos.timezone`](./timezone.md) |
| “How should work repeat durably?” | [`aivi.chronos.scheduler`](./scheduler.md) |

## When to use `Calendar`

Reach for `aivi.chronos.calendar` when your logic cares about what people see on calendars and clocks:

- billing dates,
- monthly renewals,
- “end of month” calculations,
- adding days, months, or years to a date,
- building user-facing schedules.

If you need a precise moment on the UTC timeline, use [`aivi.chronos.instant`](./instant.md). If you need time-zone-aware local time, use [`aivi.chronos.timezone`](./timezone.md).

## Mental model

`Calendar` answers **“which human date?”**

Use it when you want the meaning people expect from a calendar, not the meaning of a fixed number of seconds. For example, “one month later” belongs here even though months are different lengths.

## Overview

<<< ../../snippets/from_md/stdlib/chronos/calendar/overview.aivi{aivi}

## Common operations

These examples show the kinds of calendar questions the domain is built to answer. Read them in order: start from a date, apply calendar-aware adjustments, then inspect helper functions such as leap-year and end-of-month handling.

<<< ../../snippets/from_md/stdlib/chronos/calendar/features.aivi{aivi}

## Domain definition

The domain definition shows the underlying shapes used for dates, date-times, and calendar deltas:

<<< ../../snippets/from_md/stdlib/chronos/calendar/domain_definition.aivi{aivi}

## Helper functions

| Function | What it helps with |
| --- | --- |
| **isLeapYear** date<br><code>Date -> Bool</code> | Ask whether `date.year` is a leap year before doing year-sensitive calculations. |
| **daysInMonth** date<br><code>Date -> Int</code> | Find the number of days in the month for a given date. Useful for UI and validation. |
| **endOfMonth** date<br><code>Date -> Date</code> | Jump to the last day of the current month. Good for statements, invoices, and monthly reporting. |
| **addDays** date n<br><code>Date -> Int -> Date</code> | Move forward or backward by calendar days with normalization. |
| **addMonths** date n<br><code>Date -> Int -> Date</code> | Move by whole months while handling month length differences and day clamping. |
| **addYears** date n<br><code>Date -> Int -> Date</code> | Move by whole years while preserving calendar intent as closely as possible. |
| **negateDelta** delta<br><code>Delta -> Delta</code> | Reverse a calendar delta so you can undo or mirror an adjustment. |

## Usage examples

A good pattern is to model user-facing dates with `Calendar`, then convert to instants or zoned values only when you need execution or storage semantics.

<<< ../../snippets/from_md/stdlib/chronos/calendar/usage_examples.aivi{aivi}
