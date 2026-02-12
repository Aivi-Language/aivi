# Database Domain

The `Database` domain provides a type-safe, composable way to work with relational data. It treats tables as immutable records of schema plus rows, while compiling predicates and patches into efficient SQL under the hood.

It builds on existing AIVI features:
- **Domains** for operator overloading and delta literals
- **Predicates** for filtering and joins
- **Patching** for declarative updates
- **Effects** for explicit error handling

## Overview

```aivi
use aivi.database as db

User = { id: Int, name: Text, email: Text?, active: Bool, loginCount: Int, createdAt: Instant }

@static
userTable : Table User
userTable = db.table "users" [
  { name: "id", type: IntType, constraints: [AutoIncrement, NotNull], default: None }
  { name: "name", type: Varchar 100, constraints: [NotNull], default: None }
  { name: "email", type: Varchar 255, constraints: [], default: None }
  { name: "active", type: BoolType, constraints: [NotNull], default: Some (DefaultBool True) }
  { name: "loginCount", type: IntType, constraints: [NotNull], default: Some (DefaultInt 0) }
  { name: "createdAt", type: TimestampType, constraints: [NotNull], default: Some DefaultNow }
]

getActiveUsers : Effect DbError (List User)
getActiveUsers = effect {
  users <- db.load userTable
  pure (users |> filter active |> sortBy .createdAt)
}
```

Table schemas are defined with ordinary values. `db.table` takes a table name and a
list of `Column` values; the row type comes from the table binding's type annotation.

## Types

```aivi
// Tables carry schema metadata and hold rows
type Table A = {
  name: Text
  columns: List Column
  rows: List A
}

// Schema definitions are regular AIVI values.
// The row type is inferred from the table binding (e.g. Table User).
type ColumnType =
  | IntType
  | BoolType
  | TimestampType
  | Varchar Int

type ColumnConstraint =
  | AutoIncrement
  | NotNull

type ColumnDefault =
  | DefaultBool Bool
  | DefaultInt Int
  | DefaultText Text
  | DefaultNow

type Column = {
  name: Text
  type: ColumnType
  constraints: List ColumnConstraint
  default: ColumnDefault?
}

// Predicate alias
type Pred A = A => Bool

// Deltas express insert/update/delete
type Delta A =
  | Insert A
  | Update (Pred A) (Patch A)
  | Delete (Pred A)

// Patch functions apply record updates
type Patch A = A -> A
```

## Domain Definition

```aivi
domain Database over Table A = {
  type Delta = Delta A

  (+) : Table A -> Delta A -> Effect DbError (Table A)
  (+) table delta = db.applyDelta table delta

  ins = Insert
  upd = Update
  del = Delete
}
```

### Applying Deltas

```aivi
createUser : User -> Effect DbError User
createUser newUser = effect {
  _ <- userTable + ins newUser
  pure newUser
}

activateUsers : Effect DbError Unit
activateUsers = effect {
  _ <- userTable + upd (!active) (u => u <| { active: True, loginCount: _ + 1 })
}

deleteOldPosts : Instant -> Effect DbError Unit
deleteOldPosts cutoff = effect {
  _ <- postTable + del (_.createdAt < cutoff)
}
```

## Querying

Tables behave like lazy sequences. Operations such as `filter`, `find`, `sortBy`, `groupBy`, and `join` build a query plan. The query executes only when observed (e.g. via `db.load`, `toList`, or a generator).

```aivi
getUserById : Int -> Effect DbError (Option User)
getUserById id = effect {
  users <- db.load userTable
  pure (users |> find (_.id == id))
}
```

## Joins and Preloading

```aivi
UserWithPosts = { user: User, posts: List Post }

getUsersWithPosts : Effect DbError (List UserWithPosts)
getUsersWithPosts = effect {
  users <- db.load userTable
  posts <- db.load postTable
  pure (
    users
    |> join posts on (_.id == _.authorId)
    |> groupBy { userId = _.id, user = _.left, post = _.right }
    |> map { key, group } => {
      user: group.head.user,
      posts: group |> map .post
    }
  )
}
```

For eager loading:

```aivi
usersWithPosts <- db.load (userTable |> preload posts on (_.id == _.authorId))
```

## Migrations

Schema definitions are typed values. Mark them `@static` to allow compile-time validation and migration planning.

```aivi
migrate : Effect DbError Unit
migrate = effect {
  db.runMigrations [ userTable ]
}
```

## Notes

- `Database` compiles predicate expressions into `WHERE` clauses and patch instructions into `SET` clauses.
- Joins are translated into single SQL queries to avoid N+1 patterns.
- Advanced SQL remains available via `db.query` in [External Sources](../../02_syntax/12_external_sources.md).
