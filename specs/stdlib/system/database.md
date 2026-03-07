# Database Domain

<!-- quick-info: {"kind":"module","name":"aivi.database"} -->
The `Database` domain provides a type-safe, composable way to work with relational data. It treats tables as immutable records of schema plus rows and provides a `do Query { ... }` notation for composing typed queries over those rows.

It builds on existing AIVI features:
- **Domains** for operator overloading and delta literals
- **Predicates** for filtering and joins
- **Patching** for declarative updates
- **Effects** for explicit error handling

<!-- /quick-info -->
<div class="import-badge">use aivi.database<span class="domain-badge">domain</span></div>

## Overview

<<< ../../snippets/from_md/stdlib/system/database/overview.aivi{aivi}

Table schemas are defined with ordinary values. `db.table` takes a table name and a
list of `Column` values; the row type comes from the table binding's type annotation.

`aivi.database` supports two execution styles:

- **default connection helpers** such as `db.configure`, `db.load`, and `db.beginTx`
- **explicit connection helpers** such as `db.connect`, `db.loadOn`, and `db.beginTxOn`

New code should prefer explicit `DbConnection` handles so transaction ownership stays local and
works cleanly with pooling.

## Types

<<< ../../snippets/from_md/stdlib/system/database/types.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/system/database/domain_definition.aivi{aivi}

### Applying Deltas

<<< ../../snippets/from_md/stdlib/system/database/applying_deltas.aivi{aivi}

## Querying

<!-- quick-info: {"kind":"feature","name":"Query DSL"} -->
`aivi.database` ships a typed `Query A` type and a `do Query { ... }` notation for
composing queries in a readable, composable way.

**Current v0.1 behavior:** the portable subset (`db.from`, `db.where_`, `db.guard_`,
`db.select`, `db.orderBy`, `db.limit`, `db.offset`, `db.count`, `db.exists`, and
`do Query` blocks composed from those forms) is lowered to a structured SQL-backed
plan when every participating table has an explicit column list.  Those same static
schemas let the checker catch missing row fields and obvious bad projected/joined
field references before runtime.  Helper-built queries that fall outside the lowered
subset still use the older in-memory `Query` runtime, while unsupported `do Query`
shapes surface a query error when run instead of silently falling back.

```aivi
-- Compose a query value
activeNames : Query Text
activeNames = do Query {
  user <- db.from userTable    -- bind each row
  db.guard_ user.active        -- skip inactive rows
  db.queryOf user.name         -- project the name field
}

-- Execute against an explicit connection
main = do Effect {
  conn  <- db.connect { driver: Sqlite, url: "app.db" }
  names <- db.runQueryOn conn activeNames
  ...
}
```

When you have already called `db.configure` to set a default connection, you can skip the
explicit handle and use `db.runQuery` instead:

```aivi
-- Execute against the default configured connection
main = do Effect {
  _ <- db.configure { driver: Sqlite, url: "app.db" }
  _ <- db.runMigrations [userTable]
  names <- db.runQuery activeNames   -- uses the default connection; same lowered-query semantics
  ...
}
```

> **v0.1 note:** `db.runQuery` and `db.runQueryOn` both use the same lowered query engine.
> When a query is inside the supported SQL subset, predicates/order/paging/aggregates are
> pushed into SQL.  When a query stays outside that subset, it keeps the older in-memory
> behavior.  Prefer `db.runQueryOn` when you hold an explicit `DbConnection` (e.g. from a
> pool) so that transaction ownership stays local and explicit.

`do Query` desugars via `queryChain`/`queryOf` (not the generic monad `chain`/`of`), so
the result type is always `Query A`.  You can also compose queries with the functional
pipeline helpers if you prefer:

```aivi
activeNames : Query Text
activeNames =
  db.from userTable
  |> db.where_ _.active
  |> db.select _.name
```

> **Current subset boundary:** helper pipelines keep the legacy runtime path when they do
> not lower cleanly.  `do Query` lowering is stricter: today the supported shape is plain
> `db.from` binds, `db.guard_` filters, simple `=` let-bindings, and a final `db.queryOf`
> or helper wrapped around it.

