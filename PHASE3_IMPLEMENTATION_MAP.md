# Phase 3 Pipeline Implementation Map

## Overview

**Goal**: Implement a minimal but real vertical slice for schema-first source/data pipeline behavior based on the Phase 3 specs. This means:
- Schema-first source declarations (typed config + schema bundled with source)
- Composition model (transform, validate, retry, timeout, cache, provenance stages)
- Handler-based testing infrastructure
- Integration test coverage for load/JSON/validation with schema awareness

**Scope**: A focused C3 slice, not the full composition API. Implement core abstractions that enable the spec contracts.

---

## Spec Foundation (Already Landed)

### Files to Review
- `specs/syntax/external_sources/schema_first.md` ✓
- `specs/syntax/external_sources/composition.md` ✓
- `specs/syntax/external_sources.md` (updated with Phase 3 note) ✓

### Key Contracts
1. **Source Declarations**: `file.json`, `rest.get`, `env.decode` accept `{ path: ..., schema: source.schema.derive }`
2. **Error ADT**: `SourceError K = IOError Text | DecodeError (List aivi.validation.DecodeError)`
3. **Composition Stages**: 
   - Decode (schema-driven, part of declaration)
   - Transform (pure normalization)
   - Validate (accumulation via `Validation (List DecodeError)`)
   - Retry/timeout/cache (policies)
   - Provenance/observation (metadata)
4. **Canonical Execution**: Cache lookup → Acquisition (with retry/timeout) → Decode → Transform → Validate → Cache write → Observation

---

## Current Implementation State

### Runtime Layer (Working)
- **Core**: `crates/aivi/src/runtime/builtins/core.rs` (700 LOC)
  - `load` builtin (line ~100): currently just extracts `Source K A → Effect (SourceError K) A`
  - `__set_source_schema` (internal): attaches JSON schema to `SourceValue`
  
- **SourceValue**: `crates/aivi/src/runtime/values.rs`
  - Holds `kind: String`, `effect: Arc<EffectValue>`, `schema: Arc<Mutex<Option<JsonSchema>>>`, `raw_text`
  - No composition fields yet
  
- **Decode**: `crates/aivi/src/runtime/json_schema.rs` (800 LOC)
  - `JsonSchema` enum: Int, Float, Text, Bool, DateTime, List, Tuple, Record, Option, Enum, Any
  - `validate_json()`: validates serde_json::Value against JsonSchema, collecting `JsonMismatch` errors
  - `json_to_runtime_with_schema()`: converts JSON to AIVI `Value` with schema-aware wrapping (e.g., Option)
  
- **File Sources**: `crates/aivi/src/runtime/builtins/system/file.rs` (452 LOC)
  - `file.read`, `file.json`, `file.csv`, `file.imageMeta`, `file.image`
  - `file.json` already: reads file → parses JSON → validates against optional schema → converts to runtime value
  
- **Env Sources**: `crates/aivi/src/runtime/builtins/system/mod.rs`
  - `env.get`, `env.decode`: fetch environment vars, attempt scalar coercion
  - No schema attachment yet
  
- **REST/HTTP**: `crates/aivi/src/runtime/builtins/url_http.rs`
  - `http.get`, `http.post`, `http.fetch`: construct source with effect
  - `rest.get`, `rest.post`, `rest.fetch`: wrappers around http
  - No schema attachment yet
  
- **Error Handling**: Currently all source errors return `Value::Text(...)`; needs upgrade to ADT

### Typecheck Layer (Partial)
- **Type Signature**: `crates/aivi/src/typecheck/builtins/core_io_concurrency_html.rs`
  - `load : Source K A → Effect (SourceError K) A`
  - SourceError is registered as `SourceError : * → *` (higher-order type)
  
- **Source Schema Tracking**: `crates/aivi/src/typecheck/checker/infer_expr.rs`
  - `load_source_schemas: Vec<(String, String, CgType)>` captures call-site types
  - These are passed to codegen for `__set_source_schema` injection
  
- **Validation.aivi**: `crates/aivi/src/stdlib/validation.rs`
  - `DecodeError = { path: List Text, message: Text }`
  - `Validation E A = Valid A | Invalid E` (already landed)
  - `formatDecodeError` already present

### Stdlib Layer (Stubs)
- **core.aivi**: `crates/aivi/src/stdlib/core.rs`
  - Exports `Source`, `SourceError` (type stubs only)
  
