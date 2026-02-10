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

## Angles

Angles are represented by a dedicated domain so trigonometric functions are not called with raw `Float` values.

```aivi
Angle = { radians: Float }
```

| Function | Explanation |
| --- | --- |
| **radians** value<br><pre><code>`Float -> Angle`</code></pre> | Creates an `Angle` from a raw radians value. |

| Function | Explanation |
| --- | --- |
| **degrees** value<br><pre><code>`Float -> Angle`</code></pre> | Creates an `Angle` from a raw degrees value. |

| Function | Explanation |
| --- | --- |
| **toRadians** angle<br><pre><code>`Angle -> Float`</code></pre> | Extracts the radians value from an `Angle`. |

| Function | Explanation |
| --- | --- |
| **toDegrees** angle<br><pre><code>`Angle -> Float`</code></pre> | Extracts the degrees value from an `Angle`. |

## Basic helpers

| Function | Explanation |
| --- | --- |
| **abs** value<br><pre><code>`Int -> Int`</code></pre> | Returns the absolute value of `value`. |
| **abs** value<br><pre><code>`Float -> Float`</code></pre> | Returns the absolute value of `value`. |

| Function | Explanation |
| --- | --- |
| **sign** x<br><pre><code>`Float -> Float`</code></pre> | Returns `-1.0`, `0.0`, or `1.0` based on the sign of `x`. |
| **copysign** mag sign<br><pre><code>`Float -> Float -> Float`</code></pre> | Returns `mag` with the sign of `sign`. |

| Function | Explanation |
| --- | --- |
| **min** a b<br><pre><code>`Float -> Float -> Float`</code></pre> | Returns the smaller of `a` and `b`. |
| **max** a b<br><pre><code>`Float -> Float -> Float`</code></pre> | Returns the larger of `a` and `b`. |
| **minAll** values<br><pre><code>`List Float -> Option Float`</code></pre> | Returns the minimum of `values` or `None` when empty. |
| **maxAll** values<br><pre><code>`List Float -> Option Float`</code></pre> | Returns the maximum of `values` or `None` when empty. |

| Function | Explanation |
| --- | --- |
| **clamp** low high x<br><pre><code>`Float -> Float -> Float -> Float`</code></pre> | Limits `x` to the closed interval `[low, high]`. |
| **sum** values<br><pre><code>`List Float -> Float`</code></pre> | Sums values (empty list yields `0.0`). |
| **sumInt** values<br><pre><code>`List Int -> Int`</code></pre> | Sums values (empty list yields `0`). |

## Rounding and decomposition

| Function | Explanation |
| --- | --- |
| **floor** x<br><pre><code>`Float -> Float`</code></pre> | Rounds toward `-inf`. |
| **ceil** x<br><pre><code>`Float -> Float`</code></pre> | Rounds toward `+inf`. |
| **trunc** x<br><pre><code>`Float -> Float`</code></pre> | Rounds toward `0`. |
| **round** x<br><pre><code>`Float -> Float`</code></pre> | Uses banker's rounding (ties to even). |
| **fract** x<br><pre><code>`Float -> Float`</code></pre> | Returns the fractional part with the same sign as `x`. |

| Function | Explanation |
| --- | --- |
| **modf** x<br><pre><code>`Float -> (Float, Float)`</code></pre> | Returns `(intPart, fracPart)` where `x = intPart + fracPart`. |
| **frexp** x<br><pre><code>`Float -> (Float, Int)`</code></pre> | Returns `(mantissa, exponent)` such that `x = mantissa * 2^exponent`. |
| **ldexp** mantissa exponent<br><pre><code>`Float -> Int -> Float`</code></pre> | Computes `mantissa * 2^exponent`. |

## Powers, roots, and logs

| Function | Explanation |
| --- | --- |
| **pow** base exp<br><pre><code>`Float -> Float -> Float`</code></pre> | Raises `base` to `exp`. |
| **sqrt** x<br><pre><code>`Float -> Float`</code></pre> | Computes the square root. |
| **cbrt** x<br><pre><code>`Float -> Float`</code></pre> | Computes the cube root. |
| **hypot** x y<br><pre><code>`Float -> Float -> Float`</code></pre> | Computes `sqrt(x*x + y*y)` with reduced overflow/underflow. |

| Function | Explanation |
| --- | --- |
| **exp** x<br><pre><code>`Float -> Float`</code></pre> | Computes `e^x`. |
| **exp2** x<br><pre><code>`Float -> Float`</code></pre> | Computes `2^x`. |
| **expm1** x<br><pre><code>`Float -> Float`</code></pre> | Computes `e^x - 1` with improved precision near zero. |

