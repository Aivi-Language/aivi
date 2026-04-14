# aivi.color

A small domain type for UI colors, with interpolation helpers and the full GNOME Adwaita palette.

`Color` wraps an `Int`, so you can pass colors around as a named type instead of a raw
number.

## Import

```aivi
use aivi.color (Color)

use aivi.color (
    Color
    blend
)
```

## Overview

| Name | Type | Description |
|------|------|-------------|
| `Color` | domain over `Int` | A packed ARGB color value |
| `blend` | `Color -> Color -> Float -> Color` | Linear interpolation between two colors |
| `black` | `Color` | Fully opaque black `#000000` |
| `white` | `Color` | Fully opaque white `#FFFFFF` |
| `transparent` | `Color` | Fully transparent black |
| `gnomeBlue1..5` | `Color` | GNOME palette blue shades |
| `gnomeGreen1..5` | `Color` | GNOME palette green shades |
| `gnomeYellow1..5` | `Color` | GNOME palette yellow shades |
| `gnomeOrange1..5` | `Color` | GNOME palette orange shades |
| `gnomeRed1..5` | `Color` | GNOME palette red shades |
| `gnomePurple1..5` | `Color` | GNOME palette purple shades |
| `gnomeBrown1..5` | `Color` | GNOME palette brown shades |

## Using palette constants

The recommended way to create colors is to use the pre-built GNOME Adwaita palette
constants or `blend` them:

```aivi
use aivi.color (
    Color
    blend
    gnomeBlue3
    gnomeRed3
)

value accent : Color = gnomeBlue3
value danger : Color = gnomeRed3
value mixed : Color = blend gnomeBlue3 gnomeRed3 0.5
```

## Blending

`blend` linearly interpolates two colors channel by channel. The third argument is a
`Float` in `0.0..1.0` where `0.0` returns the first color and `1.0` returns the second.

```aivi
use aivi.color (
    Color
    blend
    gnomeBlue3
    gnomePurple3
)

value progress : Float = 0.5
value progressColor : Color = blend gnomeBlue3 gnomePurple3 progress
```

## GNOME palette constants

The full five-shade GNOME Adwaita palette is available as constants. Shade `3` is the
canonical mid-point used in GNOME HIG.

```aivi
use aivi.color (
    gnomeBlue3
    gnomeRed3
    gnomeGreen3
)
```

| Constant | Hex value | Swatch |
|----------|-----------|--------|
| `gnomeBlue3` | `#3584e4` | Blue (canonical accent) |
| `gnomeGreen3` | `#33d17a` | Green (success) |
| `gnomeYellow3` | `#f6d32d` | Yellow (warning) |
| `gnomeOrange3` | `#ff7800` | Orange |
| `gnomeRed3` | `#e01b24` | Red (destructive) |
| `gnomePurple3` | `#9141ac` | Purple |
| `gnomeBrown3` | `#986a44` | Brown |

## Notes

Current limits:

- component access (`red`, `green`, `blue`, `alpha`) is available internally in `color.aivi`
  but not importable; use the palette constants or `blend` for most UI needs
- no lightness / hue / saturation algebra
- no alternate color-space helpers such as HSL or OKLCH
- no `fromHex` text parser such as `#RRGGBB` (planned)