- **file.aivi**: `crates/aivi/src/stdlib/file.rs`
  - Exports `readJson`, `readCsv` which call `load (file.json ...)` / `load (file.csv ...)`
  - No schema-first constructors yet
  
- **rest.aivi**: `crates/aivi/src/stdlib/rest.rs`
  - Exports `get`, `post`, `fetch` which call `load (rest.get ...)` etc.
  - No schema-first config record yet
  
- **database.aivi**: `crates/aivi/src/stdlib/database.rs`
  - `load : Table A → Effect DbError (List A)` already present
  - No schema-first source declaration yet

### Integration Tests
- `integration-tests/syntax/external_sources/env_get_and_default.aivi`: basic env.get test
- `integration-tests/runtime/json_builtins.aivi`: JSON parsing tests
- No composition/transform/validate/retry tests yet
- No schema-aware error accumulation tests

---

## Required Edits for C3 Slice

### 1. Stdlib (Core Types & Public Surface)

**File**: `crates/aivi/src/stdlib/core.rs`

Changes:
- Add source schema strategy types (public stubs):
  ```aivi
  SourceSchema = DeriveSchema | JsonSchema | TableSchema
  CachePolicy = { ttlMs: Option Int }
  RetryPolicy = { attempts: Int, backoff: BackoffPolicy }
  BackoffPolicy = None | Constant Int | Exponential { baseMs: Int, factor: Int, maxMs: Int }
  ProvenancePolicy = { name: Option Text, labels: Option (Map Text Text) }
  ObservationPolicy = { kind: Text }
  ```
- Export: `SourceSchema`, `CachePolicy`, `RetryPolicy`, `BackoffPolicy`, `ProvenancePolicy`, `ObservationPolicy`

**File**: `crates/aivi/src/stdlib/file.rs`

Changes:
- Add typed config record:
  ```aivi
  JsonConfig = { path: Text, schema: SourceSchema }
  CsvConfig = { path: Text, schema: SourceSchema }
  ```
- Add schema-first constructors (keeping old ones for compat):
  ```aivi
  json : JsonConfig -> Source File A
  csv : CsvConfig -> Source File A
  ```
- Export new types

**File**: `crates/aivi/src/stdlib/rest.rs`

Changes:
- Extend existing `Request` record with schema field:
  ```aivi
  Request = {
    ... existing fields ...
    schema: SourceSchema
  }
  ```
- Keep `get`, `post`, `fetch` signatures but mark old direct calls as deprecated

**File**: `crates/aivi/src/stdlib/database.rs`

Changes:
- Add:
  ```aivi
  DbSourceConfig = { table: Table A, schema: SourceSchema }
  source : DbSourceConfig -> Source Db A
  ```

### 2. Runtime Core (Composition Infrastructure)

**File**: `crates/aivi/src/runtime/values.rs`

Changes to `SourceValue` struct:
```rust
pub(crate) struct SourceValue {
    pub(crate) kind: String,
    pub(crate) effect: Arc<EffectValue>,
    pub(crate) schema: Arc<Mutex<Option<JsonSchema>>>,
    pub(crate) raw_text: Arc<Mutex<Option<String>>>,
    
    // NEW: Composition pipeline (stored as serialized JSON for now)
    pub(crate) transforms: Arc<Mutex<Vec<String>>>,      // ["transform_id1", "transform_id2", ...]
    pub(crate) validations: Arc<Mutex<Vec<String>>>,      // ["validation_id1", ...]
    pub(crate) retry_policy: Arc<Mutex<Option<String>>>,  // JSON-serialized RetryPolicy
    pub(crate) timeout_ms: Arc<Mutex<Option<i64>>>,
    pub(crate) cache_policy: Arc<Mutex<Option<String>>>,  // JSON-serialized CachePolicy
    pub(crate) provenance: Arc<Mutex<Option<String>>>,    // JSON-serialized ProvenancePolicy
    pub(crate) observation: Arc<Mutex<Option<String>>>,   // JSON-serialized ObservationPolicy
}
```