### Sorting, paging, and slicing (v0.1)

Three additional helpers let you sort and page query results:

- **`db.orderBy key q`** — sorts rows by the projection `key`.
- **`db.limit n q`** — keeps the first `n` rows of the (optionally sorted) result.
- **`db.offset n q`** — skips the first `n` rows.

> **v0.1 note:** inside the lowered subset these map directly to SQL `ORDER BY`, `LIMIT`,
> and `OFFSET`.  Queries outside the lowered subset keep the legacy in-memory behavior.

Combine them with the pipeline style:

```aivi
recentTopNames : Query Text
recentTopNames =
  db.from userTable
  |> db.where_ _.active
  |> db.orderBy _.createdAt   -- sort ascending by creation time
  |> db.offset 10             -- skip the first 10
  |> db.limit 5               -- then take the next 5
  |> db.select _.name
```

Or inside a `do Query` block, apply the helpers directly to the source query with `|>`:

```aivi
recentTopNames : Query Text
recentTopNames = do Query {
  user <-
    db.from userTable
    |> db.where_ _.active
    |> db.orderBy _.createdAt
    |> db.offset 10
    |> db.limit 5
  db.queryOf user.name
}
```

> **Prefer the pipeline form** for sorting and paging — it is more readable and
> composes without re-querying the table.  When writing a `do Query` block, apply
> `db.orderBy`, `db.offset`, and `db.limit` with `|>` to the *source* query on the
> right-hand side of `<-`; do **not** bind a sorted query to a new name and then
> discard it.
<!-- /quick-info -->

### Multi-table joins (portable subset, v0.1)

<!-- quick-info: {"kind":"feature","name":"Multi-table join"} -->
Cross-table queries are written as **nested `do Query` binds with `guard_`**: bind rows
from each table in turn, then use `db.guard_` to apply the join predicate.  In the
lowered subset, this repeated-`db.from` pattern becomes a SQL cross join plus pushed-down
`WHERE` predicates, preserving the left-to-right row order with deterministic hidden row
ids.

```aivi
-- Schema
Order = { id: Int, userId: Int, total: Int }

orderTable : Table Order
orderTable = db.table "orders" [
  { name: "id",     type: IntType, constraints: [AutoIncrement, NotNull], default: None }
  { name: "userId", type: IntType, constraints: [NotNull],                default: None }
  { name: "total",  type: IntType, constraints: [NotNull],                default: None }
]

-- Cross-table query: active users paired with their orders
activeUserOrders : Query { user: User, order: Order }
activeUserOrders = do Query {
  user  <- db.from userTable
  db.guard_ user.active
  order <- db.from orderTable
  db.guard_ (order.userId == user.id)
  db.queryOf { user: user, order: order }
}

-- Execute
runActiveUserOrders : Effect DbError (List { user: User, order: Order })
runActiveUserOrders = do Effect {
  conn  <- db.connect { driver: Sqlite, url: "app.db" }
  pairs <- db.runQueryOn conn activeUserOrders
  _     <- db.close conn
  pure pairs
}
```

> **v0.1 limitation:** only the repeated-`db.from` + `db.guard_` join shape is lowered
> today, and each bind must still be a plain table source.  Explicit join syntax, outer
> joins, grouping, and correlated subqueries are still outside the shipped subset.
<!-- /quick-info -->

### Aggregate helpers: `count` and `exists`

<!-- quick-info: {"kind":"feature","name":"db.count / db.exists"} -->
`db.count` and `db.exists` are available in v0.1.  Inside the lowered subset `db.count`
compiles to SQL `COUNT(*)`, while `db.exists` compiles to a SQL existence probe
(`SELECT 1 ... LIMIT 1`-style).  Outside the lowered subset they keep the legacy
in-memory behavior.

