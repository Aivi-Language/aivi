# Testing Module

<!-- quick-info: {"kind":"module","name":"aivi.testing"} -->
The `Testing` module is built right into the language because reliability shouldn't be an afterthought. Instead of hunting for third-party runners or configuring complex suites, you can just write `@test` next to your code. It provides a standard, unified way to define, discover, and run tests, making sure your code does exactly what you think it does (and keeps doing it after you refactor).

<!-- /quick-info -->
<div class="import-badge">use aivi.testing</div>

## Overview

<<< ../../snippets/from_md/stdlib/core/testing/overview.aivi{aivi}

Tests are ordinary bindings annotated with the `@test` [decorator](../../syntax/decorators.md). The `@test` decorator requires a mandatory description string that names the test case (e.g. `@test "adds two numbers"`). The test runner discovers all `@test` bindings and executes them, printing the description for each success and failure. A test passes when it completes without raising an assertion error.

## Core API (v0.1)

### Assertions

| Function | Explanation |
| --- | --- |
| **assert** condition<br><pre><code>`Bool -> Unit`</code></pre> | Fails the test when `condition` is `false`. |
| **assertEq** expected actual<br><pre><code>`A -> A -> Unit`</code></pre> | Fails the test when `expected` and `actual` are not equal (requires `Eq` constraint). |
| **assertNe** a b<br><pre><code>`A -> A -> Unit`</code></pre> | Fails the test when `a` and `b` are equal. |
| **assertOk** result<br><pre><code>`Result E A -> Unit`</code></pre> | Fails the test when `result` is `Err`. |
| **assertErr** result<br><pre><code>`Result E A -> Unit`</code></pre> | Fails the test when `result` is `Ok`. |
| **assertSome** option<br><pre><code>`Option A -> Unit`</code></pre> | Fails the test when `option` is `None`. |
| **assertNone** option<br><pre><code>`Option A -> Unit`</code></pre> | Fails the test when `option` is `Some`. |

### Running tests

Tests are executed via the CLI:

```sh
aivi test path/to/file.aivi      # run tests in a single file
aivi test path/to/directory       # run all tests in a directory
```

The runner prints a summary of passed / failed tests and returns a non-zero exit code when any test fails.

### Mocking REST/HTTP requests in tests

For request-heavy code, pass the request function as a dependency and use a typed mock in `@test` cases.

```aivi
use aivi.testing
use aivi.rest

User = { id: Int, name: Text }

fetchUsers = getUsers => getUsers ~u(https://api.example.com/users)

// Production wiring
liveFetchUsers = fetchUsers rest.get

@test "fetch users with a mocked request"
fetch_users_with_mock = do Effect {
  mockGet = _ => pure [{ id: 1, name: "Ada" }]
  users <- fetchUsers mockGet

  _ <- assertEq (List.length users) 1
  users match
    | [u, ..._] => assertEq u.name "Ada"
    | []        => fail "expected one mocked user"
}
```
