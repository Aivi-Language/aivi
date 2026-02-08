# Standard Library: SQLite Domain

SQLite as a **domain-backed type system** — tables become types, rows become patchable records, and operations align with core language traversals.

---

## Conceptual Model: Tables as Arrays

In AIVI, a `Table A` is conceptually equivalent to a `List (Row A)`. This allows all standard list operations (`map`, `filter`, `take`) to compile directly to efficient SQL.

```aivi
// Conceptually:
Table A ≈ List (Row A)
```

---

## Module

```aivi
module aivi.std.sqlite = {
  export domain Db
  export Table, Row, Query, Column
  export connect, query, insert, update, delete
}
```

---

## Type-Safe Schema

```aivi
@table "users"
Status = Active | Inactive

User = {
  id: Int @primary @auto
  name: Text
  email: Text @unique
  status: Status
}

db : Source Db { users: Table User, posts: Table Post }
db = sqlite.connect "./app.db"
```

---

## Domain Definition

```aivi
domain Db over (Table A) = {
  // Query via filter
  filter : Table A -> (A -> Bool) -> Query A
  filter table pred = queryWhere table pred
  
  // Row fetching / lookup
  (<=) : Table A -> { id: Int } -> Row A
  (<=) table { id } = fetchRowById table id

  // Mutations via traversal (see Traversal section)
  traverse : Query A -> (Row A -> Effect Db B) -> Effect Db (List B)
  traverse query fn = runEffectfulMutation query fn
}
```

---

## Operations

### Query (filter)

AIVI predicates in `filter` compile directly to SQL `WHERE` clauses:

```aivi
// Find active users
activeUsers = db.users filter (status == Active)

// Compile to: SELECT * FROM users WHERE status = "active"
```

### Lookup (<=)

Directly retrieve a single record by its primary key:

```aivi
// Fetch user with ID 1
user = db.users <= { id: 1 }

// Compile to: SELECT * FROM users WHERE id = 1 LIMIT 1
```

### Patching Rows

Use the standard patch operator on a fetched row to trigger an `UPDATE`:

```aivi
user = db.users <= { id: 1 }

// Patching a Row record triggers an effectful update
effect {
  user <= { name: "Grace" }
}
```

---

## Insert and Delete via Traverse

Instead of dedicated operators, mutations are modeled as traversing the table.

### Insert

```aivi
// Inserting is a traversal that yields a new row
effect {
  db.users |> traverse (_ => Insert { 
    name: "Alice", 
    email: "alice@example.com",
    status: Active 
  })
}
```

### Delete

```aivi
// Deleting is a filtered traversal
effect {
  db.users 
    filter (status == Inactive)
    |> traverse delete
}
```

---

## Generated SQL

AIVI queries optimize to efficient SQL:

```aivi
// AIVI
recentActive = db.users
  filter (u => u.status == Active && u.lastLogin > yesterday)
  |> take 10
  |> map _.email

// Generated SQL
// SELECT email FROM users 
// WHERE status = "active" AND last_login > ? 
// LIMIT 10
```

Only selected columns are fetched. Predicates are pushed down to the engine.
## Expressive Data Access

SQLite domains leverage AIVI's patching and pipelines for very concise data orchestration.

### Fluent Joins
```aivi
// Fetch user with posts in one pipeline
user1 = db.users <= { id: 1 }
  |> map (u => { u, posts: db.posts filter (_.userId == u.id) })
```

### Conditional Batch Updates
```aivi
// Reactivate all users who logged in recently but are marked inactive
effect {
  db.users
    filter (u => u.status == Inactive && u.lastLogin > thirtyDaysAgo)
    |> traverse (_ <= { status: Active })
}
```

### Expressive Existence Checks
```aivi
// Boolean check for record existence
userExists = email => db.users 
  filter (_.email == email) 
  |> head 
  |> isSome
```
