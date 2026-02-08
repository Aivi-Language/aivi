# Standard Library: SQLite Domain

SQLite as a **domain-backed type system** — tables become types, rows become patchable records, queries become predicates.

---

## Architecture

```text
┌─────────────────┐     ┌─────────────────┐
│   AIVI (WASM)   │────▶│  SQLite (WASM)  │
└─────────────────┘     └─────────────────┘
         │                       │
         └───── Component ───────┘
              Composition
```

Both AIVI and SQLite run as WASM modules. Communication via WASM Component Model — no FFI overhead.

---

## Module

```aivi
module aivi/std/sqlite = {
  export domain Db
  export Table, Row, Query, Column
  export connect, query, insert, update, delete
}
```

---

## Type-Safe Schema

### Table Definition

```aivi
@table `users`
User = {
  id: Int @primary @auto
  name: Text
  email: Text @unique
  createdAt: Instant @default now
}
```

The `@table` decorator generates:
- Type `User`
- Table creation SQL
- Query builders

### Compile-Time Schema Validation

```aivi
db : Source Db { users: Table User, posts: Table Post }
db = sqlite.connect `./app.db`

-- Compiler verifies table exists and has correct schema
users = db.users
```

---

## Domain Definition

```aivi
domain Db over (Table A) = {
  -- Query predicate
  (?) : Table A -> (A -> Bool) -> Query A
  (?) table pred = queryWhere table pred
  
  -- Patching rows
  (|) : Row A -> Patch A -> Effect Db (Row A)
  (|) row patch = updateRow row patch
  
  -- Insert
  (+) : Table A -> A -> Effect Db (Row A)
  (+) table record = insertRow table record
  
  -- Delete
  (-) : Table A -> (A -> Bool) -> Effect Db Int
  (-) table pred = deleteWhere table pred
}
```

---

## Query via Predicates

AIVI predicates compile directly to SQL WHERE clauses:

```aivi
-- Find active users
activeUsers = db.users ? (_.status == Active)

-- Compile to: SELECT * FROM users WHERE status = 'active'
```

### Complex Predicates

```aivi
-- Multiple conditions
results = db.users ? (u => u.age > 18 && u.verified == True)

-- Compile to: SELECT * FROM users WHERE age > 18 AND verified = true
```

### Joins via Record Composition

```aivi
userWithPosts = db.users
  |> map (u => { u, posts: db.posts ? (_.userId == u.id) })

-- Eager load posts for each user
```

---

## Patching Rows

Use AIVI's patch syntax to update rows:

```aivi
user = db.users ? (_.id == 1) |> head

-- Patch the row
updated = user | { name: `New Name`, email: `new@example.com` }
```

### Batch Updates

```aivi
-- Update all matching rows
db.users
  |> filter (_.status == Inactive)
  |> map (_ | { status: Archived })
```

---

## Insert and Delete

```aivi
-- Insert new row
newUser = db.users + { name: `Alice`, email: `alice@example.com` }

-- Delete by predicate
deleted = db.users - (_.createdAt < cutoffDate)
```

---

## Transactions

```aivi
transfer : Account -> Account -> Decimal -> Effect Db Unit
transfer from to amount = db.transaction do
  from | { balance: from.balance - amount }
  to | { balance: to.balance + amount }
  pure Unit
```

If any operation fails, entire transaction rolls back.

---

## Migrations

```aivi
@migration `001_add_verified_column`
addVerified : Effect Db Unit
addVerified = db.alter `users` do
  add `verified` Bool @default False
```

Migrations are AIVI code, tracked by the compiler.

---

## Type Safety Examples

```aivi
-- Compile error: 'age' field doesn't exist on User
bad = db.users ? (_.age > 18)

-- Compile error: comparing Text to Int
bad2 = db.users ? (_.name == 42)

-- Compile error: missing required field
bad3 = db.users + { name: `Bob` }  -- email required
```

---

## Generated SQL

AIVI queries optimize to efficient SQL:

```aivi
-- AIVI
recentActive = db.users
  ? (u => u.status == Active && u.lastLogin > yesterday)
  |> take 10
  |> map _.email

-- Generated SQL
-- SELECT email FROM users 
-- WHERE status = 'active' AND last_login > ? 
-- LIMIT 10
```

Only selected columns are fetched. Predicates pushed down.