Add methods:
```rust
pub(crate) fn with_transform(self, transform_id: String) -> Self { ... }
pub(crate) fn with_validate(self, validation_id: String) -> Self { ... }
pub(crate) fn with_retry(self, policy: RetryPolicy) -> Self { ... }
pub(crate) fn with_timeout(self, ms: i64) -> Self { ... }
pub(crate) fn with_cache(self, policy: CachePolicy) -> Self { ... }
pub(crate) fn with_provenance(self, policy: ProvenancePolicy) -> Self { ... }
pub(crate) fn with_observation(self, policy: ObservationPolicy) -> Self { ... }
```

**File**: `crates/aivi/src/runtime/builtins/core.rs`

Changes to `load` builtin:
```rust
builtin("load", 1, |mut args, runtime| {
    let value = args.remove(0);
    match value {
        Value::Source(source) => {
            // NEW: If composition pipeline is populated, construct a composed effect
            // For now, just return the base effect (C3 slice doesn't execute composition yet)
            Ok(Value::Effect(source.effect.clone()))
        }
        Value::Effect(_) => Ok(value),
        _ => Err(RuntimeError::TypeError { ... }),
    }
})
```

Add new builtins for composition (wrappers that clone and mutate `SourceValue`):
```rust
"source.transform" => builtin(...) 
"source.validate" => builtin(...)
"source.retry" => builtin(...)
"source.timeout" => builtin(...)
"source.cache" => builtin(...)
"source.provenance" => builtin(...)
"source.observe" => builtin(...)
```

### 3. Stdlib Composition Constructors

**File**: `crates/aivi/src/stdlib/core.rs` or new `crates/aivi/src/stdlib/source.rs`

New module:
```aivi
module aivi.source

export SourceSchema, DeriveSchema, JsonSchema as JsonSchemaContract, TableSchemaContract
export CachePolicy, RetryPolicy, BackoffPolicy
export ProvenancePolicy, ObservationPolicy
export schema, backoff, cache, transform, validate, retry, timeout, provenance, observe

use aivi
use aivi.validation

-- Schema strategies
DeriveSchema = Unit
JsonSchemaContract = { schema: Text }  -- JSON Schema as string
TableSchemaContract = Unit  -- Placeholder for now

SourceSchema = DeriveSchema | JsonSchemaContract | TableSchemaContract

schema = {
  derive : SourceSchema = DeriveSchema,
  json : Text -> SourceSchema = schema => JsonSchemaContract { schema },
  table : Unit -> SourceSchema = _ => TableSchemaContract
}

BackoffPolicy = None | Constant { delayMs: Int } | Exponential { baseMs: Int, factor: Int, maxMs: Int }

backoff = {
  none : BackoffPolicy = None,
  constant : Int -> BackoffPolicy = ms => Constant { delayMs: ms },
  exponential : { baseMs: Int, factor: Int, maxMs: Int } -> BackoffPolicy = config => Exponential config
}

CachePolicy = { ttlMs: Option Int }
cache : CachePolicy -> Source K A -> Source K A = policy source => source  -- returns source with cache attached

RetryPolicy = { attempts: Int, backoff: BackoffPolicy }
retry : RetryPolicy -> Source K A -> Source K A = policy source => source  -- returns source with retry attached

transform : (A -> B) -> Source K A -> Source K B = f source => source  -- changes source result type
validate : (A -> Validation (List DecodeError) B) -> Source K A -> Source K B = rule source => source

ProvenancePolicy = { name: Option Text }
provenance : ProvenancePolicy -> Source K A -> Source K A = policy source => source

ObservationPolicy = { kind: Text }
observe : ObservationPolicy -> Source K A -> Source K A = policy source => source

timeout : Int -> Source K A -> Source K A = ms source => source

domain Source K A = {
  (+) : Source K A -> (A -> B) -> Source K B = source f => source |> transform f
}
```

### 4. Error ADT

**File**: `crates/aivi/src/stdlib/core.rs`

Changes:
```aivi
SourceError K = IOError Text | DecodeError (List aivi.validation.DecodeError)
```

**File**: `crates/aivi/src/runtime/values.rs` + codegen

Add `SourceError` discriminant to the `Value` enum:
```rust
pub enum Value {
    ...
    SourceError {
        kind: String,
        error: SourceErrorVariant,
    },
}

pub enum SourceErrorVariant {
    IOError(String),
    DecodeError(Vec<DecodeError>),
}

pub struct DecodeError {
    pub path: Vec<String>,
    pub message: String,
}
```

Update error conversion in file/env/rest sources to construct `Value::SourceError` instead of `Value::Text`.

