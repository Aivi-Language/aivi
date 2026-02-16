# AIVI Stdlib Audit Summary (Collections, Pooling, Domains)

Date: 2026-02-14

This document summarizes:
1) Stdlib audit findings (inventory, inconsistencies, gaps, prioritized backlog)
2) P0 API design notes (final signatures + module plan)
3) Implementation task breakdown (PR/commit-sized steps)

Repo guardrails (see `AGENTS.md`):
- `specs/` is the source of truth; changes must be specified before (or alongside) implementation.
- No invented syntax/features; keep APIs functional/immutable; no null/exceptions (use `Option`/`Result`).
- Validate with `cargo test --workspace`; keep `integration-tests/` compiling; format AIVI snippets with `aivi fmt`.

## Baseline (Quality Gate Status)

Baseline quality gates are currently green:
- `cargo fmt --all -- --check` passes.
- `cargo test --workspace` passes.
- `target/debug/aivi check integration-tests` passes (stdlib diagnostics are suppressed by default; use `target/debug/aivi check --check-stdlib <target>` to include `<embedded:...>` typecheck errors).

---

## 1) Audit Report

### 1.1 Inventory Map (Current Stdlib Modules)

Embedded stdlib modules are defined in `crates/aivi/src/stdlib/*.rs` and loaded via
`crates/aivi/src/stdlib/mod.rs`.

Current module set:
- Core: `aivi`, `aivi.prelude`, `aivi.text`, `aivi.logic`, `aivi.regex`, `aivi.testing`, `aivi.collections`, `aivi.i18n`
- Chronos: `aivi.calendar`, `aivi.duration`
- System: `aivi.system`, `aivi.console`, `aivi.crypto`, `aivi.file`, `aivi.database`, `aivi.url`
- Concurrency: `aivi.concurrency`
- Network: `aivi.net`, `aivi.net.http`, `aivi.net.https`, `aivi.net.sockets`, `aivi.net.streams`, `aivi.net.http_server`
- Math stack: `aivi.math`, `aivi.vector`, `aivi.matrix`, `aivi.linear_algebra`, `aivi.linalg`, `aivi.probability`, `aivi.signal`, `aivi.geometry`, `aivi.graph`
- Number stack: `aivi.number`, `aivi.number.bigint`, `aivi.number.rational`, `aivi.number.decimal`, `aivi.number.complex`, `aivi.number.quaternion`

Runtime-provided APIs (where the “real” implementations live today):
- Collections builtins: `crates/aivi/src/runtime/builtins/collections.rs` (registers `Map`, `Set`, `Queue`, `Deque`, `Heap`)
- Text builtins: `crates/aivi/src/runtime/builtins/text.rs`
- Regex builtins: `crates/aivi/src/runtime/builtins/regex.rs`
- Streams builtins: `crates/aivi/src/runtime/builtins/streams.rs`
- Database builtins: `crates/aivi/src/runtime/builtins/database/*`
- Concurrency/channel builtins: `crates/aivi/src/runtime/builtins/concurrency.rs`

### 1.2 Consistency / Correctness Issues

Collections:
- Docs show `Map.empty()` in at least one snippet, but runtime defines `Map.empty` as a value (not a nullary function). Examples generally use `Map.empty` (correct).
- No canonical `List` module/API; integration tests re-implement `map`/`filter`/`sum` ad-hoc (e.g. `integration-tests/legacy/02_functions.aivi`), leading to inconsistency and duplicated code.

Database:
- Spec describes query planning and operations like `filter/find/sortBy/groupBy/join`, but `aivi.database` currently provides only `configure/table/load/applyDelta/runMigrations` with persisted `rows_json` per table name. This is a spec/impl gap that should be resolved explicitly in specs before expanding the runtime.
- `DbError = Text` is weak vs the typed-error guideline (prefer ADTs/records).

Generators/Streams:
- Spec’s generator story includes `loop`/`recurse` desugaring, but runtime currently materializes generators eagerly and does not support `Recurse` in `generate` blocks.
- Streams API is minimal (`fromSocket/toSocket/chunks`) and does not provide standard transformations (`map/filter/take/fold/toList`).

Style:
- Repo guidance prefers `<|` patching for record updates; some existing stdlib code uses record spread (e.g. `aivi.url` module) and re-implements list helpers inline, both of which become unnecessary once `List` is first-class.

