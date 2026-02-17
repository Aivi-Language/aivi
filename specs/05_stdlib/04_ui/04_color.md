# Color Domain

<!-- quick-info: {"kind":"module","name":"aivi.color"} -->
The `Color` domain helps you work with **Colors** the way humans do.

Screens think in Red, Green, and Blue, but people think in **Hue**, **Saturation**, and **Lightness**. This domain lets you mix colors mathematically (e.g., `primary + 10% lightness` for a hover state) without the mud that comes from raw RGB math.

<!-- /quick-info -->
## Overview

<<< ../../snippets/from_md/05_stdlib/04_ui/04_color/block_01.aivi{aivi}

## Features

<<< ../../snippets/from_md/05_stdlib/04_ui/04_color/block_02.aivi{aivi}

For direct channel edits, prefer record patching instead of deltas (e.g., `color <| { r: 30 }`). Deltas are primarily intended for perceptual adjustments like hue, saturation, and lightness.

## Domain Definition

<<< ../../snippets/from_md/05_stdlib/04_ui/04_color/block_03.aivi{aivi}

## Helper Functions

| Function | Explanation |
| --- | --- |
| **adjustLightness** color amount<br><pre><code>`Rgb -> Int -> Rgb`</code></pre> | Increases or decreases lightness by a percentage. |
| **adjustSaturation** color amount<br><pre><code>`Rgb -> Int -> Rgb`</code></pre> | Increases or decreases saturation by a percentage. |
| **adjustHue** color degrees<br><pre><code>`Rgb -> Int -> Rgb`</code></pre> | Rotates hue by degrees. |
| **toRgb** hsl<br><pre><code>`Hsl -> Rgb`</code></pre> | Converts HSL to RGB. |
| **toHsl** rgb<br><pre><code>`Rgb -> Hsl`</code></pre> | Converts RGB to HSL. |
| **toHex** rgb<br><pre><code>`Rgb -> Hex`</code></pre> | Renders RGB as a hex string. |

## Usage Examples

<<< ../../snippets/from_md/05_stdlib/04_ui/04_color/block_04.aivi{aivi}
