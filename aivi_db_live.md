# `aivi.db` — Live Reactive Database Layer

> Mutations invalidate queries. Queries re-fetch automatically.
> No manual refetch calls. No cache keys. The signal graph handles it.

---

## 1. The core mechanism: table change signals

Every table handle carries an implicit **change signal** — a `Signal Unit` that fires whenever a mutation commits against that table. Queries subscribe to this signal through `refreshOn`, and the scheduler propagates the update.

```aivi
type TableRef A = {
    name: Text,
    conn: Connection,
    changed: Signal Unit
}
```

`TableRef` is what `table` actually returns. The `changed` signal is runtime-managed — the database provider publishes into it after every successful `commit` targeting that table.

---

## 2. Declaring a live table

```aivi
@source db.connect config with { pool: 5 }
signal db : Signal (Result DbError Connection)

// table handle with its change signal
signal users =
    db
     T|> table "users" .
     F|> Err .
```

`table "users" conn` produces a `TableRef User`. The runtime allocates a `Signal Unit` for `"users"` scoped to this connection. Every `commit` against this table fires that signal.

---

## 3. Live queries — `@source db.live`

The key new provider variant: `db.live`. It wraps a query function and automatically re-executes when the referenced table changes.

```aivi
@source db.live (activeUsersQuery conn) with {
    refreshOn: users.changed,
    debounce: 100ms
}
signal activeUsers : Signal (Result DbError (List User))

fun activeUsersQuery:(Task DbError (List User)) conn:Connection =>
    table "users" conn
     |> all
     ?|> .active
     |> sortBy .name Asc
     |> fetch
```

What happens:
- Initial load executes the query immediately
- Any `commit` against `table "users"` fires `users.changed`
- `refreshOn: users.changed` causes the scheduler to re-execute the query
- `debounce: 100ms` batches rapid mutations into a single refetch
- The signal updates, the UI reacts

---

## 4. Mutations that trigger the refresh

Mutations look exactly like before — the magic is in `commit`:

```aivi
fun createUser:(Task DbError User) conn:Connection name:Text email:Text =>
    table "users" conn
     |> insert { name: name, email: email, active: True }
     |> returning
     |> commit

fun deactivateUser:(Task DbError Unit) conn:Connection userId:Int =>
    table "users" conn
     |> find (.id == userId)
     |> update <| { active: False }
     |> commit

fun removeUser:(Task DbError Unit) conn:Connection userId:Int =>
    table "users" conn
     |> find (.id == userId)
     |> delete
     |> commit
```

Every `commit` does two things:
1. Executes the mutation against the database
2. On success, publishes `Unit` into the table's `changed` signal

No annotation needed. No manual invalidation. The table handle already knows its change signal.

---

## 5. Multi-table invalidation

Queries that join multiple tables should refresh when *any* referenced table changes.

```aivi
type PostWithAuthor = {
    post: Post,
    author: User
}

fun postsWithAuthorsQuery:(Task DbError (List PostWithAuthor)) conn:Connection =>
    table "posts" conn
     |> all
     ?|> .post.published
     |> join (table "users" conn) (.authorId == .id) PostWithAuthor
     |> sortBy .post.title Asc
     |> fetch

// refresh when either table changes
signal postsOrUsersChanged =
 &|> posts.changed
 &|> users.changed
  |> mergeUnit

@source db.live (postsWithAuthorsQuery conn) with {
    refreshOn: postsOrUsersChanged,
    debounce: 200ms
}
signal postsWithAuthors : Signal (Result DbError (List PostWithAuthor))

fun mergeUnit:Unit a:Unit b:Unit => Unit
```

`&|>` combines the two change signals applicatively. When either fires, `postsOrUsersChanged` updates, and the join query re-executes.

---

## 6. Optimistic updates

For snappy UI, you sometimes want to show the expected result *before* the server round-trip completes. The `optimistic` option on `db.live` enables this:

```aivi
@source db.live (activeUsersQuery conn) with {
    refreshOn: users.changed,
    optimistic: True
}
signal activeUsers : Signal (Result DbError (List User))
```

With `optimistic: True`, the runtime:
1. Applies the patch locally to the last known result set
2. Publishes the optimistic result immediately
3. Fires the actual query in the background
4. Reconciles — if the server result differs, publishes the correction

The local patch application reuses the same `<|` patch semantics. An `update <| { active: False }` against a `find (.id == 5)` can be applied to the in-memory list *and* sent to the database.

### 6.1 Rollback on failure

