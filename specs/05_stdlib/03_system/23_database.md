# Database Domain

The `Database` domain provides a type-safe, composable way to work with relational data. It treats tables as immutable records of schema plus rows, while compiling predicates and patches into efficient SQL under the hood.

It builds on existing AIVI features:
- **Domains** for operator overloading and delta literals
- **Predicates** for filtering and joins
- **Patching** for declarative updates
- **Effects** for explicit error handling

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

Tables behave like lazy sequences. Operations such as `filter`, `find`, `sortBy`, `groupBy`, and `join` build a query plan. The query executes only when observed (e.g. via `db.load`, `toList`, or a generator).

<<< ../../snippets/from_md/05_stdlib/03_system/23_database/block_05.aivi{aivi}

## Joins and Preloading

<<< ../../snippets/from_md/05_stdlib/03_system/23_database/block_06.aivi{aivi}

For eager loading:

<<< ../../snippets/from_md/05_stdlib/03_system/23_database/block_07.aivi{aivi}

## Migrations

Schema definitions are typed values. Mark them `@static` to allow compile-time validation and migration planning.

<<< ../../snippets/from_md/05_stdlib/03_system/23_database/block_08.aivi{aivi}

## Notes

- `Database` compiles predicate expressions into `WHERE` clauses and patch instructions into `SET` clauses.
- Joins are translated into single SQL queries to avoid N+1 patterns.
- Advanced SQL remains available via `db.query` in [External Sources](../../02_syntax/12_external_sources.md).
