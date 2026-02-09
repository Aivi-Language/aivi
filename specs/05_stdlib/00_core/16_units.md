# Units Domain

The `Units` domain adds **Dimensional Analysis** to your code.

## What is this?

A common number like `10` is ambiguous. Is it 10 meters? 10 seconds? 10 kilograms?
In standard programming, we often confuse them, leading to bugs (e.g., the Mars Climate Orbiter crash caused by mixing metric and imperial units).

This domain lets you attach "tags" to numbers. It understands that `Meters / Seconds = Speed`, but `Meters + Seconds` is impossible nonsense.

## Why this exists

To bake physics and logic rules into the type system. If you try to assign a Time value to a Distance variable, AIVI will stop you *before* the code even runs.

## Overview

```aivi
import aivi.std.core.units use { Length, Time, Velocity }

// Define values with units attached
let distance = 100.0`m`
let time = 9.58`s`

// The compiler knows (Length / Time) results in Velocity
let speed: Velocity = distance / time 
// speed is now roughly 10.43 (m/s)
```

## Supported Dimensions

```aivi
Unit = { name: Text, factor: Float }
Quantity = { value: Float, unit: Unit }
```

## Domain Definition

```aivi
domain Units over Quantity = {
  (+) : Quantity -> Quantity -> Quantity
  (+) a b = { value: a.value + b.value, unit: a.unit }
  
  (-) : Quantity -> Quantity -> Quantity
  (-) a b = { value: a.value - b.value, unit: a.unit }
  
  (*) : Quantity -> Float -> Quantity
  (*) q s = { value: q.value * s, unit: q.unit }
  
  (/) : Quantity -> Float -> Quantity
  (/) q s = { value: q.value / s, unit: q.unit }
}
```

## Helper Functions

```aivi
defineUnit : Text -> Float -> Unit
defineUnit name factor = { name: name, factor: factor }

convert : Quantity -> Unit -> Quantity
convert q target = { value: q.value * (q.unit.factor / target.factor), unit: target }

sameUnit : Quantity -> Quantity -> Bool
sameUnit a b = a.unit.name == b.unit.name
```

## Usage Examples

```aivi
use aivi.std.units

meter = defineUnit "m" 1.0
kilometer = defineUnit "km" 1000.0

distance = { value: 1500.0, unit: meter }
distanceKm = convert distance kilometer
```