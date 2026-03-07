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

**MVP limitations (v0.1):** queries are executed **in memory** — all rows are loaded
from the store first, then predicates and projections run in the AIVI runtime.  True
SQL pushdown (WHERE / SELECT compilation) is planned for a later phase.

```aivi
-- Compose a query value
activeNames : Query Text
activeNames = do Query {
  user <- db.from userTable    -- bind each row
  db.guard_ user.active        -- skip inactive rows
  db.queryOf user.name         -- project the name field
}

-- Execute it against a connection
main = do Effect {
  conn  <- db.connect { driver: Sqlite, url: "app.db" }
  names <- db.runQueryOn conn activeNames
  ...
}
```

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

- In v0.1, `Query A` executes **in memory**: `db.from tbl` loads all rows from the store, then predicates and projections run in the AIVI runtime.  SQL pushdown is not yet implemented.
- Advanced SQL strings remain available via the external-source `db.query` mechanism described in [External Sources](../../syntax/external_sources.md).
- `db.applyDelta` / `db.applyDeltas` do compile predicates to `WHERE` and patches to `SET` for the underlying store, but `do Query` predicates do not yet benefit from this.
- Transactions are scoped to a single `DbConnection`. `db.beginTxOn conn` never affects any other connection in the same pool.
- The ambient helpers (`db.beginTx`, `db.commitTx`, `db.rollbackTx`, `db.savepoint`, ...) are compatibility sugar over the current default connection selected by `db.configure`.
- Nested `beginTxOn` calls are not part of the transaction model; use savepoints for inner rollback boundaries.

## Capability mapping (Phase 1 surface)

- `db.configure`, `db.configureSqlite`, pool creation / acquisition → `db.connect`
- `db.load` and read-only query helpers → `db.query`
- `db.applyDelta`, `db.applyDeltas`, transactions, savepoints → `db.mutate`
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

### FTS helpers

`aivi.database` now includes typed helpers for preparing FTS payloads and queries:

- `ftsDoc : Text -> List Text -> FtsDoc`
- `ftsMatchAny : List Text -> FtsQuery`
- `ftsMatchAll : List Text -> FtsQuery`

### Query DSL (v0.1 MVP)

`Query A` is an in-memory query that produces `List A` when run against a `DbConnection`.
Use `do Query { ... }` to compose queries; `do Query` desugars via `queryChain`/`queryOf`.

| Function | Explanation |
| --- | --- |
| **db.from** tbl<br><code>Table A -> Query A</code> | Lifts a table into a query that loads all rows. |
| **db.where\_** pred q<br><code>(A -> Bool) -> Query A -> Query A</code> | Filters rows by a predicate (runs in memory). |
| **db.guard\_** cond<br><code>Bool -> Query Unit</code> | In a `do Query` block: passes through when `cond` is `True`, otherwise short-circuits to empty. |
| **db.select** f q<br><code>(A -> B) -> Query A -> Query B</code> | Projects each row. |
| **db.queryOf** value<br><code>A -> Query A</code> | Wraps a single value in a singleton query. |
| **db.emptyQuery**<br><code>Query A</code> | A query that always returns an empty list. |
| **db.queryChain** f q<br><code>(A -> Query B) -> Query A -> Query B</code> | Monadic bind for `Query`; used by `do Query` desugaring. |
| **db.runQueryOn** conn q<br><code>DbConnection -> Query A -> Effect DbError (List A)</code> | Executes the query against the given connection. |

### Pooling (`aivi.database.pool`)

| Function | Explanation |
| --- | --- |
| **Pool.create** config<br><code>Pool.Config Conn -> Effect E (Result Pool.PoolError (Pool Conn))</code> | Creates a connection pool from the given configuration. |
| **Pool.withConn** pool f<br><code>Pool Conn -> (Conn -> Effect E A) -> Effect E (Result Pool.PoolError A)</code> | Acquires a connection, runs `f`, and guarantees release even on failure. |
