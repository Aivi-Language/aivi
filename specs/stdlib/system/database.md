# Database Domain

<!-- quick-info: {"kind":"module","name":"aivi.database"} -->
The `Database` domain gives you a typed way to describe tables, move data in and out of relational databases, and build queries without dropping straight to raw SQL.

It combines familiar database ideas—tables, rows, filters, joins, migrations, and transactions—with AIVI's typed records, effects, and resource management.

<!-- /quick-info -->
<div class="import-badge">use aivi.database<span class="domain-badge">domain</span></div>

## Start here

This page is intentionally broad. If you are new to `aivi.database`, do not read it as one long reference manual. Start with the beginner path, copy the first useful workflow, then come back for the advanced sections and API tables when you need them.

### Beginner path

For a first database-backed feature, this is the shortest useful route:

1. define one typed `Table A` (see [Overview](#overview))
2. open or configure a connection
3. run migrations (see [Migrations](#migrations))
4. load rows or run one simple query (see [Querying](#querying))
5. apply inserts, updates, or deletes

You can safely ignore pooling, multi-table joins, savepoints, and typed mutation helpers until that flow feels familiar.

### Terms you'll see later

If this is your first pass, skim these and keep moving. The query and migration examples below will make them concrete.

| Term | Plain meaning |
| --- | --- |
| **explicit connection** | you open a `DbConnection` value yourself and pass it to the helpers that need it |
| **default / ambient connection helper** | a helper such as `db.load` that uses the process-wide connection previously configured with `db.configure` |
| **delta** | a value that describes a write such as “insert this row” or “update rows matching this predicate” |
| **savepoint** | a named rollback marker inside a larger transaction |
| **portable subset** | query shapes that cleanly translate to SQL instead of falling back to older in-memory behavior |

### First successful workflow

If you want one concrete pattern to copy, start with an explicit connection and a single query:

```aivi
main = do Effect {
  dbConn <- db.connect { driver: Sqlite, url: "app.db" }
  _      <- db.runMigrationsOn dbConn [userTable]

  activeUsersQuery =
    db.from userTable
    |> db.where_ _.active

  activeUsers <- db.runQueryOn dbConn activeUsersQuery
  _           <- db.close dbConn
  pure activeUsers
}
```

That gives you a complete first loop: connect, migrate, query, clean up.

### Quick choice: `db.load` vs `db.runQuery`

- use `db.load` when you want every row from one table with no extra filtering,
- use `db.runQuery` when you want filtering, sorting, projections, joins, or aggregates.

### Advanced path

Come back to the later sections when you need:

- explicit connection ownership for larger programs
- portable query lowering and multi-table joins
- transactions and savepoints
- connection pooling
- typed mutation helpers and reusable deltas

## Overview

<<< ../../snippets/from_md/stdlib/system/database/overview.aivi{aivi}

A table schema is described with ordinary values. `db.table` takes a table name and a list of `Column` values, while the row shape comes from the binding's type annotation.

`aivi.database` supports two connection styles:

- **explicit connections** such as `db.connect`, `db.loadOn`, and `db.beginTxOn`,
- **default connection helpers** such as `db.configure`, `db.load`, and `db.beginTx`.

Explicit `DbConnection` handles are usually the better fit for larger programs because ownership stays local and works cleanly with pooling and transactions.

### Choosing a connection style

| Style | Best for | Trade-off | Typical first example |
| --- | --- | --- | --- |
| default connection helpers (`db.configure`, `db.load`, `db.beginTx`) | tutorials, small tools, one-database apps | convenient, but the active connection is ambient | “configure once, then `db.load userTable`” |
| explicit connections (`db.connect`, `db.loadOn`, `db.beginTxOn`) | services, pooled code, transaction-heavy workflows | a little more wiring, but ownership stays obvious | “open `dbConn`, pass it to `db.runQueryOn`, then close it” |

## Types

If you are still on the beginner path, you can skim this section once and return later. The first overview, migration, query, and API-table examples are the faster route to a working mental model.

<<< ../../snippets/from_md/stdlib/system/database/types.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/system/database/domain_definition.aivi{aivi}

### Applying deltas

Deltas describe inserts, updates, deletes, and upserts as data.
That makes it possible to build mutations explicitly, combine them, and apply them through one API.

<<< ../../snippets/from_md/stdlib/system/database/applying_deltas.aivi{aivi}

## A practical mental model

If you are approaching this domain from a traditional application background, a good workflow is:

1. define a typed `Table A`,
2. open or configure a connection,
3. run migrations,
4. load rows or run a `Query`,
5. make changes with deltas or typed mutation helpers,
6. wrap related writes in a transaction when they must succeed or fail together.

## Querying

<!-- quick-info: {"kind":"feature","name":"Query DSL"} -->
`Query A` is a typed description of a database read that eventually produces values of type `A`.
The `do Query { ... }` notation lets you write those reads in a step-by-step style that feels close to a SQL `SELECT` while staying inside ordinary AIVI code.

Queries are translated to a structured SQL-backed form when every participating table has an explicit column list and the query stays within the portable subset. Here, **portable subset** means “query shapes that cleanly translate to SQL instead of relying on the older in-memory runtime.” That subset includes `db.from`, `db.where_`, `db.guard_`, `db.select`, `db.orderBy`, `db.limit`, `db.offset`, `db.count`, `db.exists`, and `do Query` blocks built from those forms.

Helper-built queries that fall outside that subset still run through the older in-memory query runtime. Unsupported `do Query` shapes fail with a query error when executed instead of silently changing behavior.

If you are learning the query DSL, read the examples in this order:

1. one-table query,
2. ambient-vs-explicit execution,
3. pipeline helpers,
4. joins,
5. aggregates.

Start here with the one-table example. The later examples reuse the same ideas and add one new concept at a time.

```aivi
-- Build a query value once
activeUserNamesQuery : Query Text
activeUserNamesQuery = do Query {
  user <- db.from userTable
  db.guard_ user.active          -- keep only active users
  db.queryOf user.name           -- project one field
}

-- Run it with an explicit connection
main = do Effect {
  dbConn <- db.connect { driver: Sqlite, url: "app.db" }
  names  <- db.runQueryOn dbConn activeUserNamesQuery
  _      <- db.close dbConn
  pure names
}
```

If you have already configured a default connection, you can use the ambient helpers instead:

```aivi
main = do Effect {
  _     <- db.configure { driver: Sqlite, url: "app.db" }
  _     <- db.runMigrations [userTable]
  names <- db.runQuery activeUserNamesQuery
  pure names
}
```

You can also build the same query with pipeline helpers:

```aivi
activeUserNamesQuery : Query Text
activeUserNamesQuery =
  db.from userTable
  |> db.where_ _.active
  |> db.select _.name
```

Sorting and paging work well in the pipeline style because they read from top to bottom:

```aivi
recentTopNames : Query Text
recentTopNames =
  db.from userTable
  |> db.where_ _.active
  |> db.orderBy _.createdAt   -- sort before slicing
  |> db.offset 10             -- skip the first page
  |> db.limit 5               -- then keep the next five rows
  |> db.select _.name
```

Inside a `do Query` block, apply those helpers to the source query on the right-hand side of `<-`.
<!-- /quick-info -->

### Multi-table joins

<!-- quick-info: {"kind":"feature","name":"Multi-table join"} -->
Multi-table reads are written as repeated `db.from` binds plus `db.guard_` conditions that relate the rows.
In the portable subset, that pattern lowers to a SQL cross join with pushed-down `WHERE` predicates. In practice, when the guard compares keys from the participating rows, that behaves like the inner joins most SQL users expect.

```aivi
Order = { id: Int, userId: Int, total: Int }

orderTable : Table Order
orderTable = db.table "orders" [
  { name: "id",     type: IntType, constraints: [AutoIncrement, NotNull], default: None }
  { name: "userId", type: IntType, constraints: [NotNull],                default: None }
  { name: "total",  type: IntType, constraints: [NotNull],                default: None }
]

activeUserOrders : Query { user: User, order: Order }
activeUserOrders = do Query {
  user  <- db.from userTable
  db.guard_ user.active
  order <- db.from orderTable
  db.guard_ (order.userId == user.id)   -- join condition
  db.queryOf { user: user, order: order }
}
```

This style currently covers inner-join-like workflows built from table sources and guard conditions.
<!-- /quick-info -->

### Aggregate helpers

<!-- quick-info: {"kind":"feature","name":"db.count / db.exists"} -->
`db.count` and `db.exists` are the simplest way to ask summary questions about a query.
When their input query stays inside the lowered subset, they compile to SQL aggregate or existence checks. Otherwise, they use the older in-memory behavior.

```aivi
activeUserCountQuery : Query Int
activeUserCountQuery = db.count (db.from userTable |> db.where_ _.active)

hasActiveUsers : Query Bool
hasActiveUsers = db.exists (db.from userTable |> db.where_ _.active)

main = do Effect {
  dbConn       <- db.connect { driver: Sqlite, url: "app.db" }
  [userCount]  <- db.runQueryOn dbConn activeUserCountQuery
  [foundUsers] <- db.runQueryOn dbConn hasActiveUsers
  _            <- db.close dbConn
  pure (userCount, foundUsers)
}
```

These helpers do not make an otherwise unsupported query lowerable; they only follow the behavior of the query they wrap.
<!-- /quick-info -->

<<< ../../snippets/from_md/stdlib/system/database/querying.aivi{aivi}

## Joins and preloading

<<< ../../snippets/from_md/stdlib/system/database/joins_and_preloading_01.aivi{aivi}

For eager loading, use the matching preload helpers and patterns shown here:

<<< ../../snippets/from_md/stdlib/system/database/joins_and_preloading_02.aivi{aivi}

## Migrations

Schema definitions are typed values, so the same information can drive both runtime behavior and migration generation.
Marking them `@static` allows compile-time validation and migration analysis when the inputs are known ahead of time.

<<< ../../snippets/from_md/stdlib/system/database/migrations.aivi{aivi}

## Schema-first source declarations

Database-backed source declarations can reuse table and migration values as schema carriers instead of hiding row shape behind the eventual `db.load` or query call.
This gives tooling enough information to validate columns and projected row shapes earlier.

A database source declaration should carry typed connector config, the table or query/projection to read from, and any typed parameters it needs.

## Pooling

Connection pooling lives in `aivi.database.pool`.
The pool is configured explicitly, and `withConn` guarantees a checked-out connection is released even if the work fails or is cancelled.

If you are still on the beginner path, skip pooling until one process needs many short-lived database operations.

<<< ../../snippets/from_md/stdlib/system/database/pooling.aivi{aivi}

## Capabilities

- `db.configure`, `db.configureSqlite`, pool creation, and connection acquisition require database connection capability.
- `db.load` and read-only query helpers require database query capability.
- `db.applyDelta`, typed mutation helpers, transactions, and savepoints require database mutation capability.
- `db.runMigrations` and `db.runMigrationSql` require database migration capability.

## Core API

This section is the reference shelf. Skim it once, then come back when you need the exact helper name or type.

### Table and connection management

| Function | What it does |
| --- | --- |
| **db.table** name columns<br><code>Text -> List Column -> Table A</code> | Creates a table definition. The row type `A` comes from the binding's type annotation. |
| **db.configure** config<br><code>DbConfig -> Effect DbError Unit</code> | Configures the default database connection used by ambient helpers. |
| **db.connect** config<br><code>DbConfig -> Effect DbError DbConnection</code> | Opens an explicit connection handle. This is the preferred entry point for pooled or transaction-heavy code. |
| **db.open** config<br><code>DbConfig -> Resource DbError DbConnection</code> | Resource wrapper around `db.connect` and `db.close` for deterministic cleanup. |
| **db.close** conn<br><code>DbConnection -> Effect DbError Unit</code> | Closes an explicit connection handle. |
| **db.runMigrations** tables<br><code>List (Table A) -> Effect DbError Unit</code> | Applies schema changes for the configured default connection. |
| **db.runMigrationsOn** conn tables<br><code>DbConnection -> List (Table A) -> Effect DbError Unit</code> | Applies schema changes through an explicit connection. |
| **db.runMigrationSql** steps<br><code>List MigrationStep -> Effect DbError Unit</code> | Runs ordered SQL migration steps against the configured backend. |
| **db.runMigrationSqlOn** conn steps<br><code>DbConnection -> List MigrationStep -> Effect DbError Unit</code> | Runs ordered SQL migration steps through an explicit connection. |
| **db.configureSqlite** tuning<br><code>SqliteTuning -> Effect DbError Unit</code> | Sets SQLite-specific tuning such as journal mode and busy timeout on the default connection. |
| **db.configureSqliteOn** conn tuning<br><code>DbConnection -> SqliteTuning -> Effect DbError Unit</code> | Applies SQLite-specific tuning to one explicit connection. |

### Loading data and applying deltas

| Function | What it does |
| --- | --- |
| **db.load** table<br><code>Table A -> Effect DbError (List A)</code> | Loads all rows from `table` using the default configured connection. |
| **db.loadOn** conn table<br><code>DbConnection -> Table A -> Effect DbError (List A)</code> | Loads all rows from `table` through an explicit connection. |
| **db.applyDelta** table delta<br><code>Table A -> Delta A -> Effect DbError (Table A)</code> | Applies one delta against `table` using the default connection. Also available through the domain `+` operator. |
| **db.applyDeltaOn** conn table delta<br><code>DbConnection -> Table A -> Delta A -> Effect DbError (Table A)</code> | Applies one delta through an explicit connection. |
| **db.applyDeltas** table deltas<br><code>Table A -> List (Delta A) -> Effect DbError (Table A)</code> | Applies several deltas in one call using the default connection. |
| **db.applyDeltasOn** conn table deltas<br><code>DbConnection -> Table A -> List (Delta A) -> Effect DbError (Table A)</code> | Applies several deltas through an explicit connection. |

### Transactions and savepoints

| Function | What it does |
| --- | --- |
| **db.beginTxOn** conn<br><code>DbConnection -> Effect DbError Unit</code> | Starts a transaction on `conn`. |
| **db.commitTxOn** conn<br><code>DbConnection -> Effect DbError Unit</code> | Commits the active transaction on `conn`. |
| **db.rollbackTxOn** conn<br><code>DbConnection -> Effect DbError Unit</code> | Rolls back the active transaction on `conn`. |
| **db.inTransactionOn** conn action<br><code>DbConnection -> Effect DbError A -> Effect DbError A</code> | Runs `action` in a transaction, committing on success and rolling back on failure. |
| **db.savepointOn** conn name<br><code>DbConnection -> Text -> Effect DbError Unit</code> | Creates a named savepoint after validating the identifier. |
| **db.releaseSavepointOn** conn name<br><code>DbConnection -> Text -> Effect DbError Unit</code> | Releases a savepoint on `conn`. |
| **db.rollbackToSavepointOn** conn name<br><code>DbConnection -> Text -> Effect DbError Unit</code> | Rolls back to a savepoint while keeping the outer transaction active. |

The ambient forms (`db.beginTx`, `db.commitTx`, `db.rollbackTx`, `db.inTransaction`, `db.savepoint`, and related helpers) operate on the current default connection configured with `db.configure`.

### Delta constructors

| Constructor | What it represents |
| --- | --- |
| **Insert** row<br><code>A -> Delta A</code> | Insert one new row. |
| **Update** pred patch<br><code>Pred A -> Patch A -> Delta A</code> | Update rows that match `pred` with `patch`. |
| **Delete** pred<br><code>Pred A -> Delta A</code> | Delete rows that match `pred`. |
| **Upsert** pred value patch<br><code>Pred A -> A -> Patch A -> Delta A</code> | Update matching rows, or insert `value` if no row matches. |

### Convenience aliases

`aivi.database` also exports short aliases for the delta constructors:

- `ins = Insert`
- `upd = Update`
- `del = Delete`
- `ups = Upsert`

When `use aivi.database (domain Database)` is in scope, these aliases work well in expressions such as `table + upd (...) (...)`.

### Typed mutation helpers

<!-- quick-info: {"kind":"feature","name":"Typed mutation helpers"} -->
The typed mutation helpers are convenience wrappers over delta construction.
They build the appropriate `Delta A` for you and then call `db.applyDeltaOn` or `db.applyDelta`, which is handy when the operation is straightforward and you do not need to name the intermediate delta value.

These helpers currently execute in memory, just like `db.applyDelta` and `db.applyDeltaOn`. Their predicates and patches do not compile to SQL mutation statements.
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

| Function | Equivalent delta expression |
| --- | --- |
| **db.insert** table row<br><code>Table A -> A -> Effect DbError (Table A)</code> | `db.applyDelta table (Insert row)` |
| **db.deleteWhere** table pred<br><code>Table A -> (A -> Bool) -> Effect DbError (Table A)</code> | `db.applyDelta table (Delete pred)` |
| **db.updateWhere** table pred patch<br><code>Table A -> (A -> Bool) -> (A -> A) -> Effect DbError (Table A)</code> | `db.applyDelta table (Update pred patch)` |
| **db.upsert** table pred value patch<br><code>Table A -> (A -> Bool) -> A -> (A -> A) -> Effect DbError (Table A)</code> | `db.applyDelta table (Upsert pred value patch)` |

Use the typed helpers for one-off mutations. Use `db.applyDeltas` or `db.applyDeltasOn` when you want to batch several deltas together.

### FTS helpers

`aivi.database` also includes typed helpers for full-text-search payloads and queries:

- `ftsDoc : Text -> List Text -> FtsDoc`
- `ftsMatchAny : List Text -> FtsQuery`
- `ftsMatchAll : List Text -> FtsQuery`

### Query DSL reference

| Function | What it does |
| --- | --- |
| **db.from** tbl<br><code>Table A -> Query A</code> | Lifts a table into a query source. |
| **db.where\_** pred q<br><code>(A -> Bool) -> Query A -> Query A</code> | Filters rows by a predicate. In lowered queries the filter is pushed into SQL. |
| **db.guard\_** cond<br><code>Bool -> Query Unit</code> | Inside `do Query`, continues when `cond` is `True` and produces no rows when it is `False`. |
| **db.select** f q<br><code>(A -> B) -> Query A -> Query B</code> | Projects each row into a new shape. |
| **db.queryOf** value<br><code>A -> Query A</code> | Wraps one value as a singleton query result. |
| **db.emptyQuery**<br><code>Query A</code> | A query that always returns an empty list. |
| **db.queryChain** f q<br><code>(A -> Query B) -> Query A -> Query B</code> | Query bind; this is what `do Query` desugars to. |
| **db.runQueryOn** conn q<br><code>DbConnection -> Query A -> Effect DbError (List A)</code> | Executes a query through an explicit connection. |
| **db.runQuery** q<br><code>Query A -> Effect DbError (List A)</code> | Executes a query through the default configured connection. |
| **db.orderBy** key q<br><code>(A -> B) -> Query A -> Query A</code> | Sorts rows by a projected key. |
| **db.limit** n q<br><code>Int -> Query A -> Query A</code> | Keeps at most `n` rows. |
| **db.offset** n q<br><code>Int -> Query A -> Query A</code> | Skips the first `n` rows. |
| **db.count** q<br><code>Query A -> Query Int</code> | Returns a singleton query containing the row count. |
| **db.exists** q<br><code>Query A -> Query Bool</code> | Returns a singleton query telling you whether any rows match. |

### Pooling (`aivi.database.pool`)

| Function | What it does |
| --- | --- |
| **Pool.create** config<br><code>Pool.Config Conn -> Effect E (Result Pool.PoolError (Pool Conn))</code> | Creates a connection pool from the given configuration. |
| **Pool.withConn** pool f<br><code>Pool Conn -> (Conn -> Effect E A) -> Effect E (Result Pool.PoolError A)</code> | Borrows a connection, runs `f`, and always releases the connection afterward. |

## Practical guidance

- Prefer explicit connections when a function opens, shares, or nests database work.
- Use ambient helpers when you truly want one process-wide default connection.
- Keep table definitions complete with explicit column lists if you want the query DSL to lower cleanly into SQL.
- Use savepoints for inner rollback boundaries instead of trying to nest transactions on the same connection.
- Reach for raw deltas when you want to construct mutations programmatically; reach for typed helpers when you want the shortest clear code.

## Notes

- Raw SQL strings through an external-source `db.query` mechanism are not part of this module's documented surface; use `db.from` and the typed query helpers instead.
- `db.applyDelta`, `db.applyDeltas`, and the typed mutation helpers operate in memory rather than compiling predicates and patches into SQL mutation statements.
- Transactions are scoped to a single `DbConnection`.