```aivi
-- Count matching rows
activeCount : Query Int
activeCount = db.count (db.from userTable |> db.where_ _.active)

-- Test whether any row matches
hasActiveUsers : Query Bool
hasActiveUsers = db.exists (db.from userTable |> db.where_ _.active)

-- Execute (same runQueryOn interface)
main = do Effect {
  conn    <- db.connect { driver: Sqlite, url: "app.db" }
  [n]     <- db.runQueryOn conn activeCount     -- Query Int produces a singleton list
  [found] <- db.runQueryOn conn hasActiveUsers
  ...
}
```

> **v0.1 limitation:** aggregate pushdown only applies when the input query is itself in
> the lowered subset; `db.count` / `db.exists` do not make an arbitrary query lowerable.
<!-- /quick-info -->

<<< ../../snippets/from_md/stdlib/system/database/querying.aivi{aivi}

## Joins and Preloading

<<< ../../snippets/from_md/stdlib/system/database/joins_and_preloading_01.aivi{aivi}

For eager loading:

<<< ../../snippets/from_md/stdlib/system/database/joins_and_preloading_02.aivi{aivi}

## Migrations

Schema definitions are typed values. Mark them `@static` to allow compile-time validation and migration planning.

<<< ../../snippets/from_md/stdlib/system/database/migrations.aivi{aivi}

## Schema-first source declarations

Phase 3 database-backed source declarations reuse table and migration values as schema carriers instead of hiding row shape only behind the eventual `db.load` / query call.

- A database source declaration must carry typed connector config: connection selection, table or query/projection, and typed parameters.
- The row contract should come from an existing `Table A` value or an equivalent explicit row schema.
- When the table/query and migration inputs are compile-time stable, the compiler should validate the projected columns against the declared row type and surface migration guidance before runtime.

`db.load table` remains valid compatibility sugar for full-table reads, but the preferred schema-first form is a named source declaration that tooling can analyze.

## Pooling

Database pooling is provided by `aivi.database.pool`. The pool is configured explicitly (no hidden globals),
and `withConn` guarantees deterministic release via AIVI resources even on failure or cancellation.
Use `db.connect` / `db.close` as the pool's acquire/release functions when you want pooled database handles.

<<< ../../snippets/from_md/stdlib/system/database/pooling.aivi{aivi}

## Notes

- In v0.1, the portable `Query` subset is SQL-backed when every participating table has an explicit column list.  Helper-built queries outside that subset still keep the older in-memory behavior, while unsupported `do Query` shapes surface a query error when run.
- Raw SQL strings via an external-source `db.query` mechanism are **not part of v0.1**; use `db.from` / `do Query` for typed queries instead.
- `db.applyDelta` / `db.applyDeltas` run predicates and patches **in memory** (in the AIVI runtime) — they do not compile to SQL `WHERE` / `SET` clauses in v0.1.
- The typed mutation helpers (`db.insertOn`, `db.deleteWhereOn`, `db.updateWhereOn`, `db.upsertOn`, and their ambient variants `db.insert`, `db.deleteWhere`, `db.updateWhere`, `db.upsert`) are convenience wrappers that construct and apply a delta in one step.  They carry the same in-memory limitation as `db.applyDelta` in v0.1.
- Transactions are scoped to a single `DbConnection`. `db.beginTxOn conn` never affects any other connection in the same pool.
- The ambient helpers (`db.beginTx`, `db.commitTx`, `db.rollbackTx`, `db.savepoint`, ...) are compatibility sugar over the current default connection selected by `db.configure`.
- Nested `beginTxOn` calls are not part of the transaction model; use savepoints for inner rollback boundaries.

## Capability mapping (Phase 1 surface)

- `db.configure`, `db.configureSqlite`, pool creation / acquisition → `db.connect`
- `db.load` and read-only query helpers → `db.query`
- `db.applyDelta`, `db.applyDeltas`, `db.insert`, `db.insertOn`, `db.deleteWhere`, `db.deleteWhereOn`, `db.updateWhere`, `db.updateWhereOn`, `db.upsert`, `db.upsertOn`, transactions, savepoints → `db.mutate`
- `db.runMigrations`, `db.runMigrationSql` → `db.migrate`
- the broader `db` family shorthand covers all database capabilities

## Core API (v0.1)

### Table management

