# Testing Module

<!-- quick-info: {"kind":"module","name":"aivi.testing"} -->
The `Testing` module is built right into the language because reliability shouldn't be an afterthought. Instead of hunting for third-party runners or configuring complex suites, you can just write `@test` next to your code. It provides a standard, unified way to define, discover, and run tests, making sure your code does exactly what you think it does (and keeps doing it after you refactor).

<!-- /quick-info -->
<div class="import-badge">use aivi.testing</div>

## What this module is for

`aivi.testing` is the standard way to write automated checks for AIVI code. Tests live next to ordinary code, use normal language features, and run through the built-in test runner.

That makes it easy to start small with a few assertions and grow toward larger test suites without switching tools.

## Overview

<<< ../../snippets/from_md/stdlib/core/testing/overview.aivi{aivi}

Tests are ordinary bindings annotated with the `@test` [decorator](../../syntax/decorators/). The decorator takes a description string such as `@test "adds two numbers"`, and the runner uses that text when reporting results.

A test passes when it finishes without raising an assertion error.

## Core API (v0.1)

### Assertions

| Function | Explanation |
| --- | --- |
| **assert** condition<br><code>Bool -> Effect Text Unit</code> | Fails the test when `condition` is false. |
| **assertEq** expected actual<br><code>A -> A -> Effect Text Unit</code> | Fails when `expected` and `actual` are not equal. |
| **assertNe** a b<br><code>A -> A -> Effect Text Unit</code> | Fails when `a` and `b` are equal. |
| **assertOk** result<br><code>Result E A -> Effect Text Unit</code> | Fails when `result` is `Err`. |
| **assertErr** result<br><code>Result E A -> Effect Text Unit</code> | Fails when `result` is `Ok`. |
| **assertSome** option<br><code>Option A -> Effect Text Unit</code> | Fails when `option` is `None`. |
| **assertNone** option<br><code>Option A -> Effect Text Unit</code> | Fails when `option` is `Some`. |
| **assertSnapshot** name value<br><code>Text -> A -> Effect Text Unit</code> | Compares `value` against a stored `.snap` file identified by `name`. The first run creates the snapshot. |

### Running tests

Use the CLI to execute tests:

```sh
aivi test path/to/file.aivi       # Run tests in one file
aivi test path/to/directory       # Run every test in a directory tree
```

The runner prints a pass/fail summary and returns a non-zero exit code when any test fails.

## Testing capability-based code

When production code depends on capabilities, the usual testing approach is to install scoped handlers with [`with { capability = handler } in`](/syntax/effect_handlers).

```aivi
@test "read config from fixtures"
readConfigFromFixtures =
  with {
    file.read = fixtureFiles,
    process.env.read = fixtureEnv
  } in do Effect {
    cfg <- readConfig
    _ <- assertEq cfg.mode "test"
    pure Unit
  }
```

This swaps the capability interpreters only inside the test scope, so the production logic stays unchanged.

## Mocking REST and HTTP calls

When code is still written around top-level bindings rather than capability signatures, [`mock ... in` expressions](/syntax/decorators/test#mock-expressions) are a practical fallback.

```aivi
use aivi.testing
use aivi.rest

User = { id: Int, name: Text }

fetchUsers = rest.get ~u(https://api.example.com/users)

@test "fetch users with a mocked request"
fetchUsersWithMock =
  mock rest.get = _ => pure [{ id: 1, name: "Ada" }]
  in do Effect {
    users <- fetchUsers
    _ <- assertEq (length users) 1
    users match
      | [u, ..._] => assertEq u.name "Ada"
      | []        => fail "expected one mocked user"
  }
```

Here the mock replaces `rest.get` only inside the test body, so the example can exercise the surrounding logic without making a real network call.

Mock expressions provide **deep scoping**: functions called inside the mocked body also see the mock binding. See the [Mock Expressions](/syntax/decorators/test#mock-expressions) spec for details on snapshot mocks and multiple mocked bindings.

Prefer capability handlers when the code under test already exposes capabilities. Keep `mock ... in` for binding-level substitution, snapshots, and APIs that have not yet been expressed as capabilities.

## Snapshot assertions

`assertSnapshot` compares a rendered value against a stored `.snap` file.

- On the first run, the snapshot file is created.
- On later runs, the current value is compared to the stored one.
- `aivi test --update-snapshots` refreshes existing snapshots when a change is intentional.

Snapshot tests are especially helpful for larger structured output such as formatted text, generated code, or UI trees.
