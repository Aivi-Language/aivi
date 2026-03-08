# `@test` — Test Declarations and Mocking

<!-- quick-info: {"kind":"decorator","name":"@test"} -->
`@test` marks a top-level definition as a runnable test. It can also decorate a module declaration to mark that module as test-oriented metadata. `aivi test` discovers decorated top-level definitions and runs them as `Effect`s.
<!-- /quick-info -->

Use `@test` for executable checks that belong in the test runner, not in shipping application logic.
Individual runnable tests are always top-level definitions. `@test` on a module declaration marks the module as test-oriented, but it does not by itself create a runnable test.

## Syntax

<<< ../../snippets/from_md/syntax/decorators/test/block_01.aivi{aivi}



## Example

<<< ../../snippets/from_md/syntax/decorators/test/block_02.aivi{aivi}


Run this file with `aivi test path/to/order_tests.aivi`. The runner reports failures by qualified name, such as `order_tests.addCommutative`.

## Practical rules

- Top-level test bindings require a description string, for example `@test "addition works"`.
- Module-level `@test` takes no argument: write `@test` on the line above `module ...`.
- `aivi test` discovers top-level definitions decorated with `@test`; module-level `@test` is metadata and does not itself create a runnable test.
- Each discovered test runs as an `Effect`.
- In current v0.1 tooling, module-level `@test` is organizational metadata, not a separate build-stripping mechanism for ordinary `aivi build` or `aivi run` workflows.
- Snapshot features on this page use the same runner with `--update-snapshots`; see [`aivi test`](/tools/cli#test) for the CLI surface.

### Decorator parse errors

| Code | Condition |
|:---- |:--------- |
| E1511 | Top-level `@test` is missing its description string |
| E1510 | Top-level `@test` argument is not a string literal |
| E1512 | Module-level `@test` was given an argument |

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
mock <binding.path> = <expr>
in <body>

mock <binding.path> = <expr>
mock <binding.path> = <expr>
in <body>

mock snapshot <binding.path>
in <body>
```

- Write the target as a binding path such as `rest.get`.
- `<expr>` is the replacement value or function, and it must type-check against the original binding.
- Multiple `mock` lines may appear before one `in` body.
- After parsing, the replacement expression and the body still go through the ordinary compiler pipeline.

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
| **Path-based** | You replace the binding named by the path, such as `rest.get` |

Use `mock ... in` for direct binding substitution in tests and temporary overrides.

### Snapshot mocks

The `mock snapshot` form records real responses when you run the test suite with `--update-snapshots`, then replays them from snapshot files on later plain test runs.
That gives you repeatable tests without keeping the real dependency online every time.

<<< ../../snippets/from_md/syntax/mock_expression/snapshot.aivi{aivi}

| Mode | What happens |
|:---- |:------------ |
| `aivi test --update-snapshots` | Calls the real function and stores the response in `__snapshots__/<module.path>/<testName>/<binding_path>.snap` |
| **Later runs** | Replays the stored `.snap` file instead of calling the real function |
| **Snapshot missing** | Fails and tells you to run with `--update-snapshots` |

For `mock snapshot`, the `<binding_path>` part of the filename is derived from the binding path, with dots written as underscores. For example, `rest.get` records to `rest_get.snap`.

### `assertSnapshot`

<<< ../../snippets/from_md/syntax/decorators/test/block_03.aivi{aivi}


`assertSnapshot` compares a serialized value against a stored snapshot.
Pass `--update-snapshots` when you intentionally want to re-record the expected output.

### Mock parse errors

| Code | Condition |
|:---- |:--------- |
| E1540 | `mock` is missing a usable binding path |
| E1543 | `mock snapshot` used with `= expr` |
| E1544 | Expected `in` after the mock binding list |

## Related

- [Testing Module](/stdlib/core/testing) — assertions, test runner, and snapshot assertions
- [`aivi test`](/tools/cli#test) — test discovery, filtering, and snapshot update mode