| Function | Explanation |
| --- | --- |
| **log** x<br><pre><code>`Float -> Float`</code></pre> | Computes the natural log. |
| **log10** x<br><pre><code>`Float -> Float`</code></pre> | Computes the base-10 log. |
| **log2** x<br><pre><code>`Float -> Float`</code></pre> | Computes the base-2 log. |
| **log1p** x<br><pre><code>`Float -> Float`</code></pre> | Computes `log(1 + x)` with improved precision near zero. |

## Trigonometry

| Function | Explanation |
| --- | --- |
| **sin** angle<br><pre><code>`Angle -> Float`</code></pre> | Computes the sine ratio for `angle`. |
| **cos** angle<br><pre><code>`Angle -> Float`</code></pre> | Computes the cosine ratio for `angle`. |
| **tan** angle<br><pre><code>`Angle -> Float`</code></pre> | Computes the tangent ratio for `angle`. |

| Function | Explanation |
| --- | --- |
| **asin** x<br><pre><code>`Float -> Angle`</code></pre> | Returns the angle whose sine is `x`. |
| **acos** x<br><pre><code>`Float -> Angle`</code></pre> | Returns the angle whose cosine is `x`. |
| **atan** x<br><pre><code>`Float -> Angle`</code></pre> | Returns the angle whose tangent is `x`. |
| **atan2** y x<br><pre><code>`Float -> Float -> Angle`</code></pre> | Returns the angle of the vector `(x, y)` from the positive x-axis. |

## Hyperbolic functions

| Function | Explanation |
| --- | --- |
| **sinh** x<br><pre><code>`Float -> Float`</code></pre> | Computes hyperbolic sine. |
| **cosh** x<br><pre><code>`Float -> Float`</code></pre> | Computes hyperbolic cosine. |
| **tanh** x<br><pre><code>`Float -> Float`</code></pre> | Computes hyperbolic tangent. |
| **asinh** x<br><pre><code>`Float -> Float`</code></pre> | Computes inverse hyperbolic sine. |
| **acosh** x<br><pre><code>`Float -> Float`</code></pre> | Computes inverse hyperbolic cosine. |
| **atanh** x<br><pre><code>`Float -> Float`</code></pre> | Computes inverse hyperbolic tangent. |

## Integer math

| Function | Explanation |
| --- | --- |
| **gcd** a b<br><pre><code>`Int -> Int -> Int`</code></pre> | Computes the greatest common divisor. |
| **lcm** a b<br><pre><code>`Int -> Int -> Int`</code></pre> | Computes the least common multiple. |
| **gcdAll** values<br><pre><code>`List Int -> Option Int`</code></pre> | Returns the gcd of all values or `None` when empty. |
| **lcmAll** values<br><pre><code>`List Int -> Option Int`</code></pre> | Returns the lcm of all values or `None` when empty. |

| Function | Explanation |
| --- | --- |
| **factorial** n<br><pre><code>`Int -> BigInt`</code></pre> | Computes `n!`. |
| **comb** n k<br><pre><code>`Int -> Int -> BigInt`</code></pre> | Computes combinations ("n choose k"). |
| **perm** n k<br><pre><code>`Int -> Int -> BigInt`</code></pre> | Computes permutations ("n P k"). |

| Function | Explanation |
| --- | --- |
| **divmod** | `Int -> Int -> (Int, Int)` | `a`: dividend; `b`: divisor. | Returns `(q, r)` where `a = q * b + r` and `0 <= r < |b|`. |
| **modPow** | `Int -> Int -> Int -> Int` | `base`: base; `exp`: exponent; `modulus`: modulus. | Computes `(base^exp) mod modulus`. |

Notes:
- `BigInt` is from `aivi.number.bigint` and is re-exported by `aivi.math`.

## Floating-point checks

| Function | Explanation |
| --- | --- |
| **isFinite** x<br><pre><code>`Float -> Bool`</code></pre> | Returns whether `x` is finite. |
| **isInf** x<br><pre><code>`Float -> Bool`</code></pre> | Returns whether `x` is infinite. |
| **isNaN** x<br><pre><code>`Float -> Bool`</code></pre> | Returns whether `x` is NaN. |
| **nextAfter** from to<br><pre><code>`Float -> Float -> Float`</code></pre> | Returns the next representable float after `from` toward `to`. |
| **ulp** x<br><pre><code>`Float -> Float`</code></pre> | Returns the size of one unit-in-the-last-place at `x`. |

## Remainders

| Function | Explanation |
| --- | --- |
| **fmod** a b<br><pre><code>`Float -> Float -> Float`</code></pre> | Returns the remainder using truncation toward zero. |
| **remainder** a b<br><pre><code>`Float -> Float -> Float`</code></pre> | Returns the IEEE-754 remainder (round-to-nearest quotient). |

## Usage Examples

```aivi
use aivi.math

angle = degrees 90.0
unit = sin angle

digits = [1.0, 2.0, 3.0] |> sum
```
