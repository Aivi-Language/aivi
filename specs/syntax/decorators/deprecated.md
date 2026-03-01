# `@deprecated` — Deprecation Warnings

<!-- quick-info: {"kind":"decorator","name":"@deprecated"} -->
`@deprecated` marks a binding as deprecated. The compiler emits a warning at every call site with the provided migration hint.
<!-- /quick-info -->

## Syntax

```aivi
@deprecated "migration hint message"
binding = ...
```

## Example

<<< ../../snippets/from_md/syntax/decorators/deprecated_example.aivi{aivi}

The compiler emits a warning at every call site. Use a human-readable migration hint as the argument.
