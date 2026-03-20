# Database Domain

<!-- quick-info: {"kind":"module","name":"aivi.database"} -->
`aivi.database` describes relational schemas as typed values, builds first-class `Query` / `GroupedQuery` ASTs from relation roots, and executes record-shaped writes against explicit or ambient connections.
<!-- /quick-info -->
<div class="import-badge">use aivi.database<span class="domain-badge">domain</span></div>

## Start here

If you are new to `aivi.database`, the shortest useful path is:

1. declare one `Relation A`
2. run migrations for that relation
3. execute a query with `rows`, `first`, `count`, or `exists`
4. mutate rows with `insert`, `delete`, `update`, or `upsert`

<<< ../../snippets/from_md/stdlib/system/database/block_01.aivi{aivi}

## Core model

| Type | Meaning |
| --- | --- |
| `Relation A` | relation root / schema declaration for rows shaped like `A` |
| `Query A` | row query that eventually returns `A`-shaped rows |
| `GroupedQuery K A` | grouped rows of `A` keyed by `K` |
| `OrderTerm A` | one sort key for rows of `A` |
| `Order A` | alias for the ordering shape accepted by `orderBy` |
| `Agg A B` | alias for grouped aggregate results |

<<< ../../snippets/from_md/stdlib/system/database/types.aivi{aivi}

Relations are schema roots. Queries are shaped from those roots. Grouped queries change scope: row expressions such as `.email` become grouped expressions such as `key`, `count`, `sum .total`, and `max .createdAt`.

## Relation definitions

Declare relations with `relation`, column metadata, and optional links:

<<< ../../snippets/from_md/stdlib/system/database/domain_definition.aivi{aivi}

Links are used in two different positions:

- inside `[]`, a relation-valued link means existential filtering (`EXISTS`-style semantics)
- inside `selectMap`, a relation-valued link stays nested instead of flattening the parent row

## Query surface

<!-- quick-info: {"kind":"feature","name":"Relation queries"} -->
Every database read starts from a `Relation A` root.

- `relation[predicate]` filters rows
- `|> orderBy ...`, `|> limit ...`, `|> offset ...`, and `|> distinct` shape the result set
- `|> selectMap { ... }` projects a new row shape
- `|> groupBy ...` switches to grouped scope
- `|> having ...` filters grouped rows
- execution stays separate: `rows`, `first`, `count`, and `exists`
<!-- /quick-info -->

### Compact grammar

```text
query ::= relation
        | query "[" predicate "]"
        | query "|>" orderBy order
        | query "|>" limit int
        | query "|>" offset int
        | query "|>" distinct
        | query "|>" selectMap recordShape
        | query "|>" groupBy keyShape
        | groupedQuery "|>" having groupPredicate
        | groupedQuery "|>" selectMap groupRecordShape
```

`orderBy` accepts either one `asc` / `desc` term or a tuple of order terms such as `(desc .createdAt, asc .id)`.

### Core stages

<<< ../../snippets/from_md/stdlib/system/database/block_02.aivi{aivi}

<<< ../../snippets/from_md/stdlib/system/database/block_04.aivi{aivi}

<<< ../../snippets/from_md/stdlib/system/database/block_05.aivi{aivi}

### Execution helpers and connection styles

The same query values run against either the ambient default connection or an explicit `DbConnection` passed by the caller.

| Concern | Ambient helper | Explicit helper |
| --- | --- | --- |
| all rows / filtered rows | `rows query` | `rowsOn conn query` |
| first row | `first query` | `firstOn conn query` |
| row count | `count query` | `countOn conn query` |
| row existence | `exists query` | `existsOn conn query` |
| migrations | `runMigrations relations` | `runMigrationsOn conn relations` |
| sqlite tuning | `configureSqlite tuning` | `configureSqliteOn conn tuning` |

<<< ../../snippets/from_md/stdlib/system/database/block_03.aivi{aivi}

## Nested relation semantics

Predicate links are existential: a nested relation inside `[]` qualifies the parent row without multiplying it.

<<< ../../snippets/from_md/stdlib/system/database/block_06.aivi{aivi}

Projection links stay nested. A child relation projected inside `selectMap` yields nested child rows instead of a flattened join result.

<<< ../../snippets/from_md/stdlib/system/database/querying.aivi{aivi}

## Grouped queries and aggregates

Grouping is the stage that changes scope. After `groupBy`, grouped projections may use:

- `key`
- `count`
- `sum .field`
- `avg .field`
- `min .field`
- `max .field`

<<< ../../snippets/from_md/stdlib/system/database/block_07.aivi{aivi}

`having` is evaluated in grouped scope, and grouped `selectMap` produces an ordinary `Query` again, so you can continue with stages such as `orderBy`, `limit`, `offset`, or `distinct`.

## Writes

Database writes stay explicit and record-shaped.

- `insert` / `insertOn` insert one full row
- `delete` / `deleteOn` remove rows selected by a root relation query
- `update` / `updateOn` apply a record patch to every matched row
- `upsert` / `upsertOn` patch matching rows or insert the seed row when none match

<<< ../../snippets/from_md/stdlib/system/database/applying_deltas.aivi{aivi}

<<< ../../snippets/from_md/stdlib/system/database/typed_mutations.aivi{aivi}

## Migrations

Schema declarations are ordinary values, so the same relation metadata drives migrations, query lowering, and runtime row decoding.

<<< ../../snippets/from_md/stdlib/system/database/migrations.aivi{aivi}

## Transactions and savepoints

Connection ownership is separate from query construction.

- `beginTx` / `beginTxOn`, `commitTx` / `commitTxOn`, and `rollbackTx` / `rollbackTxOn` control transactions
- `inTransaction` / `inTransactionOn` wrap one effectful workflow in commit-or-rollback behavior
- `savepoint`, `releaseSavepoint`, and `rollbackToSavepoint` expose nested rollback markers

These helpers work with the same `Relation` / `Query` / write helpers described above; there is no separate transaction-specific query language.

## Pooling

`aivi.database.pool` manages reusable `DbConnection` values. The usual entry point is `Pool.withConn`, which borrows a connection, runs one effectful action, and returns the connection to the pool even when the action fails.

<<< ../../snippets/from_md/stdlib/system/database/pooling.aivi{aivi}

## Runtime effects

- `configure`, `connect`, `close`, and pooling helpers manage connections
- `runMigrations`, `runMigrationSql`, `rows`, `first`, `count`, and `exists` talk to the database
- `insert`, `delete`, `update`, and `upsert` mutate persisted rows
- transactions and savepoints add the usual commit / rollback control around those same helpers
