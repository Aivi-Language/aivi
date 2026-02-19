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
