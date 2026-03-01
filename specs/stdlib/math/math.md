# Math Module

<!-- quick-info: {"kind":"module","name":"aivi.math"} -->
The `aivi.math` module provides standard numeric functions and constants for `Int` and `Float`.
It is intentionally small, predictable, and aligned with common math libraries across languages.

<!-- /quick-info -->
<div class="import-badge">use aivi.math</div>

## Overview

<<< ../../snippets/from_md/stdlib/math/math/overview.aivi{aivi}

## Constants

`inf` is positive infinity and `negInf` is negative infinity. These are IEEE 754 floating-point values   they apply to `Float` only (`Int` has no infinity concept; use `BigInt` for arbitrary precision).

<<< ../../snippets/from_md/stdlib/math/math/constants.aivi{aivi}

## Angles

Angles are represented by a dedicated domain so trigonometric functions are not called with raw `Float` values. Use the domain suffix literals `20deg` and `1.2rad` for concise angle construction, or the constructor functions `degrees` and `radians`.

<<< ../../snippets/from_md/stdlib/math/math/angles.aivi{aivi}

| Function | Explanation |
| --- | --- |
| **radians** value<br><code>Float -> Angle</code> | Creates an `Angle` from a raw radians value. |
| **degrees** value<br><code>Float -> Angle</code> | Creates an `Angle` from a raw degrees value. |
| **toRadians** angle<br><code>Angle -> Float</code> | Extracts the radians value from an `Angle`. |
| **toDegrees** angle<br><code>Angle -> Float</code> | Extracts the degrees value from an `Angle`. |

## Basic helpers

| Function | Explanation |
| --- | --- |
| **abs** value<br><code>Int -> Int</code> | Returns the absolute value of `value`. |
| **abs** value<br><code>Float -> Float</code> | Returns the absolute value of `value`. |

| Function | Explanation |
| --- | --- |
| **sign** x<br><code>Float -> Float</code> | Returns `-1.0`, `0.0`, or `1.0` based on the sign of `x`. |
| **copysign** mag sign<br><code>Float -> Float -> Float</code> | Returns `mag` with the sign of `sign`. |

| Function | Explanation |
| --- | --- |
| **min** a b<br><code>Float -> Float -> Float</code> | Returns the smaller of `a` and `b`. |
| **max** a b<br><code>Float -> Float -> Float</code> | Returns the larger of `a` and `b`. |
| **minAll** values<br><code>List Float -> Option Float</code> | Returns the minimum of `values` or `None` when empty. |
| **maxAll** values<br><code>List Float -> Option Float</code> | Returns the maximum of `values` or `None` when empty. |

| Function | Explanation |
| --- | --- |
| **clamp** low high x<br><code>Float -> Float -> Float -> Float</code> | Limits `x` to the closed interval `[low, high]`. |
| **sum** values<br><code>List Float -> Float</code> | Sums values (empty list yields `0.0`). |
| **sumInt** values<br><code>List Int -> Int</code> | Sums values (empty list yields `0`). |

## Rounding and decomposition

| Function | Explanation |
| --- | --- |
| **floor** x<br><code>Float -> Float</code> | Rounds toward `-inf`. |
| **ceil** x<br><code>Float -> Float</code> | Rounds toward `+inf`. |
| **trunc** x<br><code>Float -> Float</code> | Rounds toward `0`. |
| **round** x<br><code>Float -> Float</code> | Uses banker's rounding (ties to even). |
| **fract** x<br><code>Float -> Float</code> | Returns the fractional part with the same sign as `x`. |

| Function | Explanation |
| --- | --- |
| **modf** x<br><code>Float -> (Float, Float)</code> | Returns `(intPart, fracPart)` where `x = intPart + fracPart`. |
| **frexp** x<br><code>Float -> (Float, Int)</code> | Returns `(mantissa, exponent)` such that `x = mantissa * 2^exponent`. |
| **ldexp** mantissa exponent<br><code>Float -> Int -> Float</code> | Computes `mantissa * 2^exponent`. |

## Powers, roots, and logs

| Function | Explanation |
| --- | --- |
| **pow** base exp<br><code>Float -> Float -> Float</code> | Raises `base` to `exp`. |
| **sqrt** x<br><code>Float -> Float</code> | Computes the square root. |
| **cbrt** x<br><code>Float -> Float</code> | Computes the cube root. |
| **hypot** x y<br><code>Float -> Float -> Float</code> | Computes `sqrt(x*x + y*y)` with reduced overflow/underflow. |

