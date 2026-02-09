# Standard Library: Units Domain

## Module

```aivi
module aivi.std.units = {
  export domain Units
  export Unit, Quantity
  export defineUnit, convert, sameUnit
}
```

## Types

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