### 5. Integration Tests

**File**: `integration-tests/syntax/external_sources/schema_first_file_json.aivi`

```aivi
@no_prelude
module integrationTests.syntax.externalSources.schemaFirstFileJson

use aivi
use aivi.validation
use aivi.testing

User = { id: Int, name: Text, enabled: Bool }

usersSource : Source File (List User)
usersSource =
  file.json {
    path: "./integration-tests/fixtures/users.json",
    schema: source.schema.derive
  }

@test "load schema-first JSON source"
loadSchemaFirstJson = do Effect {
  users <- load usersSource
  assertEq (length users) 3
  assertEq users[0].name "Alice"
}
```

**File**: `integration-tests/syntax/external_sources/composition_validate.aivi`

```aivi
@no_prelude
module integrationTests.syntax.externalSources.compositionValidate

use aivi
use aivi.validation
use aivi.testing

User = { id: Int, name: Text, enabled: Bool }

validateNonEmpty : List User -> Validation (List DecodeError) (List User)
validateNonEmpty = users =>
  if isEmpty users then
    Invalid [{ path: [], message: "expected at least one user" }]
  else
    Valid users

usersSourceWithValidation : Source File (List User)
usersSourceWithValidation =
  file.json {
    path: "./integration-tests/fixtures/users.json",
    schema: source.schema.derive
  }
    |> source.validate validateNonEmpty

@test "validate source after decode"
validateAfterDecode = do Effect {
  users <- load usersSourceWithValidation
  assertEq (length users) 3
}
```

**File**: `integration-tests/syntax/external_sources/composition_transform.aivi`

```aivi
@no_prelude
module integrationTests.syntax.externalSources.compositionTransform

use aivi
use aivi.testing

RawUser = { id: Int, name: Text, enabled: Bool, legacyId: Option Text }
User = { id: Int, name: Text }

normalizeUser : RawUser -> User
normalizeUser = raw => { id: raw.id, name: raw.name }

normalizeUsers : List RawUser -> List User
normalizeUsers = list => list |> map normalizeUser

usersSourceWithTransform : Source File (List User)
usersSourceWithTransform =
  file.json {
    path: "./integration-tests/fixtures/users_legacy.json",
    schema: source.schema.derive
  }
    |> source.transform normalizeUsers

@test "transform source after decode"
transformAfterDecode = do Effect {
  users <- load usersSourceWithTransform
  assertEq (length users) 2
  assertEq users[0].name "Alice"
}
```

**File**: `integration-tests/fixtures/users.json` (test data)

```json
[
  { "id": 1, "name": "Alice", "enabled": true },
  { "id": 2, "name": "Bob", "enabled": false },
  { "id": 3, "name": "Charlie", "enabled": true }
]
```

**File**: `integration-tests/fixtures/users_legacy.json` (test data)

```json
[
  { "id": 1, "name": "Alice", "enabled": true, "legacyId": "legacy-1" },
  { "id": 2, "name": "Bob", "enabled": true, "legacyId": "legacy-2" }
]
```

---

## Existing Abstractions to Reuse

1. **SourceValue** (`runtime/values.rs`): Core carrier for source metadata, already has schema slot
2. **JsonSchema** (`runtime/json_schema.rs`): Schema representation and validation, use as-is
3. **DecodeError** (`stdlib/validation.rs`): Error accumulation, already present
4. **Validation ADT** (`stdlib/validation.rs`): Accumulation surface for errors, already landed
5. **json_to_runtime_with_schema()** (`runtime/builtins/system/mod.rs`): Schema-aware JSON→Value, already works
6. **Capability handlers** (`runtime/environment.rs`): Handler dispatch for mocking I/O in tests, reuse existing pattern
7. **Effect/EffectValue** (`runtime/values.rs`): Thunk-based execution model, use for composition steps

---

## Gotchas from Current Worktree

1. **Recent spec changes**: `schema_first.md`, `composition.md` are newly added (not in git yet); `external_sources.md` has Phase 3 additions. All three are in working directory.

2. **Typecheck flow**: `load_source_schemas` already flows through typecheck → codegen → `__set_source_schema` injection. This mechanism is already in place; C3 slice doesn't need to change it.

