# Testing Module

<!-- quick-info: {"kind":"module","name":"aivi.testing"} -->
`aivi.testing` provides the assertion helpers and snapshot assertions used inside `@test` definitions. It keeps ordinary unit checks, capability-based tests, and snapshot verification in the same built-in workflow as `aivi test`.

<!-- /quick-info -->
<div class="import-badge">use aivi.testing</div>

## What this module is for

`aivi.testing` is the standard library module for executable checks in AIVI. It is meant for the code that runs **inside** a test body: assertions, equality checks, and snapshot comparisons.

This page focuses on the module itself. For test discovery, the `@test` decorator, and full `mock ... in` rules, see the linked specs in [Related](#related).

## Overview

```aivi
use aivi
use aivi.testing

add = left right => left + right

@test "addition works"
additionWorks = do Effect {
  assertEq (add 1 1) 2
}
```

Tests are top-level bindings annotated with the [`@test` decorator](/syntax/decorators/test). The decorator's description string, such as `@test "addition works"`, is what the runner shows in its output.

A test passes when its `Effect` completes without failure. Assertion helpers in `aivi.testing` fail with `Text` messages when a check does not hold.

## Core API (v0.1)

### Assertions

| Function | Explanation |
| --- | --- |
| **assert** condition<br><code>Bool -> Effect Text Unit</code> | Fails when `condition` is `False`. |
| **assertEq** expected actual<br><code>A -> A -> Effect Text Unit</code> | Fails when `expected` and `actual` differ. This is the preferred camelCase name in examples. |
| **assert_eq** expected actual<br><code>A -> A -> Effect Text Unit</code> | Alias of `assertEq` with the same behaviour. |
| **assertSnapshot** name value<br><code>Text -> A -> Effect Text Unit</code> | Compares the current serialized value against `<project>/__snapshots__/<module.path>/<testName>/<name>.snap`. With `--update-snapshots`, it writes or refreshes that file instead. |

These are the helpers exported by the current v0.1 implementation of `aivi.testing`.

### Running tests

Use the CLI to execute tests:

```sh
aivi test path/to/file.aivi       # Run tests in one file
aivi test path/to/directory       # Run every test in a directory tree
aivi test path/to/directory --only additionWorks
aivi test path/to/directory --update-snapshots
```

The runner discovers top-level `@test` bindings under the target, executes each one as an `Effect`, reports failures by qualified name, and returns a non-zero exit code when any test fails. It also writes passed and failed file lists to `target/aivi-test-passed-files.txt` and `target/aivi-test-failed-files.txt`.

## Testing capability-based code

When production code depends on capabilities, the usual testing approach is to install scoped handlers with [`with { capability = handler } in`](/syntax/effect_handlers).

In the example below, assume `readConfig` reads through `file.read` and `process.env.read`, while `fixtureFiles` and `fixtureEnv` return deterministic test data.

<<< ../../snippets/from_md/stdlib/core/testing/block_01.aivi{aivi}


This swaps the capability interpreters only inside the test scope, so the production logic stays unchanged.

## Mocking REST and HTTP calls

Prefer capability handlers for new code. When code is still written around imported bindings such as `rest.get`, [`mock ... in` expressions](/syntax/decorators/test#mock-expressions) are a practical binding-level fallback.

<<< ../../snippets/from_md/stdlib/core/testing/block_02.aivi{aivi}


Here the mock replaces `rest.get` only inside the test body, so the example can exercise the surrounding logic without making a real network call.

Mock expressions provide **deep scoping**: functions called inside the mocked body also see the mock binding. See the [Mock Expressions](/syntax/decorators/test#mock-expressions) spec for multiple mocked bindings, restore rules, and `mock snapshot`.

Prefer capability handlers when the code under test already exposes capabilities. Keep `mock ... in` for binding-level substitution, snapshots, and APIs that have not yet been expressed as capabilities.

## Snapshot assertions

`assertSnapshot` compares a serialized value against a stored `.snap` file.

- `aivi test path --update-snapshots` writes a new snapshot file or refreshes an existing one.
- Plain `aivi test path` compares the current value against the existing snapshot file.
- If the snapshot file is missing and you did not pass `--update-snapshots`, the test fails and tells you to re-run in update mode.
- Snapshot files live under `__snapshots__/<module.path>/<testName>/<name>.snap`.

Snapshot tests are especially helpful for larger structured output such as formatted text, generated code, or UI trees.

## Related

- [`@test` decorator and `mock ... in`](/syntax/decorators/test)
- [Effect handlers](/syntax/effect_handlers)
- [`aivi test` CLI command](/tools/cli#test)
