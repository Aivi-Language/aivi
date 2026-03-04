# `@test` — Test Declarations

<!-- quick-info: {"kind":"decorator","name":"@test"} -->
`@test` marks a definition as a test case or a module as test-only. Tests are collected by `aivi test` and excluded from production builds.
<!-- /quick-info -->

## Syntax

```aivi
// Test case (description is mandatory)
@test "description of what is tested"
testName = ...

// Test-only module
@test module ModuleName
```

## Example

<<< ../../snippets/from_md/syntax/decorators/test_example.aivi{aivi}

## Rules

- A description string is **mandatory** for test cases.
- When applied to a module, the entire module is test-only.
- Tests are discovered and executed by `aivi test`.
- Test-only modules are stripped from production builds.

---

## Mock Expressions

<!-- quick-info: {"kind":"syntax","name":"mock expression"} -->
Mock expressions provide **scoped binding substitution** for testing and isolation. A `mock ... in` expression temporarily replaces a module-level binding within a lexical scope.
<!-- /quick-info -->

### Syntax

```
mock <qualified.path> = <expr>
( mock <qualified.path> = <expr> )*
in <body>
```

- `<qualified.path>` — a dotted identifier referencing an imported binding (e.g. `rest.get`).
- `<expr>` — replacement expression; must type-check against the original binding's type.
- Multiple `mock` lines may precede a single `in`.

### Basic Example

<<< ../../snippets/from_md/syntax/mock_expression/basic.aivi{aivi}

### Multiple Mocks

<<< ../../snippets/from_md/syntax/mock_expression/multiple.aivi{aivi}

### Scoping Rules

| Rule | Behaviour |
|:-----|:----------|
| **Lexical** | Active only inside the `in <body>` expression |
| **Deep** | Transitive calls see the mock (runtime environment override) |
| **Nestable** | Inner `mock` blocks can re-shadow an outer mock |
| **Restore** | Original binding is restored after `body` completes (even on error) |
| **Qualified only** | Only qualified imported names can be mocked |

### Snapshot Mocks

The `mock snapshot` variant **records real responses** on first run and **replays from snapshot files** on subsequent runs:

<<< ../../snippets/from_md/syntax/mock_expression/snapshot.aivi{aivi}

| Mode | What happens |
|:-----|:-------------|
| **First run** (or `aivi test --update-snapshots`) | Calls real function, serializes to `__snapshots__/<test>/<binding>.snap` |
| **Subsequent runs** | Deserializes from `.snap` — no real call, deterministic |
| **Snapshot missing** | Fails: "run with `--update-snapshots`" |

### `assertSnapshot`

```aivi
assertSnapshot : Text -> A -> Effect Text Unit
```

Compares a serialized value against a stored snapshot. Pass `--update-snapshots` to re-record.

```aivi
@test "user formatting"
testFormat = do Effect {
  formatted <- pure (formatUserTable [{ id: 1, name: "Ada" }])
  assertSnapshot "user_table" formatted
}
```

### Compile-Time Errors

| Code | Condition |
|:-----|:----------|
| E1540 | `mock` target is not a qualified path |
| E1541 | `mock` target does not resolve to a known binding |
| E1542 | Mock expression type does not match original type |
| E1543 | `mock snapshot` used with `= expr` (mutually exclusive) |
| E1544 | Expected `in` keyword after mock binding(s) |

## Related

- [Testing Module](/stdlib/core/testing) — assertions, test runner, snapshot assertions
