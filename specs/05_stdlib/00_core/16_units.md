# Units Domain

The `Units` domain brings **Dimensional Analysis** to your code, solving the "Mars Climate Orbiter" problem. A bare number like `10` is dangerous—is it meters? seconds? kilograms? By attaching physical units to your values, AIVI understands the laws of physics at compile time. It knows that `Meters / Seconds = Speed`, but `Meters + Seconds` is nonsense, catching bugs before they ever run.

## Overview

```aivi
use aivi.units (Length, Time, Velocity)

// Define values with units attached
distance = 100.0m
time = 9.58s

// The compiler knows (Length / Time) results in Velocity
speed = distance / time 
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

| Function | Explanation |
| --- | --- |
| **defineUnit** name factor<br><pre><code>`Text -> Float -> Unit`</code></pre> | Creates a unit with a scale factor relative to the base unit. |
| **convert** quantity target<br><pre><code>`Quantity -> Unit -> Quantity`</code></pre> | Converts a quantity into the target unit. |
| **sameUnit** a b<br><pre><code>`Quantity -> Quantity -> Bool`</code></pre> | Returns whether two quantities share the same unit name. |

## Usage Examples

```aivi
use aivi.units

meter = defineUnit "m" 1.0
kilometer = defineUnit "km" 1000.0

distance = { value: 1500.0, unit: meter }
distanceKm = convert distance kilometer
```
