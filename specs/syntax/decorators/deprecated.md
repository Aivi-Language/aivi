# `@deprecated` — Deprecation Warnings

<!-- quick-info: {"kind":"decorator","name":"@deprecated"} -->
`@deprecated` marks a binding as deprecated. The compiler emits a warning at every call site.
<!-- /quick-info -->

## Syntax

```aivi
@deprecated "migration hint message"
binding = ...
```

## Example

<<< ../../snippets/from_md/syntax/decorators/deprecated_example.aivi{aivi}
