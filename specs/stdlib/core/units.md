# Units Domain

<!-- quick-info: {"kind":"module","name":"aivi.units"} -->
The `aivi.units` module is a lightweight way to keep a numeric value and its unit together. It helps you make conversions explicit and keep module boundaries readable, but in v0.1 it does **not** provide compile-time dimensional analysis or automatic derived-unit math.

<!-- /quick-info -->
<div class="import-badge">use aivi.units<span class="domain-badge">domain</span></div>

## What this domain is for

`aivi.units` is useful when a bare `Float` would be too ambiguous. You define units such as meters, kilograms, or pixels yourself, then store values as `Quantity` records that carry both the numeric magnitude and the chosen `Unit`.

This is a good fit for sensor readings, simulation inputs, graphics measurements, engineering data, or any API boundary where “what does this number mean?” should be obvious from the value itself.

## Start here

Reach for `aivi.units` when:

- a number crosses a module boundary and should stay self-describing,
- you need explicit conversions such as meters ↔ kilometers,
- the unit is part of the meaning, not just display text.

Reach for a more specialized domain when the standard library already models the concept directly. For example, [`aivi.chronos.duration`](../chronos/duration.md) already provides built-in time literals such as `500ms` and `2s`.

## Import patterns

- `use aivi.units` brings `Unit`, `Quantity`, `defineUnit`, `convert`, and `sameUnit` into scope.
- The exported `Units` domain supplies `+`, `-`, `*`, and `/` for `Quantity`.
- For the general language rules around domains and operator ownership, see [Domains](/syntax/domains) and [Operators](/syntax/operators).

## Overview

<<< ../../snippets/from_md/stdlib/core/units/block_01.aivi{aivi}


In this example, `raceDistanceKm` becomes `1.5 km`, and `sameScale` is `True` because both quantities use the unit name `"m"`.

## Core data shapes

`aivi.units` does not ship a built-in catalog of SI units. Instead, it gives you two small record types so each module can define the units it needs:

<<< ../../snippets/from_md/stdlib/core/units/block_02.aivi{aivi}


`factor` is the scale relative to the base unit you choose for your problem. For example, if `meter` uses factor `1.0`, then `kilometer` can use factor `1000.0`.

## Domain definition

When the `Units` domain is in scope, `Quantity` supports a small arithmetic surface:

| Operator | Type | Runtime behavior |
| --- | --- | --- |
| `a + b` | `Quantity -> Quantity -> Quantity` | Adds the numeric values and keeps `a.unit`. Convert to a common unit first. |
| `a - b` | `Quantity -> Quantity -> Quantity` | Subtracts the numeric values and keeps `a.unit`. Convert to a common unit first. |
| `q * s` | `Quantity -> Float -> Quantity` | Scales the quantity by a plain number. |
| `q / s` | `Quantity -> Float -> Quantity` | Divides the quantity by a plain number. |

<<< ../../snippets/from_md/stdlib/core/units/block_03.aivi{aivi}

::: repl
```aivi
/use aivi.units
distance = 100.0 m
time = 9.58 s
speed = distance / time
// => 10.438... m/s
```
:::

## Helper functions

The usual workflow is: define or pick a unit, do your arithmetic in one consistent system, then convert only at the boundary where you display or import data.

| Function | Explanation |
| --- | --- |
| **defineUnit** name factor<br><code>Text -> Float -> Unit</code> | Creates a named unit with a scale factor relative to your chosen base unit. |
| **convert** quantity target<br><code>Quantity -> Unit -> Quantity</code> | Re-expresses a quantity in `target` by applying the stored scale factors. |
| **sameUnit** a b<br><code>Quantity -> Quantity -> Bool</code> | Checks whether `a.unit.name` and `b.unit.name` are the same text label. |

## Practical guidance

- Use `Quantity` for values that cross module boundaries so the unit stays visible in the data shape.
- Pick one canonical unit per measurement family inside a module, and convert at the edges for display or input.
- Convert to a common unit before addition or subtraction; the domain operators do not silently normalize mismatched units for you.
- Keep unit names stable and meaningful. `sameUnit` compares labels, not conversion factors.

## Limitations in v0.1

- There are no built-in `10m`, `5kg`, or `3s` suffix literals in `aivi.units`; define units explicitly with `defineUnit`.
- The module does not derive composite units such as “meters per second” for you.
- The API helps document and convert units at runtime, but it does not prove physical correctness in the type checker.

## Usage examples

<<< ../../snippets/from_md/stdlib/core/units/block_04.aivi{aivi}


Use this pattern when you want unit names and explicit conversions, but do not need a larger domain such as calendar time or geometry.
