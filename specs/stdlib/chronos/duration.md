# Duration Domain

<!-- quick-info: {"kind":"module","name":"aivi.chronos.duration"} -->
The `Duration` domain gives spans of time an explicit unit, so values like `500ms`, `30s`, or `5m` are unambiguous.

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

## Overview

<<< ../../snippets/from_md/stdlib/chronos/duration/overview.aivi{aivi}

## Common operations

These examples show the basic shapes and conversions you will use most often. A common pattern is: choose a named duration such as a `requestTimeout`, then combine it with an `Instant` when you need a precise deadline.

<<< ../../snippets/from_md/stdlib/chronos/duration/features.aivi{aivi}

## Domain definition

The domain definition shows how durations are represented and which literal forms are available:

<<< ../../snippets/from_md/stdlib/chronos/duration/domain_definition.aivi{aivi}

## Usage examples

In day-to-day code, a duration is often the most readable way to document timing intent right where it matters.

<<< ../../snippets/from_md/stdlib/chronos/duration/usage_examples.aivi{aivi}
