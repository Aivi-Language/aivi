# Math Module

<!-- quick-info: {"kind":"module","name":"aivi.math"} -->
The `aivi.math` module is the standard place for everyday numeric work: constants, rounding, powers, logs, trigonometry, integer helpers, and floating-point checks.
It is intentionally predictable and familiar if you have used math libraries in other languages, while still using AIVI-specific types such as `Angle` and `BigInt` where that improves clarity.
<!-- /quick-info -->
<div class="import-badge">use aivi.math</div>

If you are not sure where a numeric helper belongs, start here.

## Overview

<<< ../../snippets/from_md/stdlib/math/math/overview.aivi{aivi}

## Constants

`inf` is positive infinity and `negInf` is negative infinity. They are IEEE 754 `Float` values, so they apply to `Float` only. `Int` has no infinity value.

<<< ../../snippets/from_md/stdlib/math/math/constants.aivi{aivi}

## Angles

Trigonometric functions work with the dedicated `Angle` type instead of raw `Float` values. That keeps units explicit and makes code such as `sin 90deg` easier to read correctly.

<<< ../../snippets/from_md/stdlib/math/math/angles.aivi{aivi}

| Function | What it does |
| --- | --- |
| **radians** value<br><code>Float -> Angle</code> | Creates an `Angle` from a radians value. |
| **degrees** value<br><code>Float -> Angle</code> | Creates an `Angle` from a degrees value. |
| **toRadians** angle<br><code>Angle -> Float</code> | Extracts the radians value from an `Angle`. |
| **toDegrees** angle<br><code>Angle -> Float</code> | Extracts the degrees value from an `Angle`. |

## Everyday numeric helpers

### Magnitudes and signs

| Function | What it does |
| --- | --- |
| **abs** value<br><code>Int -> Int</code> | Returns the absolute value of `value`. |
| **abs** value<br><code>Float -> Float</code> | Returns the absolute value of `value`. |
| **sign** x<br><code>Float -> Float</code> | Returns `-1.0`, `0.0`, or `1.0` based on the sign of `x`. |
| **copysign** mag sign<br><code>Float -> Float -> Float</code> | Returns `mag` with the sign of `sign`. |

### Bounds and aggregation

| Function | What it does |
| --- | --- |
| **min** a b<br><code>Float -> Float -> Float</code> | Returns the smaller of `a` and `b`. |
| **max** a b<br><code>Float -> Float -> Float</code> | Returns the larger of `a` and `b`. |
| **minAll** values<br><code>List Float -> Option Float</code> | Returns the minimum of `values`, or `None` for an empty list. |
| **maxAll** values<br><code>List Float -> Option Float</code> | Returns the maximum of `values`, or `None` for an empty list. |
| **clamp** low high x<br><code>Float -> Float -> Float -> Float</code> | Restricts `x` to the closed interval `[low, high]`. |
| **sum** values<br><code>List Float -> Float</code> | Sums floating-point values; an empty list yields `0.0`. |
| **sumInt** values<br><code>List Int -> Int</code> | Sums integer values; an empty list yields `0`. |

## Rounding and decomposition

Use these helpers when you need stable, named rounding behavior instead of remembering the edge cases yourself.

| Function | What it does |
| --- | --- |
| **floor** x<br><code>Float -> Float</code> | Rounds toward `-inf`. |
| **ceil** x<br><code>Float -> Float</code> | Rounds toward `+inf`. |
| **trunc** x<br><code>Float -> Float</code> | Rounds toward `0`. |
| **round** x<br><code>Float -> Float</code> | Uses banker's rounding (ties to even). |
| **fract** x<br><code>Float -> Float</code> | Returns the fractional part with the same sign as `x`. |

| Function | What it does |
| --- | --- |
| **modf** x<br><code>Float -> (Float, Float)</code> | Returns `(intPart, fracPart)` such that `x = intPart + fracPart`. |
| **frexp** x<br><code>Float -> (Float, Int)</code> | Returns `(mantissa, exponent)` such that `x = mantissa * 2^exponent`. |
| **ldexp** mantissa exponent<br><code>Float -> Int -> Float</code> | Computes `mantissa * 2^exponent`. |

## Powers, roots, and logarithms

| Function | What it does |
| --- | --- |
| **pow** base exp<br><code>Float -> Float -> Float</code> | Raises `base` to `exp`. |
| **sqrt** x<br><code>Float -> Float</code> | Computes the square root. |
| **cbrt** x<br><code>Float -> Float</code> | Computes the cube root. |
| **hypot** x y<br><code>Float -> Float -> Float</code> | Computes `sqrt(x*x + y*y)` with reduced overflow and underflow risk. |

