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
| **portable subset** | query shapes that translate cleanly to SQL without backend-specific extensions, instead of falling back to older in-memory behavior |

### First successful workflow

If you want one concrete pattern to copy, start with an explicit connection and a single query:

<<< ../../snippets/from_md/stdlib/system/database/block_01.aivi{aivi}


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

### Ambient and explicit execution at a glance

The ambient/explicit split applies to reads, writes, and transaction control, not just connection acquisition.

| Concern | Ambient helper | Explicit helper |
| --- | --- | --- |
| load one table | `db.load userTable` | `db.loadOn conn userTable` |
| run a query | `db.runQuery q` | `db.runQueryOn conn q` |
| apply one delta | `db.applyDelta userTable delta` | `db.applyDeltaOn conn userTable delta` |
| apply several deltas | `db.applyDeltas userTable deltas` | `db.applyDeltasOn conn userTable deltas` |
| wrap work in a transaction | `db.inTransaction action` | `db.inTransactionOn conn action` |
| manage a savepoint | `db.savepoint "afterUsers"` | `db.savepointOn conn "afterUsers"` |

Ambient helpers always target the current default connection configured with `db.configure`.
Explicit helpers always target the `DbConnection` value passed in by the caller.
Any future selector-based CRUD surface should follow the same split instead of embedding connection ownership into the selector itself.

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

Queries are translated to a structured SQL-backed form when every participating table has an explicit column list and the query stays within the portable subset. Here, **portable subset** means “query shapes that cleanly translate to SQL instead of relying on the older in-memory runtime.” That subset includes `db.from`, `db.where`, `db.guard`, `db.select`, `db.orderBy`, `db.limit`, `db.offset`, `db.count`, `db.exists`, and `do Query` blocks built from those forms.

Helper-built queries that fall outside that subset still run through the older in-memory query runtime. Unsupported `do Query` shapes fail with a query error when executed instead of silently changing behavior.

If you are learning the query DSL, read the examples in this order:

1. one-table query,
2. ambient-vs-explicit execution,
3. pipeline helpers,
4. joins,
5. aggregates.

Start here with the one-table example. The later examples reuse the same ideas and add one new concept at a time.

<<< ../../snippets/from_md/stdlib/system/database/block_02.aivi{aivi}


If you have already configured a default connection, you can use the ambient helpers instead:

<<< ../../snippets/from_md/stdlib/system/database/block_03.aivi{aivi}


You can also build the same query with pipeline helpers:

<<< ../../snippets/from_md/stdlib/system/database/block_04.aivi{aivi}


Sorting and paging work well in the pipeline style because they read from top to bottom:

<<< ../../snippets/from_md/stdlib/system/database/block_05.aivi{aivi}


When a lowered query omits `db.orderBy`, rows keep the source row order so `db.limit` and `db.offset` stay deterministic.


Inside a `do Query` block, apply those helpers to the source query on the right-hand side of `<-`.
<!-- /quick-info -->

### Multi-table joins

<!-- quick-info: {"kind":"feature","name":"Multi-table join"} -->
Multi-table reads are written as repeated `db.from` binds plus `db.guard` conditions that relate the rows.
In the portable subset, that pattern lowers to a SQL cross join with pushed-down `WHERE` predicates. In practice, when the guard compares keys from the participating rows, that behaves like the inner joins most SQL users expect.

<<< ../../snippets/from_md/stdlib/system/database/block_06.aivi{aivi}


This style currently covers inner-join-like workflows built from table sources and guard conditions.
<!-- /quick-info -->

### Aggregate helpers

<!-- quick-info: {"kind":"feature","name":"db.count / db.exists"} -->
`db.count` and `db.exists` are the simplest way to ask summary questions about a query.
When their input query stays inside the lowered subset, they compile to SQL aggregate or existence checks. Otherwise, they use the older in-memory behavior.

<<< ../../snippets/from_md/stdlib/system/database/block_07.aivi{aivi}


These helpers do not make an otherwise unsupported query lowerable; they only follow the behavior of the query they wrap.
<!-- /quick-info -->

<<< ../../snippets/from_md/stdlib/system/database/querying.aivi{aivi}

## Shaping multi-table results

