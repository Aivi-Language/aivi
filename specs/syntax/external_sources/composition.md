# Source Composition

<!-- quick-info: {"kind":"topic","name":"source composition"} -->
Source composition describes how extra stages wrap a source read. Today, the verified core is decode, transform, and validate; this page also specifies the wider execution-policy model for retry, timeout, caching, provenance, and observation.
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

Conceptually, every composed source bundles a decode stage from the declaration plus optional policy layers. The composition model is expressed with pure combinators. The currently verified surface is:

<<< ../../snippets/from_md/syntax/external_sources/composition/block_01.aivi{aivi}


The wider design also reserves policy combinators for retry, timeout, cache, provenance, and observation:

<<< ../../snippets/from_md/syntax/external_sources/composition/block_02.aivi{aivi}


None of these functions perform I/O. They only return a richer source description for `load` to execute later.

## How `load` executes a composed source

Today, the verified runtime behavior is the decode → transform → validate subset. The full composition model below is still useful because it defines the canonical order that connector docs and future runtime work should share.

Even if you write the combinators in a different order, the full model for `load` follows one canonical runtime sequence:

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

<<< ../../snippets/from_md/syntax/external_sources/composition/block_02.aivi{aivi}


### Transform

Use `source.transform` for pure, total reshaping after decode succeeds:

- renaming or dropping fields,
- sorting or filtering decoded collections,
- turning a compatibility shape into a domain-specific record.

Transforms run in declaration order and may change the result type.

If a step can reject data, it is not a transform. Use validation instead so failures remain structured.

### Validate

Use `source.validate` for semantic rules that sit above structural decoding:

<<< ../../snippets/from_md/syntax/external_sources/composition/block_03.aivi{aivi}


Typical examples:

- checking that a list is not empty,
- verifying cross-field invariants,
- rejecting impossible dates or ranges,
- enforcing business rules that should work the same across file, env, REST, or database inputs.

Validation failures accumulate as `DecodeError` values and do not trigger retry.

## Retry, timeout, and backoff

The policy stages in this section are part of the broader composition model. They are specified here so source guides can share one execution story, even though the currently verified runtime subset is still `source.transform` and `source.validate`.

Retry policy is source metadata, not a second effect system:

<<< ../../snippets/from_md/syntax/external_sources/composition/block_04.aivi{aivi}


`BackoffPolicy` must support at least:

- `source.backoff.none`
- `source.backoff.constant delayMs`
- `source.backoff.exponential { baseMs, factor, maxMs }`

Rules:

- retries wrap **connector acquisition only**
- decode and validation failures are never retried automatically
- retries re-run the connector against the same source declaration
- backoff delays use ordinary sleep effects
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

Like retry and timeout, caching belongs to the broader composition model rather than the currently verified transform/validate subset.

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

Provenance and observation are specified here as source-level policy layers so all connectors can share the same vocabulary once runtime support is wired through consistently.

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
- never widen the source declaration itself,
- redact secrets such as tokens, passwords, and environment values,
- report stage boundaries in canonical execution order,
- identify the failing stage (`acquire`, `decode`, `validate`, `cache`) rather than collapsing everything into a generic I/O error.

## Testing composed sources

Composition does not need a special mocking API. Tests should keep the same composed source value and replace the surrounding bindings that fetch, sleep, or read configuration when deterministic behavior is needed.

The important invariant is that the source pipeline stays the same. Tests swap nearby bindings, not source declarations.

## End-to-end example

Here is a composition example you can verify today with the currently implemented stages:

<<< ../../snippets/from_md/syntax/external_sources/composition/block_05.aivi{aivi}


If you read `load usersSource` in plain language today:

1. call the API,
2. decode the payload using the declared schema,
3. normalize the decoded data,
4. run semantic validation.

The extended policy-aware shape looks like this:

<<< ../../snippets/from_md/syntax/external_sources/composition/block_07.aivi{aivi}


Reading that extended version in plain language:

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
- [`mock ... in`](../decorators/test.md#mock-expressions) covers binding substitution used in tests.
- [Validation](../../stdlib/core/validation.md) defines the accumulation semantics used by `source.validate`.
