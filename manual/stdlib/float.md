# aivi.core.float

Pure helpers and compiler intrinsics for finite IEEE 754 double-precision values. The MVP keeps
general numeric operations here; presentation rounding, percentages, angle wrapping, and curve
evaluation belong in application libraries.

```aivi
use aivi.core.float (
    pi
    e
    tau
    negate
    max
    min
    clamp
    lerp
    sign
    between
    isZero
    isPositive
    isNegative
    square
    toRadians
    toDegrees
    floor
    ceil
    round
    sqrt
    abs
    toInt
    fromInt
    toText
    parseText
    approxEq
)
```

## Constants

| Value | Meaning |
| --- | --- |
| `pi` | π |
| `e` | Euler's number |
| `tau` | 2π |

## Pure helpers

| Function | Type | Behavior |
| --- | --- | --- |
| `negate` | `Float -> Float` | Flip the sign |
| `max` | `Float -> Float -> Float` | Return the greater value |
| `min` | `Float -> Float -> Float` | Return the lesser value |
| `clamp` | `Float -> Float -> Float -> Float` | Restrict a value to inclusive bounds |
| `lerp` | `Float -> Float -> Float -> Float` | Linear interpolation |
| `sign` | `Float -> Float` | Return `-1.0`, `0.0`, or `1.0` |
| `between` | `Float -> Float -> Float -> Bool` | Inclusive range test |
| `isZero` | `Float -> Bool` | Test against `0.0` |
| `isPositive` | `Float -> Bool` | Test above zero |
| `isNegative` | `Float -> Bool` | Test below zero |
| `square` | `Float -> Float` | Multiply a value by itself |
| `toRadians` | `Float -> Float` | Convert degrees to radians |
| `toDegrees` | `Float -> Float` | Convert radians to degrees |
| `approxEq` | `Float -> Float -> Float -> Bool` | `approxEq epsilon a b` tests `abs (a - b) <= epsilon` |

## Compiler intrinsics

| Function | Type | Behavior |
| --- | --- | --- |
| `floor` | `Float -> Float` | Round down |
| `ceil` | `Float -> Float` | Round up |
| `round` | `Float -> Float` | Round to the nearest integral value |
| `sqrt` | `Float -> Float` | Square root |
| `abs` | `Float -> Float` | Absolute value |
| `toInt` | `Float -> Int` | Truncate toward zero |
| `fromInt` | `Int -> Float` | Convert an integer |
| `toText` | `Float -> Text` | Render decimal text |
| `parseText` | `Text -> Option Float` | Parse finite decimal text |

```aivi
use aivi.core.float (
    approxEq
    lerp
    pi
    toRadians
)

value midpoint : Float = lerp 10.0 20.0 0.5
value halfTurn : Bool = approxEq 0.000001 (toRadians 180.0) pi
```
