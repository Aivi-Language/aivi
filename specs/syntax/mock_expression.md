# Mock Expressions

<!-- quick-info: {"kind":"syntax","name":"mock expression"} -->
Mock expressions provide **scoped binding substitution** for testing and isolation.
A `mock ... in` expression temporarily replaces a module-level binding within a
lexical scope, enabling tests to swap out external dependencies (HTTP clients,
database connections, file I/O) without restructuring production code.

<!-- /quick-info -->

## Syntax

```
mock <qualified.path> = <expr>
( mock <qualified.path> = <expr> )*
in <body>
```

- `<qualified.path>` — a dotted identifier path referencing an imported or module-level binding (e.g. `rest.get`, `file.readAll`).
- `<expr>` — the replacement expression. Must type-check against the original binding's type.
- `<body>` — the expression evaluated with the mock(s) in effect.
- Multiple `mock` lines may precede a single `in`.

## Basic Example

<<< ../snippets/from_md/syntax/mock_expression/basic.aivi{aivi}

## Multiple Mocks

<<< ../snippets/from_md/syntax/mock_expression/multiple.aivi{aivi}

## Semantics

### Deep Scoping

Mock substitutions are **deep**: any function called within `body` that internally
references the mocked binding will see the mock value, not the original. This is
because all global bindings are resolved dynamically through the runtime environment.

```aivi
fetchUsers = rest.get ~u(https://api.example.com/users)

@test "deep scoping"
deepTest =
  mock rest.get = _ => pure []
  in do Effect {
    users <- fetchUsers   // fetchUsers calls rest.get internally — sees the mock
    assertEq (List.length users) 0
  }
```

### Type Safety

The mock expression must unify with the original binding's type. A type mismatch
is a compile error:

```aivi
// rest.get : Url -> Effect Text A
// ❌ Compile error: mock type `Int -> Int` does not match `Url -> Effect Text A`
mock rest.get = x => x + 1
in fetchUsers
```

### Scoping Rules

| Rule | Behaviour |
|:-----|:----------|
| **Lexical** | Mock is active only inside the `in <body>` expression |
| **Deep** | Transitive calls see the mock (runtime environment override) |
| **Nestable** | Inner `mock` blocks can re-shadow an outer mock |
| **Restore** | The original binding is restored after `body` completes (even on error) |
| **Qualified only** | Only qualified imported names can be mocked; use `let` for local bindings |

### Nesting

```aivi
@test "nested mocks"
nestedTest =
  mock rest.get = _ => pure []
  in
    mock rest.get = _ => pure [{ id: 1, name: "Ada" }]
    in do Effect {
      users <- fetchUsers   // sees the inner mock
      assertEq (List.length users) 1
    }
```

## Snapshot Mocks

The `mock snapshot` variant **records real responses** on the first run and
**replays from snapshot files** on subsequent runs:

<<< ../snippets/from_md/syntax/mock_expression/snapshot.aivi{aivi}

### Behaviour

| Mode | What happens |
|:-----|:-------------|
| **First run** (or `aivi test --update-snapshots`) | Calls the real function, serializes the response to `__snapshots__/<test_name>/<binding>.snap` |
| **Subsequent runs** | Deserializes from the `.snap` file — no real call, deterministic and fast |
| **Snapshot missing** | Fails with a clear error: "snapshot file not found; run with `--update-snapshots`" |

Snapshot files are keyed by the test name (derived from the `@test` description) and the mocked binding path. Multiple calls to the same binding are recorded in order.

### CLI Flags

| Command | Behaviour |
|:--------|:----------|
| `aivi test` | Replay from existing `.snap` files; fail if missing |
| `aivi test --update-snapshots` | Re-record all snapshot mocks from real calls |

## Snapshot Assertions

The testing module also provides `assertSnapshot` for **output comparison**:

```aivi
assertSnapshot : Text -> A -> Effect Text Unit
```

- On first run (or `--update-snapshots`): serializes the value and writes to `__snapshots__/<test>/<name>.snap`.
- On subsequent runs: compares the serialized value against the stored snapshot. Fails with a diff on mismatch.

```aivi
@test "user formatting"
testFormat = do Effect {
  users <- pure [{ id: 1, name: "Ada" }]
  formatted <- pure (formatUserTable users)
  assertSnapshot "user_table" formatted
}
```

## Compile-Time Errors

| Code | Condition |
|:-----|:----------|
| E1540 | `mock` target is not a qualified path |
| E1541 | `mock` target does not resolve to a known binding |
| E1542 | Mock expression type does not match original binding type |
| E1543 | `mock snapshot` used with `= expr` (snapshot and explicit mock are mutually exclusive) |
| E1544 | Expected `in` keyword after mock binding(s) |