### 1.3 Missing Methods (High-Impact Gaps)

P0 collections gaps:
- List: `map`, `filter`, `flatMap`, `foldl/foldr`, `scan`, `take/drop`, `takeWhile/dropWhile`, `chunk`, `intersperse`, `zip/zipWith/unzip`, `partition`, `find/findMap`, `indexOf`, `at`, `dedup`, `uniqueBy`
- Map: `getOrElse`, `alter`, `mergeWith`, `filterWithKey`, `foldWithKey`
- Set: `contains` alias + ergonomic parity with Map

P0 pooling gap:
- No `Std.Db.Pool`/`aivi.database.pool`: no safe acquisition/release, no max size/queue policy, no backoff, no health, no stats, no drain/close semantics.

P1 domain ergonomics:
- High-leverage typed literals/sigils (e.g. Money/Path) not yet present; adding them requires spec, typechecker, and runtime/compiler validation work.

### 1.4 Prioritized Backlog

P0 (do first):
- Collections: add canonical `List` API + fill Map/Set combinators (additive, pure, high leverage).
- Database pooling: add `aivi.database.pool` with deterministic resource safety + limits/backoff/health/stats.

P1:
- Domains/sigils/deltas: implement 1–2 high-leverage domains (e.g. Money with `~money(...)`, Path with `~path[...]`), including operator semantics and compile-time validation.

P2:
- Typed-error variants for parsing (e.g. `Text.parseIntResult : Text -> Result TextParseError Int`) while keeping existing `Option` functions.
- Stream transformations once generator/stream model is clarified.

---

## 2) P0 Design Notes

### 2.1 Collections P0: `List` as a first-class record

Design goal: make idiomatic pipelines possible without re-implementing list helpers in every module/example.

Placement:
- Spec: extend `specs/05_stdlib/00_core/28_collections.md` with a `List` section.
- Runtime: register a `List` record (parallel to `Map/Set/...`) with total functions using `Option/Result`.

Final `List` signatures (data-last):
- `List.empty : List A`
- `List.isEmpty : List A -> Bool`
- `List.length : List A -> Int`
- `List.map : (A -> B) -> List A -> List B`
- `List.filter : (A -> Bool) -> List A -> List A`
- `List.flatMap : (A -> List B) -> List A -> List B`
- `List.foldl : (B -> A -> B) -> B -> List A -> B`
- `List.foldr : (A -> B -> B) -> B -> List A -> B`
- `List.scanl : (B -> A -> B) -> B -> List A -> List B`
- `List.take : Int -> List A -> List A`
- `List.drop : Int -> List A -> List A`
- `List.takeWhile : (A -> Bool) -> List A -> List A`
- `List.dropWhile : (A -> Bool) -> List A -> List A`
- `List.partition : (A -> Bool) -> List A -> (List A, List A)` (stable)
- `List.find : (A -> Bool) -> List A -> Option A`
- `List.findMap : (A -> Option B) -> List A -> Option B`
- `List.at : Int -> List A -> Option A`
- `List.indexOf : A -> List A -> Option Int` (uses `==`)
- `List.zip : List A -> List B -> List (A, B)` (truncate)
- `List.zipWith : (A -> B -> C) -> List A -> List B -> List C` (truncate)
- `List.unzip : List (A, B) -> (List A, List B)`
- `List.intersperse : A -> List A -> List A`
- `List.chunk : Int -> List A -> List (List A)` (size <= 0 returns `[]` or `Err`; pick one in spec)
- `List.dedup : List A -> List A` (stable consecutive dedup)
- `List.uniqueBy : (A -> K) -> List A -> List A` (stable; requires hashable `K`)

### 2.2 Map/Set P0 additions (additive)

Extend runtime `Map` record with:
- `Map.getOrElse : K -> V -> Map K V -> V`
- `Map.alter : K -> (Option V -> Option V) -> Map K V -> Map K V`
- `Map.mergeWith : (K -> V -> V -> V) -> Map K V -> Map K V -> Map K V`
- `Map.filterWithKey : (K -> V -> Bool) -> Map K V -> Map K V`
- `Map.foldWithKey : (B -> K -> V -> B) -> B -> Map K V -> B` (order unspecified)

