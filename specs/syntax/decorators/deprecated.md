# `@deprecated` — Deprecation Warnings

<!-- quick-info: {"kind":"decorator","name":"@deprecated"} -->
`@deprecated` marks a binding as deprecated. The compiler emits a warning at every call site.
<!-- /quick-info -->

Use `@deprecated` when you want to keep an older API available for a while but guide callers toward a better replacement.
The message should tell readers what to do next, not just that the old binding is discouraged.

## Syntax

```aivi
@deprecated "use newName instead"
binding = ...
```

## Example

<<< ../../snippets/from_md/syntax/decorators/deprecated_example.aivi{aivi}

## What callers experience

When another module uses the deprecated binding, the compiler emits a warning at the call site.
This keeps migration work visible wherever the old API is still in use.

## Practical guidance

- Make the message actionable: name the replacement or the migration step.
- Prefer deprecating one binding at a time rather than describing a whole migration plan in the warning text.
- Keep the deprecated binding working until callers have a realistic path to move away from it.
