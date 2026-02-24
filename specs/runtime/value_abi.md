# Value ABI Contract (v0.1)

This document defines the canonical value-level ABI between generated/native code and the AIVI runtime.

## Scope

The Value ABI covers:

- Value/error/effect/ref handle representation
- Ownership and retain/release/clone semantics
- Versioning + compatibility handshake
- Error/effect call semantics at the boundary

It does **not** define language-level syntax or user-facing type rules.

## ABI Versioning

Runtime constants:

- `AIVI_VALUE_ABI_MAJOR = 0`
- `AIVI_VALUE_ABI_MINOR = 1`
- `AIVI_VALUE_ABI_PATCH = 0`

Compatibility policy:

1. **Major must match exactly**.
2. Required minor must be `<=` runtime minor.
3. Patch does not affect compatibility.

Handshake entrypoint:

- `runtime_value_abi_handshake(required_major, required_minor) -> Result<(), RuntimeError>`

Mismatch diagnostics must include required and runtime versions and a precise reason.

## Handle Layout

ABI handle types are `repr(C)` opaque pointers:

- `AiviValueHandle`
- `AiviErrorHandle`
- `AiviEffectHandle`
- `AiviRefHandle`

All handles are nullable (`NULL` sentinel means “no value” / invalid handle).

Threading and nullability guarantees:

- Non-null `AiviValueHandle` points to a runtime-owned reference-counted allocation.
- Handles are plain copyable tokens; ownership transitions are explicit via retain/release/clone.
- Null handles are always accepted by retain/release/clone and produce null/no-op behavior.

## Ownership States and RC Rules

For owned handles:

- `retain(handle)` increments strong count.
- `release(handle)` decrements strong count and may destroy the allocation at zero.
- `clone(handle)` creates a semantically equivalent owned handle (copying `Value`, preserving immutability rules).

Required invariants:

1. **Deterministic destruction**: final `release` destroys exactly once.
2. **No implicit steals**: read/projector operations do not consume ownership unless explicitly documented.
3. **Null safety**: retain/release/clone on null never crash.

## Error and Effect Boundary Semantics

- Errors crossing ABI boundaries must be represented as owned error handles (or runtime diagnostics mapped to `RuntimeError`).
- Effect invocation boundaries must preserve cancellation and resource lifecycle behavior already defined by runtime semantics.
- ABI adapters must never swallow errors silently.

### Runtime diagnostic invariants (v0.1)

To keep interpreter/JIT/runtime-support behavior aligned, runtime boundaries must follow these invariants:

- Unsupported runtime states must raise explicit diagnostics (no silent `Unit` fallbacks).
- Plain blocks accept only `Bind` and `Expr` block items; `Filter`/`Yield`/`Recurse` are rejected with explicit diagnostics.
- Indexed reads are canonical across runtime paths:
  - `List`/`Tuple` require `Int` indices (`>= 0`) and fail with out-of-bounds diagnostics.
  - `Map` indices must coerce to valid key values, else produce key-type diagnostics.
  - Non-indexable bases fail with a typed shape diagnostic (`List/Tuple/Map` expected).

## Non-goals (v0.1)

- Stable binary layout for every internal `Value` variant payload
- Zero-copy projections for all aggregate/value families
- Cross-process/shared-memory handle transport
- Backward compatibility across major ABI versions
