# Source Composition

<!-- quick-info: {"kind":"topic","name":"source composition"} -->
Source composition standardizes the extra steps around a source read: decode, transform, validate, retry, cache, and observe.
<!-- /quick-info -->

Source composition is for the cases where "just load the data" is not enough.

For example, you might want to:

- retry a flaky HTTP request,
- normalize old field names after decoding,
- reject values that are structurally valid but semantically wrong,
- cache successful results,
- record where the data came from and how it was obtained.

Composition keeps those rules attached to the source itself so they can be reused consistently.

## How this differs from schema-first declarations

Schema-first declarations describe **what the source is**:

- which connector it uses,
- which schema contract it carries,
- which compile-time artifacts may be checked against it.

Composition describes **how that source is executed and refined**:

- how the payload is decoded and normalized,
- how extra validation is layered on top of decoding,
- which retry and timeout rules wrap acquisition,
- when a successful value may be reused from cache,
- which provenance and observation data should be emitted.

Keeping those concerns separate avoids bloated connector records and keeps execution policy out of the type-level source contract.

A quick contrast:

- **schema-first** answers “what are we reading, and what shape should it decode into?”
- **composition** answers “once we read it, what extra policy should happen around that read?”

## Public model

Every structured source has one canonical decode stage supplied by its declaration. Composition layers additional stages around that declaration while preserving `Source K A` as pure description data.

You do not need to memorize the illustrative model below to use the feature. The everyday API is the set of combinators after it.

Conceptually:

```aivi
// Illustrative model only. The exact runtime representation is not part of the public contract.
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

The public surface is a set of pure combinators:

```aivi
source.transform  : (A -> B) -> Source K A -> Source K B
source.validate   : (A -> Validation (List DecodeError) B) -> Source K A -> Source K B
source.retry      : RetryPolicy -> Source K A -> Source K A
source.timeout    : Int -> Source K A -> Source K A
source.cache      : CachePolicy -> Source K A -> Source K A
source.provenance : ProvenancePolicy -> Source K A -> Source K A
source.observe    : ObservationPolicy -> Source K A -> Source K A
```

These functions do not perform I/O. They only return a richer source description for `load` to execute later.

## How `load` executes a composed source

Even if you write the combinators in a different order, `load` follows one canonical runtime sequence:

1. **seed provenance**
   - assign source identity, connector kind, schema fingerprint when known, and user labels;
2. **cache lookup**
   - if a fresh entry exists, return it and record a cache hit;
3. **acquisition**
   - perform connector I/O;
   - apply timeout, retry, and backoff only around this step;
4. **decode**
   - decode the payload using the source declaration's schema contract;
5. **transform**
   - run pure normalization stages left-to-right;
6. **validate**
   - run semantic validation stages left-to-right;
7. **cache commit**
   - on success, store the final value and its cache metadata;
8. **emit observation events**
   - record start, finish, and failure information for tooling.

In short: cache, retry, and timeout wrap the boundary work; transform and validate run only after decoding succeeds.

This fixed order matters because it ensures:

- cache hits skip connector I/O,
- decode and validation failures are not retried as if they were transport failures,
- provenance data is comparable across source kinds,
- tooling can explain where a failure happened without guessing from user-written combinator order.

## Choosing the right stage

### Decode

The decode stage comes from the source declaration:

- `file.json`, `file.csv`, `env.decode`, and `rest.get` all define a decode boundary,
- raw boundaries such as `file.read : Source File Text` use an identity decode step because `Text` is already the boundary type,
- compile-time schema checking does not remove runtime decoding.

Decode failures surface through:

```aivi
DecodeError (List aivi.validation.DecodeError)
```

### Transform

Use `source.transform` for pure, total reshaping after decode succeeds:

- renaming or dropping fields,
- sorting or filtering decoded collections,
- turning a compatibility shape into a domain-specific record.

Transforms run in declaration order and may change the result type.

If a step can reject data, it is not a transform. Use validation instead so failures remain structured.

### Validate

Use `source.validate` for semantic rules that sit above structural decoding:

```aivi
source.validate :
  (A -> Validation (List DecodeError) B) ->
  Source K A ->
  Source K B