3. **Error ADT migration**: Current code returns `Value::Text(...)` for source errors. Migration to `SourceError K = IOError | DecodeError` ADT is **non-trivial** across file/env/rest sources and affects capability error handling. Consider phasing this separately or using a compat shim.

4. **Composition storage**: The approach of storing transform/validate IDs as strings + separate handlers is temporary. For production, may need a proper closure/thunk registry. C3 slice can punt on execution and just build the ADT.

5. **Handler-based testing**: The specs say tests reuse Phase 1 handler model (`with { file.read = ... } in load source`). Currently working for file/env/network. Needs no changes for C3 slice, but test infrastructure will need updates to cover composed sources.

6. **Cache policy**: Phase 3 says cache is "process-local and runtime-managed". The C3 slice should **not** implement actual caching logic yet—just carry the policy metadata and trace it in provenance.

7. **Observation/provenance**: These are metadata-only in the C3 slice. No actual logging/tracing is expected. Just store the policy on SourceValue.

8. **Retry/timeout execution**: The C3 slice **does not need** to actually execute retry loops or timeout deadlines. Just attach the policy and trace it. Real execution lands in a later slice.

---

## Summary Table: Files to Edit

| Layer | File | Change Type | Scope |
|-------|------|-------------|-------|
| **Stdlib** | `crates/aivi/src/stdlib/core.rs` | Add type exports | `Source`, `SourceError` (ADT) |
| | `crates/aivi/src/stdlib/file.rs` | Add schema-first constructors | `json`, `csv` with config record |
| | `crates/aivi/src/stdlib/rest.rs` | Extend Request record | Add `schema` field |
| | `crates/aivi/src/stdlib/database.rs` | Add schema-first source | `source : DbSourceConfig → Source Db A` |
| | `crates/aivi/src/stdlib/source.rs` | **NEW** | `schema.*`, `backoff.*`, composition combinators |
| | `crates/aivi/src/stdlib/validation.rs` | Already ready | No changes (DecodeError exists) |
| **Runtime** | `crates/aivi/src/runtime/values.rs` | Extend SourceValue | Add composition fields; add SourceError variant |
| | `crates/aivi/src/runtime/builtins/core.rs` | Add composition builtins | `source.{transform,validate,retry,...}` |
| | `crates/aivi/src/runtime/builtins/system/file.rs` | Update error handling | Return SourceError ADT, not Text |
| | `crates/aivi/src/runtime/builtins/system/mod.rs` | Update error handling | env sources return SourceError ADT |
| | `crates/aivi/src/runtime/builtins/url_http.rs` | Update error handling | http/rest sources return SourceError ADT |
| | `crates/aivi/src/runtime/json_schema.rs` | Already ready | No changes (validation exists) |
| **Tests** | `integration-tests/syntax/external_sources/*.aivi` | **NEW** | schema-first, transform, validate, composition |
| | `integration-tests/fixtures/*.json` | **NEW** | test data for sources |

---

## Recommended Implementation Order

1. **Specs**: Land schema_first.md, composition.md, external_sources.md updates (already written, need commit)
2. **Stdlib types** (core.rs): Define SourceError ADT, schema strategy types
3. **Runtime SourceValue** (values.rs): Add composition fields
4. **File sources** (system/file.rs): Migrate errors to SourceError ADT
5. **Env sources** (system/mod.rs): Migrate errors to SourceError ADT
6. **REST sources** (url_http.rs): Migrate errors to SourceError ADT
7. **Composition builtins** (core.rs): Implement `source.{transform,validate,retry,timeout,cache,provenance,observe}`
8. **Stdlib module** (source.rs or core.rs): Export composition API
9. **Integration tests**: Add schema-first and composition tests
10. **Typecheck**: Verify `load` still type-checks correctly with new SourceError ADT

---

## C3 Slice Success Criteria

- [ ] Schema-first source declarations work for file.json, rest.get, env.decode with typed config
- [ ] Composition combinators (`source.transform`, `source.validate`, etc.) are callable and attach metadata to Source
- [ ] `load` executes the base source correctly (no actual composition execution yet, just metadata attached)
- [ ] SourceError ADT flows through (though error handling can be minimal for C3)
- [ ] Integration tests pass for schema-first JSON loading, transform, and validate stages
- [ ] No breaking changes to existing `load (file.json "...")` or `load (rest.get ...)` patterns
- [ ] Capability error handling still works (file.read, network.http, process.env.read)

