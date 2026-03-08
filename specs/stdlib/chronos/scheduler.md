# Scheduler Domain

<!-- quick-info: {"kind":"module","name":"aivi.chronos.scheduler"} -->
The `Scheduler` domain models durable scheduling primitives for cron, interval, and one-shot jobs.

It is meant for work that should survive process restarts or be coordinated across workers. Here, **durable** means the schedule survives restarts, **idempotent plan keys** mean the same job can be recognised safely if it is submitted twice, **leases** mean temporary ownership by one worker, and **heartbeats** are periodic “I am still alive” updates from that worker. Retries can include exponential backoff and jitter, where **jitter** means small random variation so many workers do not retry at the exact same instant. These concepts matter because without them, distributed job runners risk duplicating work, losing track of ownership, or stampeding the same resource when failures happen.
<!-- /quick-info -->
<div class="import-badge">use aivi.chronos.scheduler<span class="domain-badge">domain</span></div>

## When to use `Scheduler`

Use this domain when a simple in-process timer is not enough. Typical cases include:

- recurring jobs that must survive application restarts,
- scheduled work handled by a worker fleet (a group of workers running the same job logic),
- retry planning with backoff,
- concurrency control per tenant,
- explicit lease and heartbeat management.

If the timing only matters while one GUI app is running, lighter tools from [App architecture](/stdlib/ui/app_architecture) such as `commandAfter` or `subscriptionEvery` are usually a better fit; [Native GTK apps](/stdlib/ui/native_gtk_apps) shows the same “app-local timer vs durable schedule” split in a full UI context.

## Mental model

`Scheduler` answers **“how should this work keep happening over time?”**

More concretely: store the plan as data, let workers claim runs with leases, and keep retry rules explicit instead of burying them in ad-hoc timers.

A good rule is:

- use UI timers for short-lived in-process behavior,
- use `Scheduler` when the plan itself needs to be stored, retried, leased, or coordinated.

## Overview

<<< ../../snippets/from_md/stdlib/chronos/scheduler/overview.aivi{aivi}

## Trigger kinds at a glance

| Trigger form | Best for | Example |
| --- | --- | --- |
| `cron expression zone` | wall-clock schedules that follow a named time zone | `cron "0 0 * * *" ~tz(Asia/Singapore)` |
| `interval span` | repeated work based on elapsed time between runs | `interval { millis: 900000 }` |
| `once timestamp` | one-shot work at a specific instant | `once 2026-02-01T12:00:00Z` |

## Common operations

These examples show how plans, triggers, retries, and execution state fit together. Read them as a pipeline: define a plan, decide when it should run, then hand it to worker-side logic that tracks retries and leases.

<<< ../../snippets/from_md/stdlib/chronos/scheduler/features.aivi{aivi}

## Domain definition

The domain definition is useful when you want to understand exactly which values are persisted or exchanged with workers:

<<< ../../snippets/from_md/stdlib/chronos/scheduler/domain_definition.aivi{aivi}

## Usage examples

A good mental model is: build plans as pure data, store them durably, then let workers interpret those plans later.

<<< ../../snippets/from_md/stdlib/chronos/scheduler/usage_examples.aivi{aivi}

## Verification

The scheduler surface shown here is exercised in `integration-tests/stdlib/aivi/chronos/scheduler/scheduler.aivi`, including trigger constructors, idempotent plan indexing, retry delay behavior, tenant concurrency checks, and the worker helper functions.

Use that test file when you need a fuller end-to-end example than the focused snippets on this page.

## Worker/runtime helpers

The scheduler module also exposes pure helpers for worker-side planning logic:

| Helper | What it helps with |
| --- | --- |
| `chooseWorkerAction : PlannedRun -> WorkerState -> Int -> WorkerDecision` | Decide whether a worker should run now, wait for a lease boundary, or schedule a retry. The final `Int` is the jitter seed, which keeps retry planning deterministic in tests. |
| `renewLease : Lease -> Timestamp -> Lease` | Extend a lease from a heartbeat timestamp without mutating scheduler state in place. |
| `planRetryRun : PlannedRun -> WorkerState -> Int -> PlannedRun` | Produce the next planned run after a failure, including the incremented attempt count and retry schedule. The final `Int` is again the jitter seed. |

These helpers make the most failure-prone parts of scheduling easier to test because lease renewal, retry timing, and next-action decisions remain pure value transformations.
