# AIVI Stdlib Audit + P0 Implementation Notes (2026-02-14)

This report follows `AGENTS.md` and `.gemini/skills/**` guidance:
- Specs-first (stdlib API is documented in `specs/` and implementations must match).
- No invented syntax/features; all integration tests stay purely functional with `Option`/`Result`, no loops, pipelines preferred.
- Quality gates: `cargo test --workspace` must pass.

## Phase 0: Baseline

- Baseline command: `cargo test --workspace`
- Result (2026-02-14): PASS (all workspace crates)

## Audit Report

### 1) Inventory Map (Stdlib Modules)

The stdlib surface is primarily specified in `specs/index.md` and `specs/05_stdlib/**`. Key modules:

- Core: `aivi` facade (`crates/aivi/src/stdlib/core.rs`) exporting built-in types and module handles.
- Core docs:
  - `specs/05_stdlib/00_core/01_prelude.md`
  - `specs/05_stdlib/00_core/02_text.md`
  - `specs/05_stdlib/00_core/03_logic.md`
  - `specs/05_stdlib/00_core/16_units.md`
  - `specs/05_stdlib/00_core/24_regex.md`
  - `specs/05_stdlib/00_core/27_testing.md`
  - `specs/05_stdlib/00_core/28_collections.md`
  - `specs/05_stdlib/00_core/29_i18n.md`
  - `specs/05_stdlib/00_core/30_generator.md` (added)
- System & IO docs:
  - `specs/05_stdlib/03_system/20_file.md`
  - `specs/05_stdlib/03_system/21_console.md`
  - `specs/05_stdlib/03_system/22_crypto.md`
  - `specs/05_stdlib/03_system/23_database.md` (pooling added)
  - `specs/05_stdlib/03_system/25_url.md`
  - `specs/05_stdlib/03_system/25_system.md`
  - `specs/05_stdlib/03_system/26_log.md`
  - `specs/05_stdlib/03_system/30_concurrency.md`
- UI docs:
  - `specs/05_stdlib/04_ui/03_html.md`
  - `specs/05_stdlib/04_ui/04_color.md`
  - `specs/05_stdlib/04_ui/05_liveview.md`

Implementation touchpoints:
- Runtime builtins: `crates/aivi/src/runtime/builtins/**`
- Embedded stdlib sources: `crates/aivi/src/stdlib/**`
- Example programs (integration-style validation): `integration-tests/**/*.aivi`

### 2) Consistency Issues / Inconsistencies Found

- `TypeSig` + lambda definitions:
  - With an explicit type signature, multi-arg lambda-style definitions (`f = x y => ...`) do not bind parameters as expected.
  - Workaround in examples: use function-binding form (`f x y = ...`) when a `: ...` signature is present.
- ADT declaration forms:
  - Constructor resolution is reliable with inline ADT declarations (`Msg = Inc | Dec | ...`).
  - Multi-line ADT style with leading `|` is parsed but can fail constructor resolution in typechecking contexts.
  - Recommendation: either (a) fix compiler/typechecker constructor registration for multi-line ADTs, or (b) constrain the spec to the inline form until implemented.
- Formatter limitations that affect example style:
  - `aivi fmt` can remove required whitespace before `[` and rewrite valid multi-line applications into syntactically invalid shapes.
  - Workaround in examples: parenthesize list literals when passing them as arguments (`f ([])` / `f ([x])`) and prefer single-line applications when formatter cannot preserve layout.

### 3) Missing / Weak Methods (Selected, Prioritized)

