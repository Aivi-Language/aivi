# Scheduler Domain

<!-- quick-info: {"kind":"module","name":"aivi.chronos.scheduler"} -->
The `Scheduler` domain models durable scheduling primitives for cron, interval, and one-shot jobs.

It is meant for work that should survive process restarts or be coordinated across workers. Plan keys are idempotent, leases and heartbeats are explicit values, retries can include exponential backoff and jitter, and tenant limits can be reasoned about as typed data.
<!-- /quick-info -->
<div class="import-badge">use aivi.chronos.scheduler<span class="domain-badge">domain</span></div>

## Quick chooser

| If your main question is... | Use... |
| --- | --- |
| “How long should I wait?” | [`aivi.chronos.duration`](./duration.md) |
| “Exactly when did this happen?” | [`aivi.chronos.instant`](./instant.md) |
| “What calendar date comes next?” | [`aivi.chronos.calendar`](./calendar.md) |
| “What local time should this show in a region?” | [`aivi.chronos.timezone`](./timezone.md) |
| “How should jobs keep happening, even across restarts?” | `aivi.chronos.scheduler` |

## When to use `Scheduler`

Use this domain when a simple in-process timer is not enough. Typical cases include:

- recurring jobs that must survive application restarts,
- scheduled work handled by a worker fleet,
- retry planning with backoff,
- concurrency control per tenant,
- explicit lease and heartbeat management.

If the timing only matters while one GUI app is running, lighter tools such as `commandAfter` or `subscriptionEvery` are usually a better fit.

## Mental model

`Scheduler` answers **“how should this work keep happening over time?”**

A good rule is:

- use UI timers for short-lived in-process behavior,
- use `Scheduler` when the plan itself needs to be stored, retried, leased, or coordinated.

## Overview

<<< ../../snippets/from_md/stdlib/chronos/scheduler/overview.aivi{aivi}

## Common operations

These examples show how plans, triggers, retries, and execution state fit together. Read them as a pipeline: define a plan, decide when it should run, then hand it to worker-side logic that tracks retries and leases.

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
