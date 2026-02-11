# Layout Domain

The `Layout` domain provides type-safe units for UI dimensions.

This prevents mixing up "10 pixels" with "10 percent" or "10 apples".

## Overview

```aivi
use aivi.ui.layout (Length, Percentage)

// Typed literals
width = 100px
height = 50%

// Invalid:
// width + height -> Error: Cannot add Length and Percentage directly
```

## Features

```aivi
// Underlying representation
UnitVal = { val: Float }
```

## Domain Definition

```aivi
domain Layout over UnitVal = {
    // Length (pixels)
    type Length = Px Float

    // Percentage (0.0 - 1.0 or 0 - 100)
    type Percentage = Pct Float
    
    // Literals
    1px = Px 1.0
    1% = Pct 1.0
    
    // Arithmetic within same unit type
    (+) : Length -> Length -> Length
    (+) (Px a) (Px b) = Px (a + b)
    
    (-) : Length -> Length -> Length
    (-) (Px a) (Px b) = Px (a - b)
}
```