| Function | Explanation |
| --- | --- |
| **exp** x<br><code>Float -> Float</code> | Computes `e^x`. |
| **exp2** x<br><code>Float -> Float</code> | Computes `2^x`. |
| **expm1** x<br><code>Float -> Float</code> | Computes `e^x - 1` with improved precision near zero. |

| Function | Explanation |
| --- | --- |
| **log** x<br><code>Float -> Float</code> | Computes the natural log. |
| **log10** x<br><code>Float -> Float</code> | Computes the base-10 log. |
| **log2** x<br><code>Float -> Float</code> | Computes the base-2 log. |
| **log1p** x<br><code>Float -> Float</code> | Computes `log(1 + x)` with improved precision near zero. |

## Trigonometry

| Function | Explanation |
| --- | --- |
| **sin** angle<br><code>Angle -> Float</code> | Computes the sine ratio for `angle`. |
| **cos** angle<br><code>Angle -> Float</code> | Computes the cosine ratio for `angle`. |
| **tan** angle<br><code>Angle -> Float</code> | Computes the tangent ratio for `angle`. |

| Function | Explanation |
| --- | --- |
| **asin** x<br><code>Float -> Angle</code> | Returns the angle whose sine is `x`. |
| **acos** x<br><code>Float -> Angle</code> | Returns the angle whose cosine is `x`. |
| **atan** x<br><code>Float -> Angle</code> | Returns the angle whose tangent is `x`. |
| **atan2** y x<br><code>Float -> Float -> Angle</code> | Returns the angle of the vector `(x, y)` from the positive x-axis. |

## Hyperbolic functions

| Function | Explanation |
| --- | --- |
| **sinh** x<br><code>Float -> Float</code> | Computes hyperbolic sine. |
| **cosh** x<br><code>Float -> Float</code> | Computes hyperbolic cosine. |
| **tanh** x<br><code>Float -> Float</code> | Computes hyperbolic tangent. |
| **asinh** x<br><code>Float -> Float</code> | Computes inverse hyperbolic sine. |
| **acosh** x<br><code>Float -> Float</code> | Computes inverse hyperbolic cosine. |
| **atanh** x<br><code>Float -> Float</code> | Computes inverse hyperbolic tangent. |

## Integer math

| Function | Explanation |
| --- | --- |
| **gcd** a b<br><code>Int -> Int -> Int</code> | Computes the greatest common divisor. |
| **lcm** a b<br><code>Int -> Int -> Int</code> | Computes the least common multiple. |
| **gcdAll** values<br><code>List Int -> Option Int</code> | Returns the gcd of all values or `None` when empty. |
| **lcmAll** values<br><code>List Int -> Option Int</code> | Returns the lcm of all values or `None` when empty. |

| Function | Explanation |
| --- | --- |
| **factorial** n<br><code>Int -> BigInt</code> | Computes `n!`. |
| **comb** n k<br><code>Int -> Int -> BigInt</code> | Computes combinations ("n choose k"). |
| **perm** n k<br><code>Int -> Int -> BigInt</code> | Computes permutations ("n P k"). |

| Function | Explanation |
| --- | --- |
| **divmod** a b<br><code>Int -> Int -> (Int, Int)</code> | Returns `(q, r)` where `a = q * b + r` and `0 <= r < \|b\|`. |
| **modPow** base exp modulus<br><code>Int -> Int -> Int -> Int</code> | Computes `(base^exp) mod modulus`. |

Notes:
- `BigInt` is from `aivi.number.bigint` and is re-exported by `aivi.math`.

## Floating-point checks

| Function | Explanation |
| --- | --- |
| **isFinite** x<br><code>Float -> Bool</code> | Returns whether `x` is finite. |
| **isInf** x<br><code>Float -> Bool</code> | Returns whether `x` is infinite. |
| **isNaN** x<br><code>Float -> Bool</code> | Returns whether `x` is NaN. |
| **nextAfter** from to<br><code>Float -> Float -> Float</code> | Returns the next representable float after `from` toward `to`. |
| **ulp** x<br><code>Float -> Float</code> | Returns the size of one unit-in-the-last-place at `x`. |

## Remainders

| Function | Explanation |
| --- | --- |
| **fmod** a b<br><code>Float -> Float -> Float</code> | Returns the remainder using truncation toward zero. |
| **remainder** a b<br><code>Float -> Float -> Float</code> | Returns the IEEE-754 remainder (round-to-nearest quotient). |

## Usage Examples

<<< ../../snippets/from_md/stdlib/math/math/usage_examples.aivi{aivi}
