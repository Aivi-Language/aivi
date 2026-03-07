# Standard Library: Prelude

<!-- quick-info: {"kind":"module","name":"aivi.prelude"} -->
The **Prelude** is your default toolkit. It acts as the "standard library of the standard library," automatically using the core types and domains you use in almost every program (like `Int`, `List`, `Text`, and `Result`). It ensures you don't have to write fifty `use` lines just to add two numbers or print "Hello World".
<!-- /quick-info -->

<div class="import-badge">use aivi.prelude</div>

<<< ../../snippets/from_md/stdlib/core/prelude/standard_library_prelude.aivi{aivi}

## What the Prelude does

The Prelude gathers the names that most programs need all the time. In many languages, this is the part of the standard library that “just works” without ceremony, and AIVI follows the same idea.

That means you can focus on your program first and think about extra imports only when you need more specialized modules.

## What is typically included

The Prelude brings the common building blocks of everyday AIVI code into scope, including core types, common domains, and the small helpers that make basic programs readable.

In practical terms, it is the reason simple files do not start with a long wall of `use` lines.

## Opting out

Most projects will keep the Prelude enabled. If you need very tight control over what is in scope, you can opt out and import only the pieces you want.

<<< ../../snippets/from_md/stdlib/core/prelude/opting_out.aivi{aivi}

This is mainly useful for advanced setups, generated code, or situations where you want every dependency to be explicit.

## Why the Prelude exists

- Common domains such as dates, colors, and vectors appear in many programs.
- Delta literals and other everyday conveniences should work immediately.
- A clear opt-out path keeps the language convenient without taking away control.

## Constructor introspection

The Prelude also exposes two helpers for algebraic data type values. These are useful for debugging, logging, tooling, and generic UI code.

| Function | Type | Description |
| --- | --- | --- |
| `constructorName value` | `A -> Text` | Returns the constructor tag name, such as `Some`, `Err`, or `Published`. |
| `constructorOrdinal value` | `A -> Int` | Returns the zero-based declaration index of the constructor inside its ADT definition. |
