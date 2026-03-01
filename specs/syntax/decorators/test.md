# `@test` — Test Declarations

<!-- quick-info: {"kind":"decorator","name":"@test"} -->
`@test` marks a definition as a test case or a module as test-only. Tests are collected by `aivi test` and excluded from production builds.
<!-- /quick-info -->

## Syntax

```aivi
// Test case (description is mandatory)
@test "description of what is tested"
testName = ...

// Test-only module (entire module excluded from production)
@test module ModuleName
```

## Example

<<< ../../snippets/from_md/syntax/decorators/test_example.aivi{aivi}

## Rules

- A description string is **mandatory** for test cases.
- When applied to a module (`@test module M`), the entire module is test-only.
- Tests are discovered and executed by `aivi test`.
- Test-only modules and their contents are stripped from production builds.
