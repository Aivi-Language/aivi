# Math Module

The `aivi.math` module provides standard numeric functions and constants for `Int` and `Float`.
It is intentionally small, predictable, and aligned with common math libraries across languages.

## Overview

```aivi
use aivi.math

area = pi * r * r
clamped = clamp 0.0 1.0 x
```

## Constants

```aivi
pi : Float
tau : Float
e : Float
inf : Float
nan : Float
phi : Float
sqrt2 : Float
ln2 : Float
ln10 : Float
```

## Basic helpers

```aivi
abs : Int -> Int
abs : Float -> Float

sign : Float -> Float
copysign : Float -> Float -> Float

min : Float -> Float -> Float
max : Float -> Float -> Float
minAll : List Float -> Option Float
maxAll : List Float -> Option Float

clamp : Float -> Float -> Float -> Float
sum : List Float -> Float
sumInt : List Int -> Int
```

## Rounding and decomposition

```aivi
floor : Float -> Float
ceil : Float -> Float
trunc : Float -> Float
round : Float -> Float
fract : Float -> Float

modf : Float -> (Float, Float)
frexp : Float -> (Float, Int)
ldexp : Float -> Int -> Float
```

Notes:
- `round` uses banker's rounding (ties to even).
- `modf x` returns `(intPart, fracPart)` where `x = intPart + fracPart`.
- `frexp x` returns `(mantissa, exponent)` such that `x = mantissa * 2^exponent`.

## Powers, roots, and logs

```aivi
pow : Float -> Float -> Float
sqrt : Float -> Float
cbrt : Float -> Float
hypot : Float -> Float -> Float

exp : Float -> Float
exp2 : Float -> Float
expm1 : Float -> Float

log : Float -> Float
log10 : Float -> Float
log2 : Float -> Float
log1p : Float -> Float
```

## Trigonometry

```aivi
sin : Float -> Float
cos : Float -> Float
tan : Float -> Float
asin : Float -> Float
acos : Float -> Float
atan : Float -> Float
atan2 : Float -> Float -> Float

deg2rad : Float -> Float
rad2deg : Float -> Float
```

## Hyperbolic functions

```aivi
sinh : Float -> Float
cosh : Float -> Float
tanh : Float -> Float
asinh : Float -> Float
acosh : Float -> Float
atanh : Float -> Float
```

## Integer math

```aivi
gcd : Int -> Int -> Int
lcm : Int -> Int -> Int
gcdAll : List Int -> Option Int
lcmAll : List Int -> Option Int

factorial : Int -> BigInt
comb : Int -> Int -> BigInt
perm : Int -> Int -> BigInt

divmod : Int -> Int -> (Int, Int)
modPow : Int -> Int -> Int -> Int
```

Notes:
- `BigInt` is from `aivi.number.bigint` and is re-exported by `aivi.math`.

## Floating-point checks

```aivi
isFinite : Float -> Bool
isInf : Float -> Bool
isNaN : Float -> Bool
nextAfter : Float -> Float -> Float
ulp : Float -> Float
```

## Remainders

```aivi
fmod : Float -> Float -> Float
remainder : Float -> Float -> Float
```

Notes:
- `fmod` uses truncation toward zero for the quotient.
- `remainder` uses IEEE-754 remainder (round-to-nearest quotient).

## Usage Examples

```aivi
use aivi.math

angle = 90.0 |> deg2rad
unit = sin angle

digits = [1.0, 2.0, 3.0] |> sum
```
