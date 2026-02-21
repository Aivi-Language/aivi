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

<<< ../../snippets/from_md/05_stdlib/03_system/23_database/block_01.aivi{aivi}

Table schemas are defined with ordinary values. `db.table` takes a table name and a
list of `Column` values; the row type comes from the table binding's type annotation.

## Types

<<< ../../snippets/from_md/05_stdlib/03_system/23_database/block_02.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/05_stdlib/03_system/23_database/block_03.aivi{aivi}

### Applying Deltas

<<< ../../snippets/from_md/05_stdlib/03_system/23_database/block_04.aivi{aivi}

## Querying

In v0.1, `Table A` is a persistent in-memory structure with explicit observation via `db.load`.

Query planning utilities (`filter`, `find`, `sortBy`, `groupBy`, `join`) are part of the design direction,
but are not yet guaranteed to be implemented end-to-end in the runtime.

<<< ../../snippets/from_md/05_stdlib/03_system/23_database/block_05.aivi{aivi}

## Joins and Preloading

<<< ../../snippets/from_md/05_stdlib/03_system/23_database/block_06.aivi{aivi}

For eager loading:

<<< ../../snippets/from_md/05_stdlib/03_system/23_database/block_07.aivi{aivi}

## Migrations

Schema definitions are typed values. Mark them `@static` to allow compile-time validation and migration planning.

<<< ../../snippets/from_md/05_stdlib/03_system/23_database/block_08.aivi{aivi}

## Pooling

Database pooling is provided by `aivi.database.pool`. The pool is configured explicitly (no hidden globals),
and `withConn` guarantees deterministic release via AIVI resources even on failure or cancellation.

<<< ../../snippets/from_md/05_stdlib/03_system/23_database/block_09.aivi{aivi}

## Notes

- `Database` compiles predicate expressions into `WHERE` clauses and patch instructions into `SET` clauses.
- Joins are translated into single SQL queries to avoid N+1 patterns.
- Advanced SQL remains available via `db.query` in [External Sources](../../02_syntax/12_external_sources.md).

## Core API (v0.1)

### Table management

| Function | Explanation |
| --- | --- |
| **db.table** name columns<br><pre><code>`Text -> List Column -> Table A`</code></pre> | Creates a table definition. The row type `A` is inferred from the binding's type annotation. |
| **db.configure** config<br><pre><code>`DbConfig -> Effect DbError Unit`</code></pre> | Selects the runtime backend (Sqlite, Postgresql, Mysql). |
| **db.runMigrations** tables<br><pre><code>`List (Table A) -> Effect DbError Unit`</code></pre> | Creates or updates tables to match their column definitions. |

### Data loading

| Function | Explanation |
| --- | --- |
| **db.load** table<br><pre><code>`Table A -> Effect (SourceError Db) (List A)`</code></pre> | Loads all rows from `table`. Validates fields against type `A`. |
| **db.applyDelta** table delta<br><pre><code>`Table A -> Delta A -> Effect DbError (Table A)`</code></pre> | Applies an insert, update, or delete delta. Also available as the domain `+` operator. |

### Delta constructors

| Constructor | Explanation |
| --- | --- |
| **Insert** row<br><pre><code>`A -> Delta A`</code></pre> | Inserts a new row. |
| **Update** pred patch<br><pre><code>`Pred A -> Patch A -> Delta A`</code></pre> | Updates rows matching `pred` with `patch`. |
| **Delete** pred<br><pre><code>`Pred A -> Delta A`</code></pre> | Deletes rows matching `pred`. |

### Pooling (`aivi.database.pool`)

| Function | Explanation |
| --- | --- |
| **Pool.create** config<br><pre><code>`Pool.Config Conn -> Effect E (Result Pool.PoolError (Pool Conn))`</code></pre> | Creates a connection pool from the given configuration. |
| **Pool.withConn** pool f<br><pre><code>`Pool Conn -> (Conn -> Effect E A) -> Effect E (Result Pool.PoolError A)`</code></pre> | Acquires a connection, runs `f`, and guarantees release even on failure. |
