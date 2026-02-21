# Instant Domain

> **Status: Implemented**   available in the stdlib and runtime.

<!-- quick-info: {"kind":"module","name":"aivi.chronos.instant"} -->
The `Instant` domain represents **a specific moment in time** on the timeline, independent of time zones or calendars.

It corresponds to a UTC timestamp (Unix epoch). While `DateTime` (in `Calendar`) is about "Human Time" (what the clock says on the wall), `Instant` is about "Physics Time" (when the event actually happened).

**Implementation note (v0.1):** `Timestamp` is represented as `DateTime` (RFC3339 text) at runtime, and Instant operations parse/format that representation. Durations use `Span` from `aivi.chronos.duration` (millisecond precision).

<!-- /quick-info -->
<div class="import-badge">use aivi.chronos.instant<span class="domain-badge">domain</span></div>

## Overview

<<< ../../snippets/from_md/stdlib/chronos/instant/overview.aivi{aivi}

## Features

<<< ../../snippets/from_md/stdlib/chronos/instant/features.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/chronos/instant/domain_definition.aivi{aivi}

## Usage Examples

<<< ../../snippets/from_md/stdlib/chronos/instant/usage_examples.aivi{aivi}