| Function | Explanation |
| --- | --- |
| **db.table** name columns<br><code>Text -> List Column -> Table A</code> | Creates a table definition. The row type `A` is inferred from the binding's type annotation. |
| **db.configure** config<br><code>DbConfig -> Effect DbError Unit</code> | Selects the runtime backend (Sqlite, Postgresql, Mysql). |
| **db.connect** config<br><code>DbConfig -> Effect DbError DbConnection</code> | Opens an explicit database connection handle. Prefer this for pooled or transaction-heavy code. |
| **db.open** config<br><code>DbConfig -> Resource DbError DbConnection</code> | Resource wrapper around `db.connect` / `db.close` for deterministic cleanup. |
| **db.close** conn<br><code>DbConnection -> Effect DbError Unit</code> | Closes an explicit connection handle. |
| **db.runMigrations** tables<br><code>List (Table A) -> Effect DbError Unit</code> | Creates or updates tables to match their column definitions. |
| **db.runMigrationsOn** conn tables<br><code>DbConnection -> List (Table A) -> Effect DbError Unit</code> | Runs migrations on the given explicit connection. |
| **db.runMigrationSql** steps<br><code>List MigrationStep -> Effect DbError Unit</code> | Runs ordered SQL migration steps (id + sql) against the configured backend. |
| **db.runMigrationSqlOn** conn steps<br><code>DbConnection -> List MigrationStep -> Effect DbError Unit</code> | Runs ordered SQL migration steps on the given explicit connection. |
| **db.configureSqlite** tuning<br><code>SqliteTuning -> Effect DbError Unit</code> | Tunes SQLite `journal_mode` (WAL/DELETE) and busy-timeout for local-first workloads. |
| **db.configureSqliteOn** conn tuning<br><code>DbConnection -> SqliteTuning -> Effect DbError Unit</code> | Applies SQLite tuning to one explicit connection. |

### Data loading

| Function | Explanation |
| --- | --- |
| **db.load** table<br><code>Table A -> Effect DbError (List A)</code> | Loads all rows from `table` using the default configured connection. |
| **db.loadOn** conn table<br><code>DbConnection -> Table A -> Effect DbError (List A)</code> | Loads rows through an explicit connection handle. |
| **db.applyDelta** table delta<br><code>Table A -> Delta A -> Effect DbError (Table A)</code> | Applies an insert, update, delete, or upsert delta. Also available as the domain `+` operator. |
| **db.applyDeltaOn** conn table delta<br><code>DbConnection -> Table A -> Delta A -> Effect DbError (Table A)</code> | Applies a delta through an explicit connection. |
| **db.applyDeltas** table deltas<br><code>Table A -> List (Delta A) -> Effect DbError (Table A)</code> | Applies many deltas in one effect for projection-heavy write workloads. |
| **db.applyDeltasOn** conn table deltas<br><code>DbConnection -> Table A -> List (Delta A) -> Effect DbError (Table A)</code> | Applies many deltas through an explicit connection. |

### Transactions and savepoints

| Function | Explanation |
| --- | --- |
| **db.beginTxOn** conn<br><code>DbConnection -> Effect DbError Unit</code> | Starts a transaction on `conn`. |
| **db.commitTxOn** conn<br><code>DbConnection -> Effect DbError Unit</code> | Commits the transaction currently active on `conn`. |
| **db.rollbackTxOn** conn<br><code>DbConnection -> Effect DbError Unit</code> | Rolls back the transaction currently active on `conn`. |
| **db.inTransactionOn** conn action<br><code>DbConnection -> Effect DbError A -> Effect DbError A</code> | Runs `action` in a transaction, committing on success and rolling back on failure. |
| **db.savepointOn** conn name<br><code>DbConnection -> Text -> Effect DbError Unit</code> | Creates a savepoint with SQL-safe identifier validation. |
| **db.releaseSavepointOn** conn name<br><code>DbConnection -> Text -> Effect DbError Unit</code> | Releases a savepoint on `conn`. |
| **db.rollbackToSavepointOn** conn name<br><code>DbConnection -> Text -> Effect DbError Unit</code> | Rolls back to a savepoint while keeping the outer transaction active. |

