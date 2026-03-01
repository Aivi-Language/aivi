# Units Domain

<!-- quick-info: {"kind":"module","name":"aivi.units"} -->
The `Units` domain brings **Dimensional Analysis** to your code, solving the "Mars Climate Orbiter" problem. A bare number like `10` is dangerous is it meters? seconds? kilograms? By attaching physical units to your values, AIVI understands the laws of physics at compile time. It knows that `Meters / Seconds = Speed`, but `Meters + Seconds` is nonsense, catching bugs before they ever run.

<!-- /quick-info -->
<div class="import-badge">use aivi.units<span class="domain-badge">domain</span></div>

## Overview

<<< ../../snippets/from_md/stdlib/core/units/overview.aivi{aivi}

## Supported Dimensions

<<< ../../snippets/from_md/stdlib/core/units/supported_dimensions.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/core/units/domain_definition.aivi{aivi}

## Helper Functions

| Function | Explanation |
| --- | --- |
| **defineUnit** name factor<br><code>Text -> Float -> Unit</code> | Creates a unit with a scale factor relative to the base unit. |
| **convert** quantity target<br><code>Quantity -> Unit -> Quantity</code> | Converts a quantity into the target unit. |
| **sameUnit** a b<br><code>Quantity -> Quantity -> Bool</code> | Returns whether two quantities share the same unit name. |

## Usage Examples

<<< ../../snippets/from_md/stdlib/core/units/usage_examples.aivi{aivi}
