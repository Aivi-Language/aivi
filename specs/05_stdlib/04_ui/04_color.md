# Color Domain

The `Color` domain helps you work with **Colors** the way humans do.

Screens think in Red, Green, and Blue, but people think in **Hue**, **Saturation**, and **Lightness**. This domain lets you mix colors mathematically (e.g., `primary + 10% lightness` for a hover state) without the mud that comes from raw RGB math.

## Overview

```aivi
use aivi.color (Color)

primary = #007bff
// Mathematically correct lightening
lighter = primary + 10lightness
```

## Features

```aivi
Rgb = { r: Int, g: Int, b: Int }  // 0-255
Hsl = { h: Float, s: Float, l: Float }  // h: 0-360, s/l: 0-1
Hex = Text  // "#rrggbb"
```

## Domain Definition

```aivi
domain Color over Rgb = {
  type Delta = Lightness Int | Saturation Int | Hue Int
  
  (+) : Rgb -> Delta -> Rgb
  (+) col (Lightness n) = adjustLightness col n
  (+) col (Saturation n) = adjustSaturation col n
  (+) col (Hue n) = adjustHue col n
  
  (-) : Rgb -> Delta -> Rgb
  (-) col delta = col + (negateDelta delta)
  
  // Delta literals
  1l = Lightness 1
  1s = Saturation 1
  1h = Hue 1
}
```

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

```aivi
use aivi.color

primary = { r: 255, g: 85, b: 0 }  // Orange

lighter = primary + 10l
darker = primary - 20l
muted = primary - 30s
shifted = primary + 30h
```
