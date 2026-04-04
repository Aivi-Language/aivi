# aivi.db

Database records, query payloads, and handle vocabulary.

This module is the data vocabulary for database-backed features. It describes connections,
statements, parameters, paging options, errors, and the `DbSource` handle marker. The current
stdlib file does not export public `query` or `commit` functions on its own.

Current status: public database access is centered on `@source db`; this module is the shared
vocabulary for that capability family.

## Import

```aivi
use aivi.db (
    DbSource
    DbError
    SchemaMismatch
    QueryFailed
    ConstraintViolation
    NestedTransaction
    ConnectionFailed
    SortDir
    Asc
    Desc
    Connection
    TableRef
    DbRow
    DbParam
    DbStatement
    DbPageOpts
)
```

## Overview

| Type | Purpose |
|------|---------|
| `Connection` | Where to connect |
| `TableRef A` | Named table reference with a change signal |
| `DbRow` | Raw row data keyed by column name |
| `DbParam` | One bound query parameter |
| `DbStatement` | SQL text plus bound parameters |
| `SortDir` | Sort direction |
| `DbPageOpts` | Limit/offset paging options |
| `DbError` | Structured database failures |
| `DbSource` | Handle annotation for `@source db` |

---

## `Connection`

```aivi
type Connection = {
    database: Text
}
```

A small record naming the database target to open. In practice this is often a filename or connection string.

```aivi
use aivi.db (Connection)

value appDb : Connection = {
    database: "data/app.db"
}
```

---

## `TableRef A`

```aivi
type TableRef A = {
    name: Text,
    conn: Connection,
    changed: Signal Unit
}
```

Reference to a table together with the connection it belongs to and a signal you can watch for refreshes. The type parameter `A` lets you label the kind of rows you expect to read from that table.

```aivi
use aivi.db (
    Connection
    TableRef
)

type User = {
    id: Int,
    email: Text
}

type Connection -> Signal Unit -> TableRef User
func usersTable = conn changed =>
    {
        name: "users",
        conn: conn,
        changed: changed
    }
```

---

## `DbRow`

```aivi
type DbRow = Dict Text Text
```

A raw result row keyed by column name. Every field value is stored as `Text`, so decoding into richer application types happens somewhere else.

```aivi
use aivi.db (DbRow)

value sampleRow : DbRow = {
    entries: [
        { key: "id", value: "7" },
        {
            key: "email",
            value: "ada@example.com"
        }
    ]
}
```

---

## `DbParam`

```aivi
type DbParam = {
    kind: Text,
    bool: Option Bool,
    int: Option Int,
    float: Option Float,
    decimal: Option Decimal,
    bigInt: Option BigInt,
    text: Option Text,
    bytes: Option Bytes
}
```

A bound query parameter. `kind` tells the database layer which field to read. The matching optional field carries the actual value.

```aivi
use aivi.db (DbParam)

type Text -> DbParam
func textParam = value =>
    {
        kind: "text",
        bool: None,
        int: None,
        float: None,
        decimal: None,
        bigInt: None,
        text: Some value,
        bytes: None
    }
```

---

## `DbStatement`

```aivi
type DbStatement = {
    sql: Text,
    arguments: List DbParam
}
```

A SQL statement paired with its bound arguments.

```aivi
use aivi.db (
    DbParam
    DbStatement
)

type Text -> DbParam
func emailParam = value =>
    {
        kind: "text",
        bool: None,
        int: None,
        float: None,
        decimal: None,
        bigInt: None,
        text: Some value,
        bytes: None
    }

type Text -> DbStatement
func findUserByEmail = email =>
    {
        sql: "select * from users where email = ?",
        arguments: [emailParam email]
    }
```

---

## `SortDir`

```aivi
type SortDir = Asc | Desc
```

Sort direction for APIs that let you choose ordering.

---

## `DbPageOpts`

```aivi
type DbPageOpts = {
    limit: Int,
    offset: Int
}
```

Simple paging options.

- `limit` — how many rows to ask for
- `offset` — how many rows to skip first

```aivi
use aivi.db (DbPageOpts)

value firstPage : DbPageOpts = {
    limit: 50,
    offset: 0
}
```

---

## `DbError`

```aivi
type DbError =
  | SchemaMismatch Text
  | QueryFailed Text
  | ConstraintViolation Text
  | NestedTransaction
  | ConnectionFailed Text
```

Structured failure reasons for database work.

- `SchemaMismatch Text` — the stored schema does not match what the code expects
- `QueryFailed Text` — the query could not be run
- `ConstraintViolation Text` — a constraint such as uniqueness or foreign keys was violated
- `NestedTransaction` — a second transaction was started before the first one finished
- `ConnectionFailed Text` — the database could not be opened or reached

