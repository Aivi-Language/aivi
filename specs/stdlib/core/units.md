# Units Domain

<!-- quick-info: {"kind":"module","name":"aivi.units"} -->
The `Units` domain brings **Dimensional Analysis** to your code, solving the "Mars Climate Orbiter" problem. A bare number like `10` is dangerous is it meters? seconds? kilograms? By attaching physical units to your values, AIVI understands the laws of physics at compile time. It knows that `Meters / Seconds = Speed`, but `Meters + Seconds` is nonsense, catching bugs before they ever run.

<!-- /quick-info -->
<div class="import-badge">use aivi.units<span class="domain-badge">domain</span></div>

## Overview

<<< ../../snippets/from_md/05_stdlib/00_core/16_units/block_01.aivi{aivi}

## Supported Dimensions

<<< ../../snippets/from_md/05_stdlib/00_core/16_units/block_02.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/05_stdlib/00_core/16_units/block_03.aivi{aivi}

## Helper Functions

| Function | Explanation |
| --- | --- |
| **defineUnit** name factor<br><pre><code>`Text -> Float -> Unit`</code></pre> | Creates a unit with a scale factor relative to the base unit. |
| **convert** quantity target<br><pre><code>`Quantity -> Unit -> Quantity`</code></pre> | Converts a quantity into the target unit. |
| **sameUnit** a b<br><pre><code>`Quantity -> Quantity -> Bool`</code></pre> | Returns whether two quantities share the same unit name. |

## Usage Examples

<<< ../../snippets/from_md/05_stdlib/00_core/16_units/block_04.aivi{aivi}
