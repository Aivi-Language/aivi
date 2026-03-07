# Source Composition

<!-- quick-info: {"kind":"topic","name":"source composition"} -->
Phase 3 standardizes how a schema-bearing `Source K A` moves through decode, transform, validate, retry/timeout/backoff, caching, and provenance/observability stages before `load` produces a value.
<!-- /quick-info -->

> [!NOTE]
> Phase 3 status:
> - This page defines the public composition model for schema-bearing sources.
> - It builds on the schema-first declaration model in [Schema-First Source Definitions](schema_first.md) without redefining connector config or schema extraction.
> - Existing `load (file.json "...")`, `load (rest.get ...)`, `load (env.decode ...)`, and `db.load ...` flows remain valid compatibility forms until runtime/compiler support for the full composition pipeline lands.

## Why composition is a separate concern

Schema-first declarations answer **what a source is**:

- which connector it uses,
- which schema contract it carries,
- which compile-time contract artefacts the compiler may validate.

Source composition answers **how that declared source is executed and refined at runtime**:

- how the payload is decoded and normalized,
- how additional validation is layered on top of schema decoding,
- which retry, timeout, and backoff rules wrap acquisition,
- when successful values may be reused from cache,
- which provenance and trace metadata accompany each load.

Keeping those concerns separate avoids two bad outcomes:

1. schema declarations turning into giant connector-specific policy records, and
2. runtime execution policy leaking into the type-level schema contract.

## Public model

Every structured source has one **canonical decode stage** supplied by its declaration. Composition then layers additional stages around that declaration while preserving `Source K A` as pure description data.

Conceptually:

```aivi
-- illustrative model; exact runtime representation is not part of the public contract
Source K A =
  SourcePipeline {
    connector: SourceConnector K,
    decode: DecodeStage K Raw,
    transforms: List (TransformStage Raw A),
    validations: List (ValidationStage A),
    retry: RetryPolicy,
    timeout: TimeoutPolicy,
    cache: CachePolicy,
    provenance: ProvenancePolicy,
    observation: ObservationPolicy
  }
```

The public surface is a set of pure source combinators:

```aivi
source.transform  : (A -> B) -> Source K A -> Source K B
source.validate   : (A -> Validation (List DecodeError) B) -> Source K A -> Source K B
source.retry      : RetryPolicy -> Source K A -> Source K A
source.timeout    : Int -> Source K A -> Source K A
source.cache      : CachePolicy -> Source K A -> Source K A
source.provenance : ProvenancePolicy -> Source K A -> Source K A
source.observe    : ObservationPolicy -> Source K A -> Source K A
```

These combinators stay pure. They do not perform I/O; they only produce a richer source description for `load` to execute later.

## Canonical execution order

The order in which policy combinators are written does **not** change runtime stage order. `load` must execute a composed source in the following canonical sequence:

1. **seed provenance**
   - assign the source identity, connector kind, schema fingerprint (when available), and user-supplied labels;
2. **cache lookup**
   - if a non-expired cache entry exists, return it and record a cache-hit provenance event;
3. **acquisition**
   - perform connector I/O;
   - apply timeout / retry / backoff only around this acquisition step;
4. **decode**
   - decode the acquired payload using the source declaration's schema contract;
5. **transform**
   - run zero or more pure normalization stages left-to-right;
6. **validate**
   - run zero or more accumulated validation stages left-to-right;
7. **cache commit**
   - on success, write the final value to cache together with the provenance summary needed for future hits;
8. **emit observation events**
   - record start/finish/failure information for logs, traces, metrics, or editor tooling.

This order is fixed so that:

- cache hits bypass connector I/O,
- decode/validation failures are never silently retried as if they were transport failures,
- provenance is comparable across connectors,
- tooling can explain where a source failed without reverse-engineering user-written combinator order.

## Stage semantics

### Decode

The decode stage is part of the source declaration itself:

- `file.json`, `file.csv`, `env.decode`, `rest.get`, and schema-first declarations each provide a decode contract;
- raw boundaries such as `file.read : Source File Text` use an identity decode stage because `Text` is already the boundary type;
- compile-time schema validation never removes runtime decode; `load` still decodes live data.

Decode failures surface through the existing `SourceError K` shape as:

```aivi
DecodeError (List aivi.validation.DecodeError)
```

Phase 3 extends that same `DecodeError` bucket to include validation failures produced by `source.validate`.

### Transform

`source.transform` is for **pure, total normalization** after a successful decode:

- renaming or reshaping fields,
- filtering or sorting decoded collections,
- patching records,
- converting a decoded compatibility shape into a domain-specific view model.

Transforms run in declaration order and may change the source's result type.

If a step can reject data, it is **not** a transform. Model rejection with `source.validate` so failures remain structured and accumulate as `DecodeError` values.

### Validate

`source.validate` runs after transforms and uses `aivi.validation.Validation` as the standard accumulation surface:

```aivi
source.validate :
  (A -> Validation (List DecodeError) B) ->
  Source K A ->
  Source K B
```

Validation is for semantic rules that sit above structural decoding, for example:

- cross-field invariants,
- non-empty collections,
- domain-specific constraints such as "start date must be before end date",
- connector-independent checks reused across file, env, REST, or database inputs.

Multiple validation stages accumulate errors left-to-right. A failing validation does **not** trigger retry. By the time validation runs, the source has already acquired and decoded a specific payload.

## Retry, timeout, and backoff

Retry policy is source-level metadata, not a second effect system.

```aivi
RetryPolicy = {
  attempts: Int
  backoff: BackoffPolicy
}
```

`BackoffPolicy` must support at least:

- `source.backoff.none`
- `source.backoff.constant delayMs`
- `source.backoff.exponential { baseMs, factor, maxMs }`

### Rules

- retries wrap **connector acquisition only**
- decode and validation failures are never retried automatically
- retries re-run the connector against the same source declaration and capability scope
- backoff delays use the `clock.sleep` capability
- source retry policy is additive with connector-specific transient-failure classification

`source.timeout ms` defines a **per-attempt acquisition deadline**. If a caller needs a budget for the entire pipeline, wrap `load source` with ordinary `timeoutWith` from `aivi.concurrency`.

This split keeps source policy precise:

- `source.timeout` says "one connector attempt must not run longer than this",
- `timeoutWith` says "the whole effectful load must not outlive this enclosing budget".

### Composition rules for policy stages

Policy stages are canonicalized rather than stacked operationally:

- the **last** `source.retry` wins,
- the **last** `source.timeout` wins,
- the **last** `source.cache` wins,
- `source.provenance` entries merge by field, with later fields overriding earlier ones,
- `source.observe` entries accumulate.

This lets helpers provide defaults while call sites still override them locally without creating ambiguous nested retry or timeout behavior.

## Caching

Phase 3 standardizes caching as **read-through reuse of the final validated value**.

That means a cache entry stores the value **after** decode, transform, and validate have succeeded. Future cache hits behave observationally as if those stages had already run successfully for the same source fingerprint.

The public cache contract is intentionally narrow:

- cache lookup happens before connector I/O,
- only successful values are cached by default,
- failed decode / validation results are not cached,
- cache keys are derived from the source declaration fingerprint unless the cache policy provides an explicit stable key,
- cache expiry is based on the policy's TTL,
- `@static` embedded sources behave like permanent pre-populated values and therefore make ordinary cache lookup redundant.

The initial Phase 3 cache surface is **process-local and runtime-managed**. Persistent or shared cache backends are intentionally deferred so the composition model can land without introducing a new storage capability family.

## Provenance and observability

Every composed source carries provenance metadata even when `load` still returns only `A`.

The provenance contract must capture at least:

- a stable source identity or user-facing name,
- source kind (`File`, `RestApi`, `Env`, `Db`, `Static`, ...),
- acquisition mode (`live`, `cache-hit`, `static`),
- schema fingerprint when known,
- retry / timeout summary (attempt count, last timeout boundary),
- cache status (`disabled`, `miss`, `write`, `hit`, `expired`),
- human-oriented labels for dashboards, logs, or editor tooling.

`source.provenance` attaches this metadata to the source declaration. `source.observe` opts the source into standard runtime observation sinks such as structured trace events, logs, or metrics.

Observation must follow these rules:

- it never changes the decoded value,
- it never widens the source capability set,
- it must redact secrets such as bearer tokens, passwords, and environment values,
- it reports stage boundaries in canonical execution order,
- failures identify the failing stage (`acquire`, `decode`, `validate`, `cache`) rather than collapsing everything into an undifferentiated I/O message.

The exact concrete constructors for `ObservationPolicy` may settle with the runtime work. The contract fixed here is semantic: composed sources can opt into standardized observation without changing their value type or authority story.

## Handler-based testing and mocking

Source composition does **not** introduce a source-specific mocking API.

Tests reuse the Phase 1 handler model:

- install file / network / env / db handlers to replace connector I/O,
- install `clock.sleep` or `clock.now` handlers to make retry and timeout behavior deterministic,
- keep the same composed source value in production and tests,
- use `mock ... in` only when substituting a specific binding rather than interpreting a capability.

```aivi
loadUsersForTest : Effect (SourceError RestApi) (List User)
loadUsersForTest =
  with {
    network.http = fixtureHttp,
    clock.sleep = immediateClock
  } in load usersSource
```

The important invariant is that the source pipeline remains unchanged. Tests swap interpreters, not the source declaration itself.

## End-to-end example

```aivi
use aivi.validation

RawUser = { id: Int, name: Text, enabled: Bool, legacyId: Option Text }
User = { id: Int, name: Text, enabled: Bool }

normalizeUsers : List RawUser -> List User
normalizeUsers =
  users => users |> map (user => {
    id: user.id,
    name: user.name,
    enabled: user.enabled
  })

validateUsers : List User -> Validation (List DecodeError) (List User)
validateUsers = users =>
  if isEmpty users then
    Invalid [{ path: [], message: "expected at least one user" }]
  else
    Valid users

usersSource : Source RestApi (List User)
usersSource =
  rest.get {
    url: ~u(https://api.example.com/users),
    schema: source.schema.derive,
    strictStatus: True
  }
    |> source.transform normalizeUsers
    |> source.validate validateUsers
    |> source.retry {
      attempts: 3,
      backoff: source.backoff.exponential {
        baseMs: 200,
        factor: 2,
        maxMs: 2_000
      }
    }
    |> source.timeout 5_000
    |> source.cache {
      ttlMs: 60_000
    }
    |> source.provenance {
      name: "users-api"
    }
    |> source.observe {
      kind: "trace"
    }
```

Semantically, `load usersSource` now means:

1. identify `users-api`,
2. look for a fresh cached value,
3. on miss, perform the REST request with a 5s per-attempt deadline,
4. retry at most three attempts with exponential backoff,
5. decode according to the declared schema,
6. normalize the decoded list,
7. validate semantic constraints,
8. cache the successful value,
9. emit provenance / observation events.

## Relationship to adjacent specs

- [Schema-First Source Definitions](schema_first.md) defines how a single source declaration carries connector config and schema.
- [External Sources](../external_sources.md) defines the `Source K A` model and capability mapping for `load`.
- [Effects](../effects.md#load) defines `load` as the effectful boundary.
- [Effect Handlers](../effect_handlers.md) defines the testing/interpreter story reused by composed sources.
- [Validation](../../stdlib/core/validation.md) defines the accumulation semantics used by `source.validate`.

This page intentionally does **not** redefine:

- connector-specific config records,
- schema extraction or compile-time schema checking,
- persistent/shared cache backends,
- streaming source semantics,
- query DSLs or connector-specific contract languages.
