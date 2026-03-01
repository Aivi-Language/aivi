# `@no_prelude` — Skip Prelude Import

<!-- quick-info: {"kind":"decorator","name":"@no_prelude"} -->
`@no_prelude` opts a module out of the implicit prelude import. Useful for low-level modules that intentionally avoid or redefine prelude symbols.
<!-- /quick-info -->

## Syntax

```aivi
@no_prelude module ModuleName
```

## Example

<<< ../../snippets/from_md/syntax/decorators/no_prelude_example.aivi{aivi}
