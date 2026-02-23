# AIVI Runtime/Stdlib Gaps for Mailfox (Generic Capabilities Only)

This document lists *generic platform/runtime gaps* that would improve AIVI for advanced local apps like Mailfox.

Design rule: Mailfox-specific product behavior (todo semantics, job-tracker logic, kanban UX, danger-zone policy) remains app-level and should **not** be added to stdlib.



Check existing RUST crates that can help implementing these:

## 1) Database and SQLite capabilities

Needed generic additions:

- robust `aivi.db.sqlite` module with:
  - migration runner
  - prepared statements and typed row decoders
  - explicit transaction/savepoint APIs
  - WAL and busy-timeout tuning
- first-class FTS5 helpers for indexing/querying
- batch write helpers for projection-heavy workloads

Why generic:

- these are useful to any local-first desktop app, not specific to Mailfox.

## 2) Scheduler execution runtime

Current scheduler modeling is useful; still needed generically:

- durable scheduler worker loop helpers
- lease persistence/renewal utilities
- retry/backoff orchestration over durable job tables
- cron next-fire computation utility with timezone correctness

Why generic:

- applies broadly to background workers and sync engines.

## 3) Structured concurrency and cancellation

Needed:

- task spawn/join/race helpers
- cancellation tokens/scopes
- bounded channels/queues with backpressure
- effect-level timeout/retry combinators

Why generic:

- required for stable long-running daemons in many domains.

## 4) Secrets and credential storage

Needed:

- `aivi.secrets` abstraction with OS keyring backend hooks (libsecret on GNOME)
- typed encrypted blob APIs with key identifiers

Why generic:

- all OAuth/API-key-using apps need secure local secret handling.

## 5) Email protocol completeness (generic transport layer)

Needed:

- richer IMAP APIs: IDLE, CONDSTORE/QRESYNC, incremental flags sync, partial fetch/streaming
- outbound email transport module (SMTP/JMAP-style abstraction)
- MIME traversal utilities for multipart and attachment extraction

Why generic:

- protocol/runtime primitives are generic; application interpretation remains app-level.

## 6) JSON schema and typed decoding support

Needed:

- schema validation module with precise error paths
- strict decoder combinators for versioned payloads
- schema migration helpers for persisted JSON evolution

Why generic:

- any app consuming external JSON APIs benefits from this.

## 7) GTK4/libadwaita runtime bindings maturity

Needed:

- stronger libadwaita coverage (`Adw*` navigation, preferences, split views, adaptive patterns)
- richer drag-and-drop and action-target APIs
- notification action callback plumbing

Why generic:

- generic UI toolkit capability, independent of Mailfox business logic.

## 8) Observability primitives

Needed:

- standardized structured log events with context propagation
- metrics counters/histograms/timers with local exporters
- tracing spans over effects

Why generic:

- critical for any production daemon/UI app stack.

## 9) File/content processing helpers

Needed:

- safe HTML->Markdown conversion utilities
- quote/signature stripping helpers as generic text-processing tools
- streamed attachment content extraction interfaces

Why generic:

- reusable for many content-centric apps.

## 10) Priority proposal for AIVI evolution

P0:

- sqlite transactions + migrations + FTS
- durable scheduler execution helpers
- structured concurrency/cancellation
- secrets/keyring abstraction

P1:

- richer IMAP + outbound mail transport primitives
- JSON schema/decoder/migration toolkit
- libadwaita binding expansion

P2:

- observability framework and advanced content-processing utilities

## Explicit non-goals for stdlib

Do **not** add these to stdlib/runtime as first-class features:

- Mailfox-specific todo categories or kanban lane semantics
- Mailfox-specific billing/subscription/job-application business rules
- Mailfox-specific danger-zone policy logic
- Mailfox-specific drawer/quick-reply UI compositions

Those belong in Mailfox app modules built on top of generic AIVI primitives.
