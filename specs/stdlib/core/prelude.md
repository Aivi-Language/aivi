# Standard Library: Prelude

<!-- quick-info: {"kind":"module","name":"aivi.prelude"} -->
The `aivi.prelude` module is imported implicitly into every module. It brings the core types, common constructors, small helper functions, and a handful of widely used domains into scope so ordinary AIVI code can start without boilerplate imports.
<!-- /quick-info -->

<div class="import-badge">use aivi.prelude</div>

You usually do **not** write this line yourself: every module implicitly starts with `use aivi.prelude`. Write it explicitly only when re-enabling the prelude after [`@no_prelude`](/syntax/decorators/no_prelude) or when showing imports in teaching material.

## What the Prelude does

The Prelude is the default namespace for day-to-day AIVI code. Its job is to make common programs readable from the first line while still leaving an escape hatch for modules that want a fully explicit import list.

That is why simple code can use names such as `Int`, `Text`, `Option`, `Some`, `None`, `Result`, `Ok`, and `Err` without extra `use` lines.

```aivi
module docs.prelude.example

status : Option Int
status = Some 3

summary : Text
summary = status match
  | Some n => "count = {n}"
  | None   => "no value"
```

## What is typically included

The Prelude is a curated surface, not the entire standard library. In practice it brings the names most programs reach for immediately:

- core types and constructors such as `Int`, `Float`, `Bool`, `Text`, `List`, `Option`, `Result`, `Tuple`, `Some`, `None`, `Ok`, and `Err`
- common interfaces and helpers such as `ToText`, `toText`, `not`, and `any`
- commonly used domains such as `Calendar`, `Duration`, `Color`, and `Vector`
- constructor introspection helpers such as `constructorName` and `constructorOrdinal`

When you need a broader API, import the specific module directly instead of treating the Prelude as a replacement for the rest of the standard library. For example, see [`aivi.text`](/stdlib/core/text) and [`aivi.logic`](/stdlib/core/logic) for the full text and logic toolboxes.

## Opting out

Most projects should keep the Prelude enabled. If you want a fully explicit environment, opt out with [`@no_prelude`](/syntax/decorators/no_prelude) as described in [Modules: The Prelude](/syntax/modules#106-the-prelude).

```aivi
@no_prelude
module docs.explicit

use aivi (Bool, Int, Result, Text, Ok, Err)

isPositive : Int -> Bool
isPositive = n => n > 0

status : Result Text Int
status = Ok 1
```

Once you opt out, even basic names must be imported explicitly. This is mainly useful for low-level modules, generated code, compiler tests, or teaching material that wants every dependency to be visible. If you only wanted to turn the implicit import off temporarily, you can also re-enable it later with `use aivi.prelude`.

## Why the Prelude exists

- Everyday code should compile without a wall of setup imports.
- The most common names should be available consistently across examples, tutorials, and production modules.
- A clear opt-out path keeps the language convenient without taking away control.

## Constructor introspection

The Prelude also makes constructor introspection available for algebraic data type values. These helpers are most useful for logging, diagnostics, tests, and generic UI tooling; when you are making control-flow decisions, pattern matching is usually clearer.

| Function | Type | Description |
| --- | --- | --- |
| `constructorName value` | `A -> Text` | Returns the constructor tag name, such as `Some`, `Err`, or `Published`. |
| `constructorOrdinal value` | `A -> Int` | Returns the zero-based declaration index of the constructor inside its ADT definition. |

```aivi
Status = Draft | Published | Archived

tag : Text
tag = constructorName Published

position : Int
position = constructorOrdinal Published
```

In this example, `tag` is `"Published"` and `position` is `1` because `Draft` is the first constructor.