The ambient forms (`db.beginTx`, `db.commitTx`, `db.rollbackTx`, `db.inTransaction`, `db.savepoint`, ...)
operate on the current default connection configured with `db.configure`. They exist for convenience and
backward compatibility, but explicit `...On` forms are the canonical transaction API.

### Delta constructors

| Constructor | Explanation |
| --- | --- |
| **Insert** row<br><code>A -> Delta A</code> | Inserts a new row. |
| **Update** pred patch<br><code>Pred A -> Patch A -> Delta A</code> | Updates rows matching `pred` with `patch`. |
| **Delete** pred<br><code>Pred A -> Delta A</code> | Deletes rows matching `pred`. |
| **Upsert** pred value patch<br><code>Pred A -> A -> Patch A -> Delta A</code> | Patches matching rows; inserts `value` when no row matches `pred`. |

### Convenience aliases

`aivi.database` also exports constructor aliases and domain sugar:

- `ins = Insert`
- `upd = Update`
- `del = Delete`
- `ups = Upsert`

When `use aivi.database (domain Database)` is in scope, these are available in delta expressions such as `table + upd (...) (...)` and `table + ups (...) value (...)`.

### Typed mutation helpers

<!-- quick-info: {"kind":"feature","name":"Typed mutation helpers"} -->
The typed mutation helpers are **convenience wrappers over the delta machinery**.
Each one constructs the appropriate `Delta A` and calls `db.applyDeltaOn` (or
`db.applyDelta` for the ambient form) in a single step, so you do not need to
name the intermediate delta value when the operation is straightforward.

**v0.1 behaviour:** Like `db.applyDelta` / `db.applyDeltaOn`, these helpers
execute entirely **in memory** — predicates and patches run in the AIVI runtime;
they do **not** compile to SQL `INSERT`, `DELETE`, `UPDATE`, or `INSERT … ON
CONFLICT` statements in v0.1.  SQL pushdown for mutations is planned for a later
phase.
<!-- /quick-info -->

<<< ../../snippets/from_md/stdlib/system/database/typed_mutations.aivi{aivi}

#### Explicit (`…On`) forms

| Function | Equivalent delta expression |
| --- | --- |
| **db.insertOn** conn table row<br><code>DbConnection -> Table A -> A -> Effect DbError (Table A)</code> | `db.applyDeltaOn conn table (Insert row)` |
| **db.deleteWhereOn** conn table pred<br><code>DbConnection -> Table A -> (A -> Bool) -> Effect DbError (Table A)</code> | `db.applyDeltaOn conn table (Delete pred)` |
| **db.updateWhereOn** conn table pred patch<br><code>DbConnection -> Table A -> (A -> Bool) -> (A -> A) -> Effect DbError (Table A)</code> | `db.applyDeltaOn conn table (Update pred patch)` |
| **db.upsertOn** conn table pred value patch<br><code>DbConnection -> Table A -> (A -> Bool) -> A -> (A -> A) -> Effect DbError (Table A)</code> | `db.applyDeltaOn conn table (Upsert pred value patch)` |

#### Ambient forms

The ambient forms omit the `conn` argument and operate on the default connection
configured with `db.configure`.  They exist for convenience and mirror the
explicit forms exactly.

| Function | Equivalent delta expression |
| --- | --- |
| **db.insert** table row<br><code>Table A -> A -> Effect DbError (Table A)</code> | `db.applyDelta table (Insert row)` |
| **db.deleteWhere** table pred<br><code>Table A -> (A -> Bool) -> Effect DbError (Table A)</code> | `db.applyDelta table (Delete pred)` |
| **db.updateWhere** table pred patch<br><code>Table A -> (A -> Bool) -> (A -> A) -> Effect DbError (Table A)</code> | `db.applyDelta table (Update pred patch)` |
| **db.upsert** table pred value patch<br><code>Table A -> (A -> Bool) -> A -> (A -> A) -> Effect DbError (Table A)</code> | `db.applyDelta table (Upsert pred value patch)` |

