# Units Domain

<!-- quick-info: {"kind":"module","name":"aivi.units"} -->
The `Units` domain brings **dimensional analysis** to your code. In plain language, that means numbers carry their physical meaning with them, so `10 meters` and `10 seconds` are not interchangeable by accident. This helps prevent the classic “the numbers looked compatible, but the units were wrong” kind of bug.

<!-- /quick-info -->
<div class="import-badge">use aivi.units<span class="domain-badge">domain</span></div>

## What this domain is for

`aivi.units` lets you work with physical quantities without losing track of what the numbers mean. Instead of passing around unlabelled `Float` values, you can express measurements such as meters, seconds, and kilograms directly in the type system.

This helps when you are writing code for science, engineering, graphics, simulations, sensors, or any other place where mixing units would be a real bug.

## Start here

Reach for `aivi.units` when:

- a number crosses a module boundary and should stay self-describing,
- you are combining measurements such as distance, time, speed, or mass,
- a wrong unit would be a real defect, not just a display issue.

## Overview

<<< ../../snippets/from_md/stdlib/core/units/overview.aivi{aivi}

## Supported dimensions

Start here if you want to scan the built-in measurement families before reading the full domain definition.

<<< ../../snippets/from_md/stdlib/core/units/supported_dimensions.aivi{aivi}

## Domain definition

<<< ../../snippets/from_md/stdlib/core/units/domain_definition.aivi{aivi}

## Helper functions

The usual workflow is: define or pick a unit, do your arithmetic in one consistent system, then convert only at the boundary where you display or import data.

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