```

Typical examples:

- checking that a list is not empty,
- verifying cross-field invariants,
- rejecting impossible dates or ranges,
- enforcing business rules that should work the same across file, env, REST, or database inputs.

Validation failures accumulate as `DecodeError` values and do not trigger retry.

## Retry, timeout, and backoff

Retry policy is source metadata, not a second effect system:

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

Rules:

- retries wrap **connector acquisition only**
- decode and validation failures are never retried automatically
- retries re-run the connector against the same source declaration and capability scope
- backoff delays use the `clock.sleep` capability
- source retry policy works with connector-specific transient-failure classification

`source.timeout ms` sets a per-attempt acquisition deadline. If you need a budget for the whole load, wrap `load source` with the usual `timeoutWith` from `aivi.concurrency`.

### Policy override rules

Policy stages are canonicalized rather than stacked operationally:

- the last `source.retry` wins,
- the last `source.timeout` wins,
- the last `source.cache` wins,
- `source.provenance` entries merge by field, with later fields overriding earlier ones,
- `source.observe` entries accumulate.

This makes it safe for helpers to provide defaults while allowing call sites to override them.

## Caching

Caching is read-through reuse of the final validated value.

That means a cache entry stores the value **after** decode, transform, and validate have all succeeded.

The public contract is intentionally small:

- cache lookup happens before connector I/O,
- only successful values are cached by default,
- failed decode and validation results are not cached,
- cache keys come from the source declaration fingerprint unless a stable key is provided explicitly,
- expiry is controlled by the cache policy's TTL,
- `@static` embedded sources already behave like permanently available values.

## Provenance and observability

Every composed source carries provenance metadata even when `load` still returns only `A`.

At minimum, provenance must capture:

- a stable source identity or user-facing name,
- source kind such as `File`, `RestApi`, `Env`, `Db`, or `Static`,
- acquisition mode such as `live`, `cache-hit`, or `static`,
- schema fingerprint when known,
- retry and timeout summary,
- cache status,
- human-friendly labels for logs, dashboards, or editor tooling.

`source.provenance` attaches metadata to the source declaration. `source.observe` opts the source into standard runtime sinks such as logs, traces, or metrics.

Observation must:

- never change the decoded value,
- never widen the source capability set,
- redact secrets such as tokens, passwords, and environment values,
- report stage boundaries in canonical execution order,
- identify the failing stage (`acquire`, `decode`, `validate`, `cache`) rather than collapsing everything into a generic I/O error.

## Testing composed sources

Composition does not need a special mocking API. Tests reuse ordinary handlers:

- replace file, network, env, or db handlers to control connector I/O,
- replace `clock.sleep` or `clock.now` when you want deterministic retry and timeout behavior,
- keep the same composed source value in production and tests,
- use `mock ... in` only when you are substituting a specific binding rather than interpreting a capability.

```aivi
loadUsersForTest : Effect (SourceError RestApi) (List User)
loadUsersForTest =
  with {
    network.http = fixtureHttp,   // serve a known response
    clock.sleep = immediateClock  // avoid real waiting during retries
  } in load usersSource
```

The important invariant is that the source pipeline stays the same. Tests swap interpreters, not source declarations.

## End-to-end example

Here is how those pieces fit together in one realistic source declaration:

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

Reading `load usersSource` in plain language:

1. identify the source as `"users-api"`,
2. look for a fresh cached value,
3. if needed, call the API with a 5-second per-attempt deadline,
4. retry up to three times with exponential backoff,
5. decode the payload using the declared schema,
6. normalize the decoded data,
7. run semantic validation,
8. cache the successful result,
9. emit observation events for tooling.

## Relationship to nearby specs

- [Schema-First Source Definitions](schema_first.md) explains how a source declaration carries connector config and schema.
- [External Sources](../external_sources.md) introduces `Source K A` and `load`.
- [Effects](../effects.md#load) defines `load` as the effectful boundary.
- [Effect Handlers](../effect_handlers.md) explains the interpreter model reused for testing.
- [Validation](../../stdlib/core/validation.md) defines the accumulation semantics used by `source.validate`.
