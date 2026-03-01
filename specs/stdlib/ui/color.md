# Color Domain

<!-- quick-info: {"kind":"module","name":"aivi.color"} -->
The `Color` domain helps you work with **Colors** the way humans do.

Screens think in Red, Green, and Blue, but people think in **Hue**, **Saturation**, and **Lightness**. This domain lets you mix colors mathematically (e.g., `primary + 10% lightness` for a hover state) without the mud that comes from raw RGB math.

<!-- /quick-info -->
<div class="import-badge">use aivi.color<span class="domain-badge">domain</span></div>

## Overview

<<< ../../snippets/from_md/stdlib/ui/color/overview.aivi{aivi}

## Features

<<< ../../snippets/from_md/stdlib/ui/color/features.aivi{aivi}

For direct channel edits, prefer record patching instead of deltas (e.g., `color <| { r: 30 }`). Deltas are primarily intended for perceptual adjustments like hue, saturation, and lightness.

## Short Constructor

| Constructor | Type | Equivalent |
| --- | --- | --- |
| `rgb r g b` | `Int -> Int -> Int -> Rgb` | `{ r: r, g: g, b: b }` |

<<< ../../snippets/from_md/stdlib/ui/color/short_constructor.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/ui/color/domain_definition.aivi{aivi}

## Helper Functions

| Function | Explanation |
| --- | --- |
| **adjustLightness** color amount<br><code>Rgb -> Int -> Rgb</code> | Increases or decreases lightness by a percentage. |
| **adjustSaturation** color amount<br><code>Rgb -> Int -> Rgb</code> | Increases or decreases saturation by a percentage. |
| **adjustHue** color degrees<br><code>Rgb -> Int -> Rgb</code> | Rotates hue by degrees. |
| **toRgb** hsl<br><code>Hsl -> Rgb</code> | Converts HSL to RGB. |
| **toHsl** rgb<br><code>Rgb -> Hsl</code> | Converts RGB to HSL. |
| **toHex** rgb<br><code>Rgb -> Hex</code> | Renders RGB as a hex string. |

## Usage Examples

<<< ../../snippets/from_md/stdlib/ui/color/usage_examples.aivi{aivi}