Collections and generators are the highest leverage for idiomatic pipelines. The audit compares against functional baselines (Elm/OCaml/F#/Haskell) and pragmatic APIs (Rust/Swift/Kotlin), while staying in AIVI’s purity/effects model.

P0 (next, high impact / low risk):
- `Text`: `split`, `join`, `trim`, `startsWith`, `endsWith`, `replace`, `lines`, `words`, `parseInt`, `parseFloat` (typed `Result` errors).
- `List`: `sortBy`, `groupBy`, `splitOn`, `dedupBy`, `traverseOption`, `sequenceResult`, `toArray/fromArray` (if array-like carrier exists in v0.1).
- `Map`: `keys`, `values`, `toList/fromList`, `union`, `intersect`, `difference`, `update`, `remove`, `insert` (if not already exposed uniformly).
- `Generator`: `take`, `drop`, `enumerate`, `repeat`, `iterate` (pure, total).

P1 (domains / ergonomics):
- `Path` and `Url` domain literals (compile-time parsing + safe join/resolve).
- `Money` domain (currency carrier + delta ops; explicit parsing/validation).
- `Duration`/`Calendar` deltas beyond day/month/year (weeks, quarters) with clear laws.

P2 (nice-to-haves):
- Property-based tests for List/Map laws (associativity/identity where applicable).
- More “boundary” decoder/encoder utilities (typed, no decorators beyond spec).

### 4) Prioritized Backlog (Impact/Risk/Complexity)

- P0 Collections (done + remaining):
  - Done: core `List` record API + Map/Set extensions (see “P0 Implemented”).
  - Remaining: Text core ops; Generator “missing core”; Map/Set completeness sweep.
- P0 Database pooling:
  - Done: `aivi.database.pool` with bounded acquire, timeout, idle retirement, health checks, backoff, queue policy, stats, drain/close, and `withConn` release guarantees.
  - Remaining: `maxLifetime` enforcement, stronger deterministic tests (fake clock), and clearer separation of pool-level errors vs backend errors.
- P1 Domains:
  - Not yet implemented in this batch (recommend starting with `Path`/`Url` because it aligns with WASI constraints and avoids financial correctness pitfalls).

## P0 Design Notes (Final Signatures + Placement)

This section summarizes the *final* module placements and signatures used for P0 in this repo. The normative reference is the spec pages listed.

### Collections

Doc/spec: `specs/05_stdlib/00_core/28_collections.md`

- `List` is a runtime record (`List.map`, `List.foldl`, ...) registered in `crates/aivi/src/runtime/builtins/core.rs`.
- `Map`/`Set` are runtime records with additional methods for parity and ergonomics.

### Generator

Doc/spec: `specs/05_stdlib/00_core/30_generator.md`

- Module: `aivi.generator` (embedded stdlib module, pure)
- Key functions implemented: `map`, `filter`, `foldl`, `toList`, `fromList`, `range`

### Database Pooling

Doc/spec: `specs/05_stdlib/03_system/23_database.md`

- Module: `aivi.database.pool` (embedded stdlib + runtime builtins)
- Error model (pool-level): `Result PoolError a` (no exceptions)
- Resource safety: `withConn` implemented in runtime to guarantee release on success/failure/cancellation

Config support status:
- Implemented: `maxSize`, `minIdle`, `acquireTimeout`, `idleTimeout`, `healthCheckInterval`, `backoffPolicy`, `queuePolicy`, `acquire`, `release`, `healthCheck`
- Typed but not yet enforced in runtime: `maxLifetime`

## P0 Implemented (End-to-End)

### Collections (List/Map/Set)

- Spec/docs: `specs/05_stdlib/00_core/28_collections.md`
- Runtime:
  - `crates/aivi/src/runtime/builtins/list.rs` (new `List` record + methods)
  - `crates/aivi/src/runtime/builtins/collections.rs` (Map/Set additions)
  - `crates/aivi/src/runtime/builtins/core.rs` (register `List`)
- Tests: `crates/aivi/src/runtime/tests.rs` (`list_core_ops`, `map_new_ops`)

### Generator utilities

- Spec/docs: `specs/05_stdlib/00_core/30_generator.md`
- Embedded module: `crates/aivi/src/stdlib/generator.rs` (`aivi.generator`)
- Snippet: `specs/snippets/from_md/05_stdlib/00_core/30_generator/block_01.aivi`

### Database pooling

- Spec/docs: `specs/05_stdlib/03_system/23_database.md`
- Runtime:
  - `crates/aivi/src/runtime/builtins/database/pool.rs`
  - `crates/aivi/src/runtime/builtins/database/delta_apply.rs` (expose `database.pool`)
- Embedded module: `crates/aivi/src/stdlib/database_pool.rs`
- Tests: `crates/aivi/src/runtime/tests.rs` (`database_pool_withconn_releases_on_failure`, `database_pool_acquire_times_out_when_full`)
- Snippet: `specs/snippets/from_md/05_stdlib/03_system/23_database/block_09.aivi`

## Implementation Task Breakdown (PR-Ready / Commit-Oriented)

No commits were created in this workspace session. If you want a PR-ready history, a clean sequence that matches `AGENTS.md` discipline would be:

1) Specs: Collections + Generator + Database pool docs
   - Touch: `specs/05_stdlib/00_core/28_collections.md`
   - Add: `specs/05_stdlib/00_core/30_generator.md`
   - Touch: `specs/05_stdlib/03_system/23_database.md`
   - Update indices: `specs/index.md`, `specs/README.md`
   - Add snippets under `specs/snippets/from_md/**`

2) Runtime builtins: Collections/List
   - Add `crates/aivi/src/runtime/builtins/list.rs`
   - Wire in `crates/aivi/src/runtime/builtins/core.rs`
   - Extend `crates/aivi/src/runtime/builtins/collections.rs`
   - Add focused runtime unit tests in `crates/aivi/src/runtime/tests.rs`

3) Embedded stdlib modules: Generator + Database pool facade
   - Add `crates/aivi/src/stdlib/generator.rs`
   - Add `crates/aivi/src/stdlib/database_pool.rs`
   - Register in `crates/aivi/src/stdlib/mod.rs`

4) Database pool runtime support + tests
   - Add `crates/aivi/src/runtime/builtins/database/pool.rs`
   - Expose via `crates/aivi/src/runtime/builtins/database/delta_apply.rs`
   - Add deterministic tests in `crates/aivi/src/runtime/tests.rs`

5) Fix/format integration tests and keep quality gates green
   - Adjust formatting/scoping as needed (notably `integration-tests/legacy/28_mixed_test.aivi`)
   - Run: `cargo test --workspace`

## Appendix A: Proposed Next P0 APIs (Detailed)