```aivi
use aivi.db (
    DbError
    SchemaMismatch
    QueryFailed
    ConstraintViolation
    NestedTransaction
    ConnectionFailed
)

type DbError -> Text
func describeDbError = error => error
 ||> SchemaMismatch msg      -> "schema mismatch: {msg}"
 ||> QueryFailed msg         -> "query failed: {msg}"
 ||> ConstraintViolation msg -> "constraint violation: {msg}"
 ||> NestedTransaction       -> "nested transactions are not supported"
 ||> ConnectionFailed msg    -> "connection failed: {msg}"
```

---

## Using the handle

```aivi
use aivi.db (
    DbSource
    Connection
    DbRow
    DbStatement
)

value connection : Connection = {
    database: "data/app.db"
}

@source db connection
signal database : DbSource

value loadUsersQuery : DbStatement = {
    sql: "select * from users",
    arguments: []
}

value loadUsers : Task Text (List DbRow) = database.query loadUsersQuery
```

The source-backed side of the family stays on `db.connect` / `db.live`. On-demand database work
uses handle members such as `database.query ...` and `database.commit ...`, which return ordinary
`Task Text ...` values on the current command path.

---

## Reactive queries with `db.live`

`db.live` turns a SQL query into a reactive signal that automatically refreshes when the
underlying data changes. This is the primary pattern for database-driven UIs in AIVI.

### Connecting and querying

```aivi
use aivi.db (
    DbSource
    DbError
    Connection
    DbRow
    DbStatement
    DbParam
)

value conn : Connection = {
    database: "data/todos.db"
}

@source db conn
signal database : DbSource

value loadTodos : DbStatement = {
    sql: "select id, title, done from todos order by id",
    arguments: []
}

@source db.live loadTodos with {
    refreshOn: database
}
signal todos : Signal (Result DbError (List DbRow))
```

The `db.live` source runs the query on a worker thread and republishes whenever `refreshOn`
fires. After a successful `database.commit`, the runtime automatically advances matching
`.changed` signals, which triggers the refresh.

### Inserting a row

```aivi
type Text -> DbParam
func textParam = value =>
    {
        kind: "text",
        bool: None,
        int: None,
        float: None,
        decimal: None,
        bigInt: None,
        text: Some value,
        bytes: None
    }

type Text -> DbStatement
func insertTodo = title =>
    {
        sql: "insert into todos (title, done) values (?, 0)",
        arguments: [textParam title]
    }

value addTask : Task Text Unit = database.commit (insertTodo "Buy groceries")
```

After the commit succeeds, `db.live` signals with `refreshOn: database` automatically re-query.

### Updating and deleting

```aivi
type Int -> DbParam
func intParam = value =>
    {
        kind: "int",
        bool: None,
        int: Some value,
        float: None,
        decimal: None,
        bigInt: None,
        text: None,
        bytes: None
    }

type Int -> DbStatement
func markDone = id =>
    {
        sql: "update todos set done = 1 where id = ?",
        arguments: [intParam id]
    }

type Int -> DbStatement
func deleteTodo = id =>
    {
        sql: "delete from todos where id = ?",
        arguments: [intParam id]
    }
```

### Putting it together

A minimal reactive todo list:

```aivi
use aivi.db (
    DbSource
    DbError
    Connection
    DbRow
    DbStatement
    DbParam
)

type Text -> DbParam
func textParam = value =>
    {
        kind: "text",
        bool: None,
        int: None,
        float: None,
        decimal: None,
        bigInt: None,
        text: Some value,
        bytes: None
    }

type Int -> DbParam
func intParam = value =>
    {
        kind: "int",
        bool: None,
        int: Some value,
        float: None,
        decimal: None,
        bigInt: None,
        text: None,
        bytes: None
    }

value conn : Connection = {
    database: "data/todos.db"
}

@source db conn
signal database : DbSource

value listQuery : DbStatement = {
    sql: "select id, title, done from todos order by id",
    arguments: []
}

@source db.live listQuery with {
    refreshOn: database
}
signal todoRows : Signal (Result DbError (List DbRow))

signal todoCount : Signal Text

value main =
    <Window title="Todos">
        <Box orientation="vertical" spacing={8}>
            <Label text={todoCount} />
        </Box>
    </Window>

export main
```

The data flow:

```
db.connect  →  database handle
                    ↓
db.live     →  todoRows signal (auto-refreshes after commits)
                    ↓
                todoCount derives from todoRows
                    ↓
                <Label> updates automatically
```
