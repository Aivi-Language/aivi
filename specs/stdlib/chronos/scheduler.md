# Scheduler Domain

<!-- quick-info: {"kind":"module","name":"aivi.chronos.scheduler"} -->
The `Scheduler` domain models production-grade scheduling primitives for cron, interval, and one-shot jobs with deterministic planning semantics.

It is designed for distributed workers: plan keys are idempotent, leases and heartbeats are explicit values, retries support exponential backoff + jitter, and tenant limits are enforced through typed run-state analysis.

<!-- /quick-info -->
<div class="import-badge">use aivi.chronos.scheduler<span class="domain-badge">domain</span></div>

## Overview

<<< ../../snippets/from_md/stdlib/chronos/scheduler/overview.aivi{aivi}

## Features

<<< ../../snippets/from_md/stdlib/chronos/scheduler/features.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/chronos/scheduler/domain_definition.aivi{aivi}

## Usage Examples

<<< ../../snippets/from_md/stdlib/chronos/scheduler/usage_examples.aivi{aivi}

## Worker/runtime helpers (v0.1)

The scheduler module also exposes execution-planning helpers for durable workers:

- `chooseWorkerAction : PlannedRun -> WorkerState -> Int -> WorkerDecision`
- `renewLease : Lease -> Timestamp -> Lease`
- `planRetryRun : PlannedRun -> WorkerState -> Int -> PlannedRun`

These helpers keep lease, retry, and next-action logic deterministic and testable as pure values.