These are *proposals* (not implemented in this batch). They are selected to be small, coherent, and immediately useful for pipelines, while respecting AIVI’s purity and typed-error rules.

### Text

Module: `aivi.text` (docs: `specs/05_stdlib/00_core/02_text.md`)

1) `Text.split : Text -> Text -> List Text`
- Signature (data-last): `Text -> Text -> List Text` (`sep` then `text`)
- Behavior: split on exact substring separator; preserves empty segments.
- Laws:
  - `Text.join sep (Text.split sep t) == t` (when `sep` is non-empty).
- Complexity: O(n) in input length (plus substring search cost).
- Edge cases:
  - `sep == ""`: return `[t]` (avoid infinite splitting).
  - Leading/trailing separators produce empty strings in the result.
- Example:
  ```aivi
  "a,b,,c," |> Text.split "," |> List.filter (t => t != "")
  ```

2) `Text.join : Text -> List Text -> Text`
- Signature: `Text -> List Text -> Text` (`sep` then `parts`)
- Behavior: intercalate with `sep` (no leading/trailing sep).
- Complexity: O(total length).
- Edge cases: empty list returns `""`.
- Example:
  ```aivi
  ["a", "b", "c"] |> Text.join "-"
  ```

3) `Text.trim : Text -> Text`
- Signature: `Text -> Text`
- Behavior: remove leading/trailing Unicode whitespace.
- Complexity: O(n).
- Edge cases: whitespace-only becomes `""`.

4) `Text.startsWith : Text -> Text -> Bool`
- Signature: `Text -> Text -> Bool` (`prefix` then `text`)
- Behavior: returns `True` iff `text` begins with `prefix`.
- Laws:
  - `Text.startsWith "" t == True`
- Complexity: O(|prefix|).

5) `Text.endsWith : Text -> Text -> Bool`
- Signature: `Text -> Text -> Bool` (`suffix` then `text`)
- Behavior: returns `True` iff `text` ends with `suffix`.
- Laws:
  - `Text.endsWith "" t == True`
- Complexity: O(|suffix|).

6) `Text.replace : Text -> Text -> Text -> Text`
- Signature: `Text -> Text -> Text -> Text` (`needle` then `replacement` then `text`)
- Behavior: replace all non-overlapping occurrences.
- Complexity: O(n).
- Edge cases:
  - `needle == ""`: return `text` (avoid “between every char” behavior).

7) `Text.lines : Text -> List Text`
- Signature: `Text -> List Text`
- Behavior: split on `\n`, trimming a trailing `\r` from each line (CRLF).
- Example:
  ```aivi
  fileContents |> Text.lines |> List.take 10
  ```

8) `Text.words : Text -> List Text`
- Signature: `Text -> List Text`
- Behavior: split on Unicode whitespace; collapses runs of whitespace.
- Example:
  ```aivi
  " hello   world " |> Text.words
  ```

9) `Text.parseInt : Text -> Result ParseIntError Int`
- Proposed error ADT:
  - `ParseIntError = Empty | InvalidDigit | Overflow`
- Behavior: parses optional leading `+`/`-` and decimal digits.
- Complexity: O(n).
- Edge cases: leading/trailing whitespace is rejected (use `Text.trim` first).
- Example:
  ```aivi
  input |> Text.trim |> Text.parseInt |> attempt
  ```

10) `Text.parseFloat : Text -> Result ParseFloatError Float`
- Proposed error ADT:
  - `ParseFloatError = Empty | InvalidFormat | Overflow`
- Behavior: decimal float parsing with optional exponent.
- Complexity: O(n).

### Generator

Module: `aivi.generator` (docs: `specs/05_stdlib/00_core/30_generator.md`)

1) `Generator.take : Int -> Generator a -> Generator a`
- Behavior: yields up to `n` items, then ends.
- Complexity: O(min(n, k)) yields.
- Edge cases: `n <= 0` yields nothing.

2) `Generator.drop : Int -> Generator a -> Generator a`
- Behavior: skips first `n` items, yields the rest.
- Complexity: O(n + remaining).
- Edge cases: `n <= 0` is identity.

3) `Generator.enumerate : Generator a -> Generator (Int, a)`
- Behavior: yields `(0, a0), (1, a1), ...`.
- Laws: `enumerate (fromList xs) |> toList == List.zip (List.range 0 ...) xs` (if `List.range` exists).

4) `Generator.iterate : (a -> a) -> a -> Generator a`
- Behavior: yields `seed, f seed, f (f seed), ...` (infinite).

5) `Generator.repeat : a -> Generator a`
- Behavior: yields `a` forever (infinite).

### List

Doc/spec: `specs/05_stdlib/00_core/28_collections.md`

1) `List.sortBy : (a -> k) -> List a -> List a` (requires `Ord k`)
- Behavior: stable sort by key.
- Complexity: O(n log n).
- Edge cases: empty/singleton unchanged.

2) `List.groupBy : (a -> k) -> List a -> Map k (List a)` (requires `Hash k`)
- Behavior: stable grouping; preserves input order within each group.
- Complexity: O(n) average with hashing.