Extend runtime `Set` record with:
- `Set.contains : A -> Set A -> Bool` (alias of `has`)

### 2.3 Generators P0: `aivi.generator` module

Motivation: align library surface with generator encoding and make generator utilities canonical.

Spec/type:
- `Generator A = (R -> A -> R) -> R -> R` (per `specs/04_desugaring/06_generators.md`)

Final signatures:
- `Generator.foldl : (B -> A -> B) -> B -> Generator A -> B`
- `Generator.toList : Generator A -> List A`
- `Generator.fromList : List A -> Generator A`
- `Generator.map : (A -> B) -> Generator A -> Generator B`
- `Generator.filter : (A -> Bool) -> Generator A -> Generator A`
- `Generator.range : Int -> Int -> Generator Int` (half-open `[start, end)`; `end <= start` empty)

Note: runtime currently materializes `generate` eagerly and does not support generator `recurse`; P0 can still implement the above utilities correctly for finite generators, but the spec/impl gap must be tracked.

### 2.4 Database Pooling P0: `aivi.database.pool`

Goal: deterministic resource safety via `withConn`, plus limits, timeouts, backoff, health, and stats.

Types:
- `Pool Conn`
- `PoolError = Timeout | Closed | HealthFailed | InvalidConfig Text`
- `PoolStats = { size: Int, idle: Int, inUse: Int, waiters: Int, closed: Bool }`

Config (explicit boundaries, no hidden globals):
- `Config Conn = { maxSize: Int, minIdle: Int, acquireTimeout: Span, idleTimeout: Option Span, maxLifetime: Option Span, healthCheckInterval: Option Span, backoffPolicy: BackoffPolicy, queuePolicy: QueuePolicy, acquire: Unit -> Effect DbError Conn, release: Conn -> Effect DbError Unit, healthCheck: Conn -> Effect DbError Bool }`

Functions:
- `create : Config Conn -> Effect DbError (Result PoolError (Pool Conn))`
- `withConn : Pool Conn -> (Conn -> Effect DbError A) -> Effect DbError (Result PoolError A)`
- `stats : Pool Conn -> Effect DbError PoolStats`
- `drain : Pool Conn -> Effect DbError Unit`
- `close : Pool Conn -> Effect DbError Unit` (idempotent)

Non-negotiable behavior:
- `withConn` releases exactly once if it acquires, even on effect failure or cancellation.

---

## 3) Implementation Task Breakdown (PR/Commit Steps)

Pre-step (unblock baseline):
1) Fix formatter idempotence for `integration-tests/legacy/27_algorithms.aivi` and re-run `cargo test --workspace` to restore green baseline.

P0 Collections:
2) Specs: update `specs/05_stdlib/00_core/28_collections.md`
   - Add `List` section + add Map/Set new functions.
   - Fix docs using `Map.empty()` to `Map.empty`.
3) Runtime: add `List` record builtins (new `crates/aivi/src/runtime/builtins/list.rs` + registration in core builtins)
4) Runtime: extend `Map` and `Set` records with the P0 methods.
5) Tests: add/extend Rust runtime unit tests for `List/Map/Set` behaviors (total, Option/Result patterns, stability laws).

P0 Generator:
6) Specs: add `aivi.generator` page (and update indices) documenting signatures and examples.
7) Stdlib: add embedded module `aivi.generator` under `crates/aivi/src/stdlib/` and include in `EMBEDDED_MODULES`.
8) Tests: generator utilities tests (`range/map/filter/toList`).

P0 Database Pool:
9) Specs: add Pool section/page describing `Pool`, `Config`, `PoolError`, `PoolStats`, and resource-safety laws.
10) Runtime: implement pool handle + builtins for `database.pool.*` (limits/queue policy/backoff/health/stats).
11) Stdlib: add embedded module `aivi.database.pool` (thin wrappers/export surface).
12) Tests: pool correctness tests (max size, timeout, fairness/queue policy, health checks, and leak-prevention for `withConn`).

Quality gates (before finalizing):
13) Run `cargo test --workspace`
14) Run `cargo fmt --all -- --check`
15) Run `aivi fmt` for any touched AIVI doc snippets and ensure integration tests still parse/build.
