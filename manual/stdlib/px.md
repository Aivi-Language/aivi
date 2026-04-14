# aivi.px

A domain type for pixel dimensions. Wraps `Int` to give pixel values a distinct type
identity so they cannot be accidentally mixed with unrelated integers.

## Import

```aivi
use aivi.px (
    Px
    px
)
```

## Overview

| Name | Type | Description |
|------|------|-------------|
| `Px` | domain over `Int` | A pixel dimension |
| `px` | `Int -> Px` | Construct a `Px` value |
| `zero` | `Px` | Zero pixels |
| `addPx` | `Px -> Px -> Px` | Add two pixel values |
| `subPx` | `Px -> Px -> Px` | Subtract pixel values |
| `scalePx` | `Px -> Float -> Px` | Scale a pixel value by a float factor |
| `maxPx` | `Px -> Px -> Px` | Larger of two pixel values |
| `minPx` | `Px -> Px -> Px` | Smaller of two pixel values |

## Usage

Construct `Px` values with `px n` and pass them into widget attributes that accept `Int`
by using the GTK bridge's carrier-unwrapping (widget attributes accept `Px` values
directly — the bridge handles unwrapping automatically).

```aivi
use aivi.px (
    Px
    px
    addPx
)

type Layout = {
    iconSize: Px,
    padding: Px
}

value layout : Layout = {
    iconSize: px 48,
    padding: px 12
}
```

## Arithmetic

```aivi
use aivi.px (
    Px
    px
    addPx
    subPx
    scalePx
    maxPx
)

value base : Px = px 8
value larger : Px = addPx base base
value smaller : Px = subPx base (px 2)
value half : Px = scalePx base 0.5
value capped : Px = maxPx base (px 16)
```