Use repeated `db.from` binds plus `db.guard` when you want one flattened row per match; the [Multi-table joins](#multi-table-joins) example above is the canonical portable form.

When you need a nested parent/child result such as `{ user, posts }`, keep the database read typed and do the final shaping in ordinary AIVI code after one or more explicit `db.runQueryOn` or `db.loadOn` calls. That keeps this page aligned with the currently documented `Query` and table-loading helpers without implying extra preload-specific APIs.

## Migrations

Schema definitions are typed values, so the same information can drive both runtime behavior and migration generation.
Marking them `@static` allows compile-time validation and migration analysis when the inputs are known ahead of time.

<<< ../../snippets/from_md/stdlib/system/database/migrations.aivi{aivi}

## Schema-first source declarations

Database-backed source declarations can reuse table and migration values as schema carriers instead of hiding row shape behind the eventual `db.load` or query call.
This gives tooling enough information to validate columns and projected row shapes earlier.

A database source declaration should carry typed connector config, the table or query/projection to read from, and any typed parameters it needs.

For the `Source Db` forms themselves, see [External Sources](../../syntax/external_sources.md) and [Schema-First Source Definitions](../../syntax/external_sources/schema_first.md). A minimal table-backed declaration looks like this:

<<< ../../snippets/from_md/stdlib/system/database/block_01.aivi{aivi}


`load usersRows` is still the effectful step; the declaration just makes the database boundary reusable and statically inspectable.

## Pooling

Connection pooling lives in `aivi.database.pool`.
The pool is configured explicitly, and `withConn` guarantees a checked-out connection is released even if the work fails or is cancelled.
The underlying pool API is generic; the signatures below show the usual database instantiation with `DbConnection` and `DbError`.

If you are still on the beginner path, skip pooling until one process needs many short-lived database operations.

<<< ../../snippets/from_md/stdlib/system/database/pooling.aivi{aivi}

## Runtime effects

- `db.configure`, `db.configureSqlite`, pool creation, and connection acquisition open or configure database connections.
- `db.load` and read-only query helpers execute database reads.
- `db.applyDelta`, typed mutation helpers, transactions, and savepoints execute database writes.
- `db.runMigrations` and `db.runMigrationSql` apply schema changes.

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

### Selector CRUD helpers

<!-- quick-info: {"kind":"feature","name":"Selector CRUD helpers"} -->
The high-level CRUD helpers keep inserts explicit and route reads, updates, deletes, and upserts through a shared `table[predicate]` selector surface.
Selectors are pure descriptions of “which rows in which table”; they do not open connections, start transactions, or perform I/O by themselves.

These helpers currently execute in memory, just like `db.query`, `db.applyDelta`, and `db.applyDeltaOn`.
Their predicates and patches do not compile to SQL mutation statements.
<!-- /quick-info -->

<<< ../../snippets/from_md/stdlib/system/database/typed_mutations.aivi{aivi}

#### Selector values

`aivi.database` exposes a structural selector carrier:

```aivi
DbSelection A = { table: Table A, pred: Pred A }
```

`table[predicate]` is the constructor surface for this type.
When the left-hand side elaborates to `Table A`, the bracket body is interpreted as a row predicate of type `A -> Bool` using the same predicate-lifting rules already used by `db.where` and collection selectors.

That means all of these are valid selector forms:

- `userTable[id == userId]`
- `userTable[active]`
- `postTable[createdAt < cutoff]`

#### Explicit (`…On`) forms

| Function | Canonical meaning |
| --- | --- |
| **db.insertOn** conn table row<br><code>DbConnection -> Table A -> A -> Effect DbError (Table A)</code> | `db.applyDeltaOn conn table (Insert row)` |
| **db.rowsOn** conn selection<br><code>DbConnection -> DbSelection A -> Effect DbError (List A)</code> | `db.runQueryOn conn (db.where selection.pred (db.from selection.table))` |
| **db.firstOn** conn selection<br><code>DbConnection -> DbSelection A -> Effect DbError (Option A)</code> | `db.rowsOn conn selection`, then keep the first row if any |
| **db.deleteOn** conn selection<br><code>DbConnection -> DbSelection A -> Effect DbError (Table A)</code> | `db.applyDeltaOn conn selection.table (Delete selection.pred)` |
| **db.updateOn** conn selection patch<br><code>DbConnection -> DbSelection A -> Patch A -> Effect DbError (Table A)</code> | `db.applyDeltaOn conn selection.table (Update selection.pred patch)` |
| **db.upsertOn** conn selection row patch<br><code>DbConnection -> DbSelection A -> A -> Patch A -> Effect DbError (Table A)</code> | `db.applyDeltaOn conn selection.table (Upsert selection.pred row patch)` |

#### Ambient forms

| Function | Canonical meaning |
| --- | --- |
| **db.insert** table row<br><code>Table A -> A -> Effect DbError (Table A)</code> | `db.applyDelta table (Insert row)` |
| **db.rows** selection<br><code>DbSelection A -> Effect DbError (List A)</code> | `db.runQuery (db.where selection.pred (db.from selection.table))` |
| **db.first** selection<br><code>DbSelection A -> Effect DbError (Option A)</code> | `db.rows selection`, then keep the first row if any |
| **db.delete** selection<br><code>DbSelection A -> Effect DbError (Table A)</code> | `db.applyDelta selection.table (Delete selection.pred)` |
| **db.update** selection patch<br><code>DbSelection A -> Patch A -> Effect DbError (Table A)</code> | `db.applyDelta selection.table (Update selection.pred patch)` |
| **db.upsert** selection row patch<br><code>DbSelection A -> A -> Patch A -> Effect DbError (Table A)</code> | `db.applyDelta selection.table (Upsert selection.pred row patch)` |

Use the selector helpers for one-off CRUD operations.
Use `db.applyDeltas` or `db.applyDeltasOn` when you want to batch several deltas together explicitly.

#### Ambient vs explicit execution

Ambient selector operations run on the default connection configured with `db.configure`.
For example, `db.inTransaction (...)` can contain both `db.delete userTable[id == userId]` and `userTable[id == userId] <| { active: False }`, and both operations stay on that ambient connection for the whole transaction.

Explicit selector operations run on the `DbConnection` value passed to the helper.
For example, `db.inTransactionOn conn (...)` can contain both `db.deleteOn conn userTable[id == userId]` and `db.updateOn conn userTable[id == userId] (patch { active: False })`, and both operations stay on that specific connection for the whole transaction.

The selector itself never captures a connection and never changes transaction boundaries.

#### Selector-specific `<|` sugar

`<|` keeps its ordinary record-patching meaning on ordinary data, and gains one database-specific case when the left-hand side is a `DbSelection A`:

- `selection <| { ... }` is shorthand for `db.update selection (patch { ... })`
- `selection <| -` is shorthand for `db.delete selection`

So these forms are equivalent:

```aivi
db.update userTable[id == userId] (patch { role: "admin" })
userTable[id == userId] <| { role: "admin" }
```

and:

```aivi
db.delete userTable[id == userId]
userTable[id == userId] <| -
```

Standalone `-` remains invalid in ordinary expression position.
Its delete meaning exists only as the direct right-hand side of `<|` when the left-hand side is a database selector.

#### Typing and desugaring

The selector CRUD surface type-checks with these rules:

- if `table : Table A` and `pred` checks as `A -> Bool` under predicate lifting, then `table[pred] : DbSelection A`
- `db.rows : DbSelection A -> Effect DbError (List A)`
- `db.rowsOn : DbConnection -> DbSelection A -> Effect DbError (List A)`
- `db.first : DbSelection A -> Effect DbError (Option A)`
- `db.firstOn : DbConnection -> DbSelection A -> Effect DbError (Option A)`
- `db.update : DbSelection A -> Patch A -> Effect DbError (Table A)`
- `db.updateOn : DbConnection -> DbSelection A -> Patch A -> Effect DbError (Table A)`
- `db.delete : DbSelection A -> Effect DbError (Table A)`
- `db.deleteOn : DbConnection -> DbSelection A -> Effect DbError (Table A)`
- `db.upsert : DbSelection A -> A -> Patch A -> Effect DbError (Table A)`
- `db.upsertOn : DbConnection -> DbSelection A -> A -> Patch A -> Effect DbError (Table A)`

`selection <| { ... }` type-checks as `Effect DbError (Table A)` when `selection : DbSelection A` and the patch block checks as a valid `Patch A`.

`selection <| -` type-checks as `Effect DbError (Table A)` when `selection : DbSelection A`.

The surface desugars as follows:

| Surface form | Desugared form |
| --- | --- |
| `db.rows table[pred]` | `db.runQuery (db.where pred (db.from table))` |
| `db.rowsOn conn table[pred]` | `db.runQueryOn conn (db.where pred (db.from table))` |
| `db.first table[pred]` | `db.rows table[pred]`, then return the first row as `Option` |
| `db.firstOn conn table[pred]` | `db.rowsOn conn table[pred]`, then return the first row as `Option` |
| `db.update table[pred] patchFn` | `db.applyDelta table (Update pred patchFn)` |
| `db.updateOn conn table[pred] patchFn` | `db.applyDeltaOn conn table (Update pred patchFn)` |
| `db.delete table[pred]` | `db.applyDelta table (Delete pred)` |
| `db.deleteOn conn table[pred]` | `db.applyDeltaOn conn table (Delete pred)` |
| `db.upsert table[pred] seed patchFn` | `db.applyDelta table (Upsert pred seed patchFn)` |
| `db.upsertOn conn table[pred] seed patchFn` | `db.applyDeltaOn conn table (Upsert pred seed patchFn)` |
| `table[pred] <| { ... }` | `db.update table[pred] (patch { ... })` |
| `table[pred] <| -` | `db.delete table[pred]` |

This keeps the selector layer shallow on purpose.
It is syntax and API sugar over the existing single-table query and delta model, not a new transaction or connection mechanism.

#### Compile-fail cases and diagnostics

The compiler should reject at least these cases with targeted diagnostics:

| Example | Why it is rejected | Suggested help |
| --- | --- | --- |
| `db.delete userTable` | `db.delete` expects a `DbSelection A`, not a whole `Table A` | suggest `db.delete userTable[pred]` |
| `db.rows userTable[id]` | `id` lifts to `User -> Int`, but a selector predicate must resolve to `Bool` | suggest `userTable[id == someId]` or another boolean predicate |
| `db.update userTable[id == userId] { role: "admin" }` | the function form expects a `Patch A` value; plain braces in argument position are not a patch value | suggest `db.update userTable[id == userId] (patch { role: "admin" })` or `userTable[id == userId] <| { role: "admin" }` |
| `userTable[id == userId] <| 1` | selector patch shorthand accepts only a patch block or the delete marker `-` | suggest `userTable[id == userId] <| { ... }` or `userTable[id == userId] <| -` |
| `userTable[id == userId] <| { nope: True }` | the patch mentions a field that is not present on the selected row type | report the unknown field just as ordinary patching already does |

These are specification-level diagnostics, not exact error strings.
The important part is that the compiler explains whether the failure came from selector formation, patch typing, or misuse of selector shorthand.

### FTS helpers

`aivi.database` also includes typed helpers for full-text-search payloads and queries:

- `ftsDoc : Text -> List Text -> FtsDoc`
- `ftsMatchAny : List Text -> FtsQuery`
- `ftsMatchAll : List Text -> FtsQuery`

### Query DSL reference

| Function | What it does |
| --- | --- |
| **db.from** tbl<br><code>Table A -> Query A</code> | Lifts a table into a query source. |
| **db.where** pred q<br><code>(A -> Bool) -> Query A -> Query A</code> | Filters rows by a predicate. In lowered queries the filter is pushed into SQL. |
| **db.guard** cond<br><code>Bool -> Query Unit</code> | Inside `do Query`, continues when `cond` is `True` and produces no rows when it is `False`. |
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
| **Pool.create** config<br><code>Pool.Config DbConnection -> Effect DbError (Result Pool.PoolError (Pool DbConnection))</code> | Creates a connection pool from the given configuration. |
| **Pool.withConn** pool f<br><code>Pool DbConnection -> (DbConnection -> Effect DbError A) -> Effect DbError (Result Pool.PoolError A)</code> | Borrows a connection, runs `f`, and always releases the connection afterward. |

## Practical guidance

- Prefer explicit connections when a function opens, shares, or nests database work.
- Use ambient helpers when you truly want one process-wide default connection.
- Keep table definitions complete with explicit column lists if you want the query DSL to lower cleanly into SQL.
- Use savepoints for inner rollback boundaries instead of trying to nest transactions on the same connection.
- Reach for raw deltas when you want to construct mutations programmatically; reach for typed helpers when you want the shortest clear code.

## Notes

- The external-source form `db.query "SELECT ..."` is documented with [External Sources](../../syntax/external_sources.md) and [Schema-First Source Definitions](../../syntax/external_sources/schema_first.md), not with the `Query A` DSL on this page.
- `db.applyDelta`, `db.applyDeltas`, and the typed mutation helpers operate in memory rather than compiling predicates and patches into SQL mutation statements.
- Transactions are scoped to a single `DbConnection`.
