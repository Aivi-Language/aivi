# Units Domain

<!-- quick-info: {"kind":"module","name":"aivi.units"} -->
The `Units` domain brings **Dimensional Analysis** to your code, solving the "Mars Climate Orbiter" problem. A bare number like `10` is dangerous is it meters? seconds? kilograms? By attaching physical units to your values, AIVI understands the laws of physics at compile time. It knows that `Meters / Seconds = Speed`, but `Meters + Seconds` is nonsense, catching bugs before they ever run.

<!-- /quick-info -->
<div class="import-badge">use aivi.units<span class="domain-badge">domain</span></div>

## What this domain is for

`aivi.units` lets you work with physical quantities without losing track of what the numbers mean. Instead of passing around unlabelled `Float` values, you can express measurements such as meters, seconds, and kilograms directly in the type system.

This helps when you are writing code for science, engineering, graphics, simulations, sensors, or any other place where mixing units would be a real bug.

## Overview

<<< ../../snippets/from_md/stdlib/core/units/overview.aivi{aivi}

## Supported dimensions

<<< ../../snippets/from_md/stdlib/core/units/supported_dimensions.aivi{aivi}

## Domain definition

<<< ../../snippets/from_md/stdlib/core/units/domain_definition.aivi{aivi}

## Helper functions

| Function | Explanation |
| --- | --- |
| **defineUnit** name factor<br><code>Text -> Float -> Unit</code> | Defines a unit relative to a base unit using a scale factor. |
| **convert** quantity target<br><code>Quantity -> Unit -> Quantity</code> | Converts a quantity into another unit of the same dimension. |
| **sameUnit** a b<br><code>Quantity -> Quantity -> Bool</code> | Checks whether two quantities use the same unit name. |

## Practical guidance

- Use units on values that cross module boundaries so their meaning stays obvious.
- Prefer conversions at the edges of your program, such as I/O or display formatting, instead of constantly changing units internally.
- `sameUnit` checks the unit label, while the type-level dimension rules protect the deeper physical correctness.

## Usage examples

<<< ../../snippets/from_md/stdlib/core/units/usage_examples.aivi{aivi}
