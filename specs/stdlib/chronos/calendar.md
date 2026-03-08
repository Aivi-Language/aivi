# Calendar Domain

<!-- quick-info: {"kind":"module","name":"aivi.chronos.calendar"} -->
The `Calendar` domain is for human calendar math: dates, month boundaries, leap years, and “what date comes next?” style questions.

It exists because calendar arithmetic is full of edge cases. Months have different lengths, leap years happen, and “one month later” is not the same thing as “thirty days later”.
<!-- /quick-info -->
<div class="import-badge">use aivi.chronos.calendar<span class="domain-badge">module</span></div>

## Quick chooser

| If your main question is... | Use... |
| --- | --- |
| “How long should I wait?” | [`aivi.chronos.duration`](./duration.md) |
| “Exactly when did this happen?” | [`aivi.chronos.instant`](./instant.md) |
| “What calendar date comes next?” | `aivi.chronos.calendar` |
| “What local time should this show in a region?” | [`aivi.chronos.timezone`](./timezone.md) |
| “How should work repeat durably?” | [`aivi.chronos.scheduler`](./scheduler.md) |

## When to use `Calendar`

Reach for `aivi.chronos.calendar` when your logic cares about date-based rules people expect from calendars:

- billing dates,
- monthly renewals,
- “end of month” calculations,
- adding days, months, or years to a date,
- building user-facing schedules.

If you need a precise moment on the UTC timeline, use [`aivi.chronos.instant`](./instant.md). If you need time-zone-aware local time, use [`aivi.chronos.timezone`](./timezone.md).

## Importing it today

Use `aivi.chronos.calendar` for the exported types and helper functions such as `addMonths`, `endOfMonth`, and `now`.

Like other domains, the operator and suffix-literal sugar is activated separately. In the current implementation, the working import pattern is:

<<< ../../snippets/from_md/stdlib/chronos/calendar/block_01.aivi{aivi}


That gives you the named helpers from `aivi.chronos.calendar` together with the calendar-aware literals and operators shown below. For background on domain imports, see [Domains](../../syntax/domains.md).

## Mental model

`Calendar` answers **“which human date?”**

Use it when you want the meaning people expect from a calendar, not the meaning of a fixed number of seconds. For example, “one month later” belongs here even though months are different lengths.

## Overview

<<< ../../snippets/from_md/stdlib/chronos/calendar/block_02.aivi{aivi}


The `~d(...)` literal gives you a `Date`, while `~dt(...)` gives you a `DateTime` value you can pass to other chronos modules when you need a full timestamp.

## Common operations

These examples show the kinds of calendar questions the domain is built to answer. Read them top to bottom: start from a date, apply explicit helpers, then compare those results with the domain sugar.

<<< ../../snippets/from_md/stdlib/chronos/calendar/block_03.aivi{aivi}


These helpers are all pure. The only effectful entry point in this module is `now`.

## Domain definition

`Date` values are plain records such as `{ year: 2025, month: 2, day: 8 }`. `DateTime` is also exported for interop with the rest of chronos, but the calendar-specific domain logic itself is defined over `Date`.

The underlying delta shapes and literals are:

<<< ../../snippets/from_md/stdlib/chronos/calendar/block_04.aivi{aivi}


In other words, the domain gives you four calendar deltas today: days, months, years, and the special `eom` marker for “move to the end of this month”.

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
| **now**<br><code>Effect DateTime</code> | Read the current wall-clock `DateTime`. This is effectful and depends on the active `clock.now` handler. |

## Usage examples

A good pattern is to model user-facing dates with `Calendar`, then convert to instants or zoned values only when you need execution or storage semantics.

<<< ../../snippets/from_md/stdlib/chronos/calendar/block_05.aivi{aivi}


This style keeps the business rule readable:

- `trialEnds` is a fixed count of calendar days after signup,
- `renewsOn` uses month-aware clamping, so January 31 rolls to the last valid day of February,
- `statementClosesOn` says “end of this month” directly instead of reimplementing month-length logic by hand.