| Function | What it does |
| --- | --- |
| **exp** x<br><code>Float -> Float</code> | Computes `e^x`. |
| **exp2** x<br><code>Float -> Float</code> | Computes `2^x`. |
| **expm1** x<br><code>Float -> Float</code> | Computes `e^x - 1` with improved precision near zero. |

| Function | What it does |
| --- | --- |
| **log** x<br><code>Float -> Float</code> | Computes the natural logarithm. |
| **log10** x<br><code>Float -> Float</code> | Computes the base-10 logarithm. |
| **log2** x<br><code>Float -> Float</code> | Computes the base-2 logarithm. |
| **log1p** x<br><code>Float -> Float</code> | Computes `log(1 + x)` with improved precision near zero. |

## Trigonometry

The forward trig functions accept `Angle`; the inverse functions return `Angle`. That makes unit conversions explicit at API boundaries.

| Function | What it does |
| --- | --- |
| **sin** angle<br><code>Angle -> Float</code> | Computes the sine ratio for `angle`. |
| **cos** angle<br><code>Angle -> Float</code> | Computes the cosine ratio for `angle`. |
| **tan** angle<br><code>Angle -> Float</code> | Computes the tangent ratio for `angle`. |
| **asin** x<br><code>Float -> Angle</code> | Returns the angle whose sine is `x`. |
| **acos** x<br><code>Float -> Angle</code> | Returns the angle whose cosine is `x`. |
| **atan** x<br><code>Float -> Angle</code> | Returns the angle whose tangent is `x`. |
| **atan2** y x<br><code>Float -> Float -> Angle</code> | Returns the angle of the vector `(x, y)` from the positive x-axis. |

## Hyperbolic functions

| Function | What it does |
| --- | --- |
| **sinh** x<br><code>Float -> Float</code> | Computes hyperbolic sine. |
| **cosh** x<br><code>Float -> Float</code> | Computes hyperbolic cosine. |
| **tanh** x<br><code>Float -> Float</code> | Computes hyperbolic tangent. |
| **asinh** x<br><code>Float -> Float</code> | Computes inverse hyperbolic sine. |
| **acosh** x<br><code>Float -> Float</code> | Computes inverse hyperbolic cosine. |
| **atanh** x<br><code>Float -> Float</code> | Computes inverse hyperbolic tangent. |

## Integer math

These helpers are useful for counting problems, modular arithmetic, and exact integer combinatorics.

| Function | What it does |
| --- | --- |
| **gcd** a b<br><code>Int -> Int -> Int</code> | Computes the greatest common divisor. |
| **lcm** a b<br><code>Int -> Int -> Int</code> | Computes the least common multiple. |
| **gcdAll** values<br><code>List Int -> Option Int</code> | Returns the gcd of all values, or `None` when the list is empty. |
| **lcmAll** values<br><code>List Int -> Option Int</code> | Returns the lcm of all values, or `None` when the list is empty. |
| **factorial** n<br><code>Int -> BigInt</code> | Computes `n!`. |
| **comb** n k<br><code>Int -> Int -> BigInt</code> | Computes combinations (“n choose k”). |
| **perm** n k<br><code>Int -> Int -> BigInt</code> | Computes permutations (“n P k”). |
| **divmod** a b<br><code>Int -> Int -> (Int, Int)</code> | Returns `(q, r)` such that `a = q * b + r` and `0 <= r < |b|`. |
| **modPow** base exp modulus<br><code>Int -> Int -> Int -> Int</code> | Computes `(base^exp) mod modulus`. |

`BigInt` comes from `aivi.number.bigint` and is re-exported by `aivi.math`.

## Floating-point checks

Use these when you are handling user input, numerical edge cases, or low-level algorithms where NaN and infinity matter.

| Function | What it does |
| --- | --- |
| **isFinite** x<br><code>Float -> Bool</code> | Returns whether `x` is finite. |
| **isInf** x<br><code>Float -> Bool</code> | Returns whether `x` is infinite. |
| **isNaN** x<br><code>Float -> Bool</code> | Returns whether `x` is NaN. |
| **nextAfter** from to<br><code>Float -> Float -> Float</code> | Returns the next representable float after `from` toward `to`. |
| **ulp** x<br><code>Float -> Float</code> | Returns the size of one unit in the last place at `x`. |

## Remainders

| Function | What it does |
| --- | --- |
| **fmod** a b<br><code>Float -> Float -> Float</code> | Returns the remainder using truncation toward zero. |
| **remainder** a b<br><code>Float -> Float -> Float</code> | Returns the IEEE-754 remainder using the nearest integer quotient. |

## Usage Examples

<<< ../../snippets/from_md/stdlib/math/math/usage_examples.aivi{aivi}