> **When to prefer the typed helpers vs. raw deltas:** Use the helpers for
> one-shot mutations where you do not need to compose or batch multiple deltas.
> Use `db.applyDeltas` / `db.applyDeltasOn` when you need to apply several
> mutations in a single call, or when you are building delta values programmatically.

### FTS helpers

`aivi.database` now includes typed helpers for preparing FTS payloads and queries:

- `ftsDoc : Text -> List Text -> FtsDoc`
- `ftsMatchAny : List Text -> FtsQuery`
- `ftsMatchAll : List Text -> FtsQuery`

### Query DSL (v0.1 MVP)

`Query A` produces `List A` when run against a `DbConnection`.  `do Query` still desugars
via `queryChain`/`queryOf`, but the compiler recognizes the portable subset and attaches a
structured SQL-backed plan alongside the ordinary runtime representation.

| Function | Explanation |
| --- | --- |
| **db.from** tbl<br><code>Table A -> Query A</code> | Lifts a table into a query source. With explicit columns, the lowered subset plans SQL directly against the mirrored table. |
| **db.where\_** pred q<br><code>(A -> Bool) -> Query A -> Query A</code> | Filters rows by a predicate. Lowered queries push the predicate into SQL; unsupported shapes keep the legacy in-memory behavior. |
| **db.guard\_** cond<br><code>Bool -> Query Unit</code> | In a `do Query` block: passes through when `cond` is `True`, otherwise short-circuits to empty. |
| **db.select** f q<br><code>(A -> B) -> Query A -> Query B</code> | Projects each row. Lowered queries support scalar projections, whole-row projections, and record projections built from those pieces. |
| **db.queryOf** value<br><code>A -> Query A</code> | Wraps a single value in a singleton query. |
| **db.emptyQuery**<br><code>Query A</code> | A query that always returns an empty list. |
| **db.queryChain** f q<br><code>(A -> Query B) -> Query A -> Query B</code> | Monadic bind for `Query`; used by `do Query` desugaring. |
| **db.runQueryOn** conn q<br><code>DbConnection -> Query A -> Effect DbError (List A)</code> | Executes the query against the given connection. Lowered queries execute as SQL; other queries fall back to the older runtime path. |
| **db.runQuery** q<br><code>Query A -> Effect DbError (List A)</code> | Executes the query against the **default connection** configured with `db.configure`, using the same lowered-vs-legacy behavior as `runQueryOn`. Prefer `runQueryOn` when you hold an explicit handle or a pool-acquired connection. |
| **db.orderBy** key q<br><code>(A -> B) -> Query A -> Query A</code> | Sorts rows by the given key function. Lowered queries emit SQL `ORDER BY`; other queries keep the older in-memory sort. |
| **db.limit** n q<br><code>Int -> Query A -> Query A</code> | Keeps at most `n` rows. Lowered queries emit SQL `LIMIT`; other queries keep the older in-memory slice. |
| **db.offset** n q<br><code>Int -> Query A -> Query A</code> | Skips the first `n` rows. Lowered queries emit SQL `OFFSET`; other queries keep the older in-memory slice. |
| **db.count** q<br><code>Query A -> Query Int</code> | Returns a singleton `Query Int` with the number of rows produced by `q`. Lowered queries emit SQL `COUNT(*)`; other queries keep the older in-memory count. |
| **db.exists** q<br><code>Query A -> Query Bool</code> | Returns a singleton `Query Bool` that is `True` when `q` produces at least one row. Lowered queries emit a SQL existence probe; other queries keep the older in-memory check. |

### Pooling (`aivi.database.pool`)

| Function | Explanation |
| --- | --- |
| **Pool.create** config<br><code>Pool.Config Conn -> Effect E (Result Pool.PoolError (Pool Conn))</code> | Creates a connection pool from the given configuration. |
| **Pool.withConn** pool f<br><code>Pool Conn -> (Conn -> Effect E A) -> Effect E (Result Pool.PoolError A)</code> | Acquires a connection, runs `f`, and guarantees release even on failure. |
