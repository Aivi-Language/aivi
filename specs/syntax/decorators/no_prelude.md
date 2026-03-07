# `@no_prelude` — Skip Prelude Import

<!-- quick-info: {"kind":"decorator","name":"@no_prelude"} -->
`@no_prelude` opts a module out of the implicit prelude import.
<!-- /quick-info -->

Use `@no_prelude` when you want full control over what a module imports.
This is most useful for minimal examples, teaching material, compiler tests, or modules that intentionally avoid the default namespace.

## Syntax

```aivi
@no_prelude module ModuleName
```

## Example

<<< ../../snippets/from_md/syntax/decorators/no_prelude_example.aivi{aivi}

## What changes

Without the implicit prelude, names that would normally be available by default must be imported explicitly.
That makes dependencies more visible, but it also means even common helpers may need a `use` line.
