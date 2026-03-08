# `@deprecated` — Deprecation Warnings

<!-- quick-info: {"kind":"decorator","name":"@deprecated"} -->
`@deprecated` marks a value binding as obsolete. The compiler emits warning `W2500` everywhere code still refers to that name and includes your migration message.
<!-- /quick-info -->

Use `@deprecated` when you want to keep an older API available for a while but guide callers toward a better replacement.
The message should tell readers what to do next, not just that the old binding is discouraged.

## Syntax

<<< ../../snippets/from_md/syntax/decorators/deprecated/block_01.aivi{aivi}


The message argument is required and must be a string literal.

## Example

<<< ../../snippets/from_md/syntax/decorators/deprecated_example.aivi{aivi}

The old name still works, but every reference to `setTitle` now warns and points callers to `windowSetTitle`.

## What callers experience

The warning is attached to uses of the deprecated name, not just function calls.
That means the same rule covers deprecated functions, constants, and other value bindings that callers can still reference.
If another module imports and uses the old name, the warning appears at that use site, which keeps migration work visible wherever the old API is still in use.

## How to verify

Compile code that still refers to the deprecated name and confirm that the compiler reports warning `W2500`.
The warning includes both the deprecated name and the string literal message from the decorator.

## Diagnostics

| Code | Condition |
|:---- |:--------- |
| W2500 | Code refers to a deprecated value name |
| E1510 | `@deprecated` argument is not a string literal |
| E1511 | `@deprecated` is missing its message argument |

## Practical guidance

- Make the message actionable: name the replacement or the migration step.
- Prefer deprecating one binding at a time rather than describing a whole migration plan in the warning text.
- Keep the deprecated binding working until callers have a realistic path to move away from it.

## Related

- [Decorators](./index) — overview of built-in decorators