```aivi
@source db.live (activeUsersQuery conn) with {
    refreshOn: users.changed,
    optimistic: True,
    onRollback: rollbackSignal
}
signal activeUsers : Signal (Result DbError (List User))
signal rollbackSignal : Signal DbError
```

If the server rejects the mutation, `rollbackSignal` fires with the error, and the query reverts to the last confirmed server state.

---

## 7. Scoped change tracking with `watch`

Sometimes you want finer-grained invalidation — not "the whole table changed" but "rows matching this condition changed."

```aivi
signal adminChanged =
    users
     |> watch ?|> .isAdmin

@source db.live (adminQuery conn) with {
    refreshOn: adminChanged
}
signal admins : Signal (Result DbError (List User))
```

`watch` attaches a predicate filter to the table's change signal. The runtime checks committed mutations against the predicate and only fires when a matching row was affected. This is a runtime optimization — the query still fetches fresh, but avoids unnecessary refetches when unrelated rows change.

---

## 8. CRUD action signals for UI wiring

In a full UI, mutations are triggered by user events. Here's the complete wiring:

```aivi
// --- connection ---

@source db.connect config with { pool: 5 }
signal db : Signal (Result DbError Connection)

signal users =
    db
     T|> table "users" .
     F|> Err .

// --- live query ---

@source db.live (listUsersQuery conn searchText) with {
    refreshOn: users.changed,
    debounce: 150ms
}
signal userList : Signal (Result DbError (List User))

fun listUsersQuery:(Task DbError (List User)) conn:Connection search:Text =>
    table "users" conn
     |> all
     ?|> .active and (.name |> contains search)
     |> sortBy .name Asc
     |> limit 100
     |> fetch

// --- input signals from UI ---

signal searchText : Signal Text
signal createClicked : Signal { name: Text, email: Text }
signal deleteClicked : Signal Int
signal toggleClicked : Signal Int

// --- mutation tasks ---

fun doCreate:(Task DbError User) conn:Connection input:{ name: Text, email: Text } =>
    table "users" conn
     |> insert { name: input.name, email: input.email, active: True }
     |> returning
     |> commit

fun doDelete:(Task DbError Unit) conn:Connection userId:Int =>
    table "users" conn
     |> find (.id == userId)
     |> delete
     |> commit

fun doToggle:(Task DbError Unit) conn:Connection userId:Int =>
    table "users" conn
     |> find (.id == userId)
     |> update <| { active: flipBool }
     |> commit

fun flipBool:Bool b:Bool =>
    b
     T|> False
     F|> True

// --- view ---

value view =
    <Window title="User Manager">
        <Box orientation="vertical" spacing={12}>
            <Entry
                text={searchText}
                placeholder="Search users..."
                onChanged={searchText}
            />

            <match on={userList}>
                <case pattern={Ok users}>
                    <each of={users} as={user} key={user.id}>
                        <Box orientation="horizontal" spacing={8}>
                            <Label text={user.name} />
                            <Label text={user.email} />
                            <Switch active={user.active} onToggled={toggleClicked user.id} />
                            <Button label="Delete" onClick={deleteClicked user.id} />
                        </Box>
                        <empty>
                            <Label text="No users found" />
                        </empty>
                    </each>
                </case>
                <case pattern={Err e}>
                    <Label text="Error loading users" />
                </case>
            </match>
        </Box>
    </Window>
```

The data flow is a closed loop:
1. `userList` fetches on load
2. User clicks Delete → `deleteClicked` fires → `doDelete` runs → `commit` fires `users.changed`
3. `userList` refetches automatically via `refreshOn`
4. UI updates with the new list

No imperative `refetch()`. No cache invalidation logic. The signal graph is the invalidation graph.

---

## 9. Summary — what the runtime does

| Event | Runtime action |
|---|---|
| `commit` succeeds | Publish `Unit` into the table's `changed` signal |
| `changed` fires | Scheduler marks downstream `db.live` queries as dirty |
| `debounce` window closes | Re-execute the query `Task`, publish new result |
| `optimistic: True` | Apply patch to in-memory result immediately, then confirm |
| Mutation fails | If optimistic, roll back to last server state and fire `onRollback` |
| `watch` predicate misses | Suppress the `changed` propagation for that specific listener |

The user writes:
- **Queries** as pipe chains with `fetch`
- **Mutations** as pipe chains with `<|` and `commit`
- **Liveness** as `@source db.live` with `refreshOn: table.changed`

Everything else — connection pooling, change tracking, debounced refetch, optimistic reconciliation — is provider-owned runtime behavior behind the source boundary.
