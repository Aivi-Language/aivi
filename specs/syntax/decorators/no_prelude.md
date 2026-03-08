# `@no_prelude` — Skip Prelude Import

<!-- quick-info: {"kind":"decorator","name":"@no_prelude"} -->
`@no_prelude` opts a module out of the implicit `use aivi.prelude`.
<!-- /quick-info -->

Use `@no_prelude` when you want full control over what a module imports.
This is most useful for low-level modules, generated code, teaching material, or compiler tests where every dependency should stay visible.

## Syntax

<<< ../../snippets/from_md/syntax/decorators/no_prelude/block_01.aivi{aivi}


`@no_prelude` takes no argument and must appear on its own line immediately before the `module` declaration.

## Example

<<< ../../snippets/from_md/syntax/decorators/no_prelude_example.aivi{aivi}

This example works without any `use` lines because it only relies on a locally defined binding.
As soon as the module needs prelude names such as `Option`, `Result`, `Some`, `None`, or `Text`, you must import them explicitly.

## What changes

Without the implicit prelude, the compiler does not auto-insert `use aivi.prelude`.
Names that would normally be available by default must be imported explicitly.
That makes dependencies more visible, but it also means even common helpers may need a `use` line.
If you want to opt out first and then restore the default namespace later in the file, add `use aivi.prelude` yourself.

## Related

- [Decorators](/syntax/decorators/)
- [Modules: The Prelude](/syntax/modules#106-the-prelude)
- [Standard Library: Prelude](/stdlib/core/prelude)
