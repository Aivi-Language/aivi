# Duration Domain

<!-- quick-info: {"kind":"module","name":"aivi.chronos.duration"} -->
The `Duration` domain gives spans of time an explicit unit, so values like `500ms`, `30s`, or `5min` are unambiguous.

That is especially helpful for timeouts, retry delays, sleeps, polling intervals, and any other code where “just an integer” would be unclear.
<!-- /quick-info -->
<div class="import-badge">use aivi.chronos.duration<span class="domain-badge">domain</span></div>

## Quick chooser

| If your main question is... | Use... |
| --- | --- |
| “How long should I wait?” | `aivi.chronos.duration` |
| “Exactly when did this happen?” | [`aivi.chronos.instant`](./instant.md) |
| “What calendar date comes next?” | [`aivi.chronos.calendar`](./calendar.md) |
| “What local time should this show in a region?” | [`aivi.chronos.timezone`](./timezone.md) |
| “How should jobs keep happening over time?” | [`aivi.chronos.scheduler`](./scheduler.md) |

## When to use `Duration`

Use this domain for fixed spans of elapsed time, such as:

- request timeouts,
- retry backoff,
- scheduling delays,
- polling intervals,
- “wait 250 milliseconds” style logic.

If your logic is about calendar concepts like months, years, or “next business day”, use [`aivi.chronos.calendar`](./calendar.md) instead.

## Mental model

`Duration` answers **“how long?”**

It is the right tool when you would otherwise be tempted to pass around a bare integer and hope everyone remembers whether it means seconds, milliseconds, or minutes.

Duration literals use the suffixes `ms`, `s`, `min`, and `h`. If your timing idea is “one month later” rather than “sixty seconds later”, switch to [`aivi.chronos.calendar`](./calendar.md).

## Overview

<<< ../../snippets/from_md/stdlib/chronos/duration/overview.aivi{aivi}

## Common operations

These examples show the basic shapes and conversions you will use most often. A common pattern is: choose a named duration such as a `requestTimeout`, then combine it with an [`Instant`](./instant.md) when you need a precise deadline.

When you see `{ millis: 0 } + 30s`, read it as “start from an empty span, then apply a delta literal.” That pattern turns a unit-tagged delta into a concrete `Span` value you can store or compare.

<<< ../../snippets/from_md/stdlib/chronos/duration/features.aivi{aivi}

## Domain definition

The domain definition shows how durations are represented and which literal forms are available (`ms`, `s`, `min`, and `h`):

<<< ../../snippets/from_md/stdlib/chronos/duration/domain_definition.aivi{aivi}

## Helper functions

| Function | What it helps with |
| --- | --- |
| **negateDelta** delta<br><code>Delta -> Delta</code> | Reverse a duration delta so you can subtract a delay, undo an adjustment, or express “the same amount backwards” without rewriting the unit yourself. |

## Usage examples

In day-to-day code, a duration is often the most readable way to document timing intent right where it matters.

<<< ../../snippets/from_md/stdlib/chronos/duration/usage_examples.aivi{aivi}
