# Standard Library: Prelude

<!-- quick-info: {"kind":"module","name":"aivi.prelude"} -->
The **Prelude** is your default toolkit. It acts as the "standard library of the standard library," automatically using the core types and domains you use in almost every program (like `Int`, `List`, `Text`, and `Result`). It ensures you don't have to write fifty `use` lines just to add two numbers or print "Hello World".
<!-- /quick-info -->

<div class="import-badge">use aivi.prelude</div>

<<< ../../snippets/from_md/stdlib/core/prelude/standard_library_prelude.aivi{aivi}

## Opting Out

<<< ../../snippets/from_md/stdlib/core/prelude/opting_out.aivi{aivi}

## Rationale

- Common domains (dates, colors, vectors) are used universally
- Delta literals should "just work" without explicit `use`
- Explicit opt-out preserves control for advanced use cases

## Constructor introspection

The prelude also exposes two helpers for ADT values:

| Function | Type | Description |
| --- | --- | --- |
| `constructorName value` | `A -> Text` | Returns the constructor tag name (for example `Some`, `Err`, `Published`). |
| `constructorOrdinal value` | `A -> Int` | Returns the zero-based declaration index of the constructor inside its ADT definition. |
