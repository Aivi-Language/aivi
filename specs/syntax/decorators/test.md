# `@test` — Test Declarations

<!-- quick-info: {"kind":"decorator","name":"@test"} -->
`@test` marks a definition as a test case or a module as test-only. Tests are collected by `aivi test` and excluded from production builds.
<!-- /quick-info -->

Use `@test` to mark executable checks that belong in the test runner, not in the shipping application.
You can apply it to individual definitions or to a whole module.

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

## Practical rules

- A description string is mandatory for test cases.
- When applied to a module, the whole module becomes test-only.
- Tests are discovered and executed by `aivi test`.
- Test-only modules are stripped from production builds.

---

## Mock expressions

Beyond marking tests, AIVI also provides `mock ... in` for short-lived dependency replacement inside one test or one expression.

<!-- quick-info: {"kind":"syntax","name":"mock expression"} -->
Mock expressions provide temporary binding replacement for testing and isolation. A `mock ... in` expression temporarily replaces a module-level binding within a lexical scope.
<!-- /quick-info -->

Use `mock ... in` when you want a test to replace a dependency for one expression without permanently changing the program.
This is useful for HTTP calls, clocks, file access, and other bindings that should behave differently in tests.

### Syntax

```text
mock <qualified.path> = <expr>
( mock <qualified.path> = <expr> )*
in <body>
```

- `<qualified.path>` is a dotted identifier for an imported binding such as `rest.get`.
- `<expr>` is the replacement value or function, and it must type-check against the original binding.
- Multiple `mock` lines may appear before one `in` body.

### Basic example

<<< ../../snippets/from_md/syntax/mock_expression/basic.aivi{aivi}

### Multiple mocks

<<< ../../snippets/from_md/syntax/mock_expression/multiple.aivi{aivi}

### Scoping rules

Read these rules as guardrails for “replace this dependency only here, then put it back”.

| Rule | Behaviour |
|:---- |:--------- |
| **Lexical** | The mock is active only inside the `in <body>` expression |
| **Deep** | Functions called from inside the body also see the mocked binding |
| **Nestable** | An inner `mock` block can shadow an outer one |
| **Restore** | The original binding is restored after `body` finishes, even on error |
| **Qualified only** | Only qualified imported names can be mocked |

Capability handlers in [Effect Handlers](/syntax/effect_handlers) solve a different problem: they install interpreters for capability scopes via `with { capability = handler } in`.
Prefer handlers for capability-polymorphic business logic, and use `mock ... in` for direct binding substitution.

### Snapshot mocks

The `mock snapshot` form records real responses on the first run and replays them from snapshot files on later runs.
That gives you repeatable tests without keeping the real dependency online every time.

<<< ../../snippets/from_md/syntax/mock_expression/snapshot.aivi{aivi}

| Mode | What happens |
|:---- |:------------ |
| **First run** or `aivi test --update-snapshots` | Calls the real function and stores the response in `__snapshots__/<test>/<binding>.snap` |
| **Later runs** | Replays the stored `.snap` file instead of calling the real function |
| **Snapshot missing** | Fails and tells you to run with `--update-snapshots` |

### `assertSnapshot`

```aivi
assertSnapshot : Text -> A -> Effect Text Unit
```

`assertSnapshot` compares a serialized value against a stored snapshot.
Pass `--update-snapshots` when you intentionally want to re-record the expected output.

```aivi
@test "user formatting"
testFormat = do Effect {
  formatted = formatUserTable [{ id: 1, name: "Ada" }]
  assertSnapshot "user_table" formatted   // compare against the stored golden result
}
```

### Compile-time errors

| Code | Condition |
|:---- |:--------- |
| E1540 | `mock` target is not a qualified path |
| E1541 | `mock` target does not resolve to a known binding |
| E1542 | Mock expression type does not match the original binding |
| E1543 | `mock snapshot` used with `= expr` |
| E1544 | Expected `in` after the mock binding list |

## Related

- [Testing Module](/stdlib/core/testing) — assertions, test runner, and snapshot assertions
