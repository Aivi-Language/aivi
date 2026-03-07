# Scheduler Domain

<!-- quick-info: {"kind":"module","name":"aivi.chronos.scheduler"} -->
The `Scheduler` domain models durable scheduling primitives for cron, interval, and one-shot jobs.

It is meant for work that should survive process restarts or be coordinated across workers. Plan keys are idempotent, leases and heartbeats are explicit values, retries can include exponential backoff and jitter, and tenant limits can be reasoned about as typed data.
<!-- /quick-info -->
<div class="import-badge">use aivi.chronos.scheduler<span class="domain-badge">domain</span></div>

## Quick chooser

| If you need... | Use... |
| --- | --- |
| a fixed elapsed span like `5m` or `250ms` | [`aivi.chronos.duration`](./duration.md) |
| one exact UTC moment | [`aivi.chronos.instant`](./instant.md) |
| human date math such as “next month” or “end of month” | [`aivi.chronos.calendar`](./calendar.md) |
| local clock time in a named region | [`aivi.chronos.timezone`](./timezone.md) |
| durable plans, cron rules, leases, or retry schedules | `aivi.chronos.scheduler` |

## When to use `Scheduler`

Use this domain when a simple in-process timer is not enough. Typical cases include:

- recurring jobs that must survive application restarts,
- scheduled work handled by a worker fleet,
- retry planning with backoff,
- concurrency control per tenant,
- explicit lease and heartbeat management.

If the timing only matters while one GUI app is running, lighter tools such as `commandAfter` or `subscriptionEvery` are usually a better fit.

## Overview

<<< ../../snippets/from_md/stdlib/chronos/scheduler/overview.aivi{aivi}

## Common operations

These examples show how plans, triggers, retries, and execution state fit together:

<<< ../../snippets/from_md/stdlib/chronos/scheduler/features.aivi{aivi}

## Domain definition

The domain definition is useful when you want to understand exactly which values are persisted or exchanged with workers:

<<< ../../snippets/from_md/stdlib/chronos/scheduler/domain_definition.aivi{aivi}

## Usage examples

A good mental model is: build plans as pure data, store them durably, then let workers interpret those plans later.

<<< ../../snippets/from_md/stdlib/chronos/scheduler/usage_examples.aivi{aivi}

## Worker/runtime helpers

The scheduler module also exposes pure helpers for worker-side planning logic:

- `chooseWorkerAction : PlannedRun -> WorkerState -> Int -> WorkerDecision`
- `renewLease : Lease -> Timestamp -> Lease`
- `planRetryRun : PlannedRun -> WorkerState -> Int -> PlannedRun`

These helpers make the most failure-prone parts of scheduling easier to test because lease renewal, retry timing, and next-action decisions remain pure value transformations.
