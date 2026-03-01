# Database Domain

<!-- quick-info: {"kind":"module","name":"aivi.database"} -->
The `Database` domain provides a type-safe, composable way to work with relational data. It treats tables as immutable records of schema plus rows, while compiling predicates and patches into efficient SQL under the hood.

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

## Types

<<< ../../snippets/from_md/stdlib/system/database/types.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/system/database/domain_definition.aivi{aivi}

### Applying Deltas

<<< ../../snippets/from_md/stdlib/system/database/applying_deltas.aivi{aivi}

## Querying

In v0.1, `Table A` is a persistent in-memory structure with explicit observation via `db.load`.

Query planning utilities (`filter`, `find`, `sortBy`, `groupBy`, `join`) are specified, but runtime coverage is partial in v0.1.

<<< ../../snippets/from_md/stdlib/system/database/querying.aivi{aivi}

## Joins and Preloading

<<< ../../snippets/from_md/stdlib/system/database/joins_and_preloading_01.aivi{aivi}

For eager loading:

<<< ../../snippets/from_md/stdlib/system/database/joins_and_preloading_02.aivi{aivi}

## Migrations

Schema definitions are typed values. Mark them `@static` to allow compile-time validation and migration planning.

<<< ../../snippets/from_md/stdlib/system/database/migrations.aivi{aivi}

## Pooling

Database pooling is provided by `aivi.database.pool`. The pool is configured explicitly (no hidden globals),
and `withConn` guarantees deterministic release via AIVI resources even on failure or cancellation.

<<< ../../snippets/from_md/stdlib/system/database/pooling.aivi{aivi}

## Notes

- `Database` compiles predicate expressions into `WHERE` clauses and patch instructions into `SET` clauses.
- Joins are translated into single SQL queries to avoid N+1 patterns.
- Advanced SQL remains available via `db.query` in [External Sources](../../syntax/external_sources.md).

## Core API (v0.1)

### Table management

| Function | Explanation |
| --- | --- |
| **db.table** name columns<br><code>Text -> List Column -> Table A</code> | Creates a table definition. The row type `A` is inferred from the binding's type annotation. |
| **db.configure** config<br><code>DbConfig -> Effect DbError Unit</code> | Selects the runtime backend (Sqlite, Postgresql, Mysql). |
| **db.runMigrations** tables<br><code>List (Table A) -> Effect DbError Unit</code> | Creates or updates tables to match their column definitions. |
| **db.runMigrationSql** steps<br><code>List MigrationStep -> Effect DbError Unit</code> | Runs ordered SQL migration steps (id + sql) against the configured backend. |
| **db.configureSqlite** tuning<br><code>SqliteTuning -> Effect DbError Unit</code> | Tunes SQLite `journal_mode` (WAL/DELETE) and busy-timeout for local-first workloads. |

### Data loading

| Function | Explanation |
| --- | --- |
| **db.load** table<br><code>Table A -> Effect (SourceError Db) (List A)</code> | Loads all rows from `table`. Validates fields against type `A`. |
| **db.applyDelta** table delta<br><code>Table A -> Delta A -> Effect DbError (Table A)</code> | Applies an insert, update, delete, or upsert delta. Also available as the domain `+` operator. |
| **db.applyDeltas** table deltas<br><code>Table A -> List (Delta A) -> Effect DbError (Table A)</code> | Applies many deltas in one effect for projection-heavy write workloads. |

### Transactions and savepoints

| Function | Explanation |
| --- | --- |
| **db.beginTx**<br><code>Effect DbError Unit</code> | Starts a transaction. |
| **db.commitTx**<br><code>Effect DbError Unit</code> | Commits the current transaction. |
| **db.rollbackTx**<br><code>Effect DbError Unit</code> | Rolls back the current transaction. |
| **db.savepoint** name<br><code>Text -> Effect DbError Unit</code> | Creates a savepoint with SQL-safe identifier validation. |
| **db.releaseSavepoint** name<br><code>Text -> Effect DbError Unit</code> | Releases a savepoint. |
| **db.rollbackToSavepoint** name<br><code>Text -> Effect DbError Unit</code> | Rolls back to a savepoint while keeping outer transaction active. |

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

### Pooling (`aivi.database.pool`)

| Function | Explanation |
| --- | --- |
| **Pool.create** config<br><code>Pool.Config Conn -> Effect E (Result Pool.PoolError (Pool Conn))</code> | Creates a connection pool from the given configuration. |
| **Pool.withConn** pool f<br><code>Pool Conn -> (Conn -> Effect E A) -> Effect E (Result Pool.PoolError A)</code> | Acquires a connection, runs `f`, and guarantees release even on failure. |
