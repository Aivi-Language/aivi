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
use aivi.db (Connection)

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
use aivi.core.dict (fromList)

use aivi.db (DbRow)

value sampleRow : DbRow =
    fromList [
        ("id", "7"),
        ("email", "ada@example.com")
    ]
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
use aivi.db (DbParam)

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
func describeDbError =
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
    statement
)

value connection : Connection = {
    database: "data/app.db"
}

@source db connection
signal database : DbSource

value loadUsers : Task Text (List (Map Text Text)) = database.query (statement "select * from users" [])
```

The source-backed side of the family stays on `db.connect` / `db.live`. On-demand database work
uses handle members such as `database.query ...` and `database.commit ...`, which return ordinary
`Task Text ...` values on the current command path.

Database tasks support ordinary `map`, `apply`, `chain`, and `join` composition.
Successful queries supply their rows to callbacks; successful commits supply
`Unit`. SQL failures stop the remaining task effects and make `aivi execute` exit
unsuccessfully. A successful task may itself return a `Result` value without
turning that payload into a task failure.

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
    TableRef
)

value conn : Connection = {
    database: "data/todos.db"
}

@source db conn
signal database : DbSource

value loadTodos : Task Text (List (Map Text Text)) = database.query {
    sql: "select id, title, done from todos order by id",
    arguments: []
}

signal todosChanged : Signal Unit
value todosTable : TableRef Unit = {
    name: "todos",
    conn: conn,
    changed: todosChanged
}

@source db.live loadTodos with {
    refreshOn: todosTable.changed
}
signal todos : Signal (Result DbError (List (Map Text Text)))
```

The `db.live` source runs the query on a worker thread and republishes whenever `refreshOn`
fires. After a successful `database.commit`, the runtime automatically advances matching
`.changed` signals, which triggers the refresh.

`db.live` publishes a successful task payload as `Ok payload` and converts task
failures into the declared signal error type. If the task payload is itself a
`Result`, it stays nested inside that outer `Ok`. Native success values, including
map rows, require no external decoder. Each successful commit notifies
matching reactive queries, including commits in a composed task whose later
steps fail. A rolled-back transaction sends no change notification.

### Inserting a row

```aivi
use aivi.db (
    Connection
    DbSource
    paramText
    statement
)

value insertConnection : Connection = {
    database: "data/todos.db"
}

@source db insertConnection
signal insertDatabase : DbSource

func insertTodo = title =>
    statement "insert into todos (title, done) values (?, 0)" [
        paramText title
    ]

value addTask : Task Text Unit =
    insertDatabase.commit ["todos"] [
        insertTodo "Buy groceries"
    ]
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
    TableRef
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

value listQuery : Task Text (List (Map Text Text)) = database.query {
    sql: "select id, title, done from todos order by id",
    arguments: []
}

signal todosChanged : Signal Unit
value todosTable : TableRef Unit = {
    name: "todos",
    conn: conn,
    changed: todosChanged
}

@source db.live listQuery with {
    refreshOn: todosTable.changed
}
signal todoRows : Signal (Result DbError (List (Map Text Text)))

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

```text
db.connect  →  database handle
                    ↓
db.live     →  todoRows signal (auto-refreshes after commits)
                    ↓
                todoCount derives from todoRows
                    ↓
                <Label> updates automatically
```

## Additional constructors and records

`SchemaMismatch message`, `QueryFailed message`, `ConstraintViolation message`, and
`ConnectionFailed message` preserve database failure details. `Asc` and `Desc` select ascending and
descending order. `TableRef A` identifies a typed table by its name, connection, and change signal.
