# Numbers

<!-- quick-info: {"kind":"module","name":"aivi.number"} -->
The `aivi.number` family groups numeric domains that sit above `Int` and `Float`:

- `aivi.number.bigint` for arbitrary-precision integers
- `aivi.number.rational` for exact fractions
- `aivi.number.decimal` for fixed-point base-10 arithmetic
- `aivi.number.complex` for complex arithmetic
- `aivi.number.quaternion` for quaternion arithmetic

You can use either the facade module or the specific domain module depending on how much you want in scope.

### Choosing between Decimal and Rational

| | **Decimal** | **Rational** |
| --- | --- | --- |
| **Representation** | Fixed-point base-10 (like `123.45`) | Exact fraction `numerator / denominator` (BigInt-backed) |
| **Use case** | Financial math, currency ($19.99) | Exact math (1/3 stays 1/3, never rounds) |
| **Precision** | Fixed decimal places, rounds at boundaries | Infinite (denominators can grow unboundedly) |
| **Speed** | Faster (bounded representation) | Slower (numerator/denominator may grow large) |
| **Float problem solved** | `0.1 + 0.2 == 0.3` (exact in Decimal) | `1/3 * 3 == 1` (exact in Rational) |

**Rule of thumb**: Use `Decimal` for money and human-facing decimal values. Use `Rational` for symbolic math where any rounding is unacceptable.

<!-- /quick-info -->
<div class="import-badge">use aivi.number</div>

<<< ../../snippets/from_md/stdlib/math/number/choosing_between_decimal_and_rational.aivi{aivi}


## BigInt

`BigInt` is an **opaque native type** for arbitrary-precision integers.

<<< ../../snippets/from_md/stdlib/math/number/bigint_01.aivi{aivi}

<<< ../../snippets/from_md/stdlib/math/number/bigint_02.aivi{aivi}

Helpers:

| Function | Explanation |
| --- | --- |
| **fromInt** value<br><pre><code>`Int -> BigInt`</code></pre> | Converts a machine `Int` into `BigInt`. |
| **toInt** value<br><pre><code>`BigInt -> Option Int`</code></pre> | Converts a `BigInt` to `Int`. Returns `None` if the value overflows the machine `Int` range. |

Example:

<<< ../../snippets/from_md/stdlib/math/number/bigint_03.aivi{aivi}

## Decimal

`Decimal` is an **opaque native type** for fixed-point arithmetic (base-10), suitable for financial calculations where `Float` precision errors are unacceptable.

<<< ../../snippets/from_md/stdlib/math/number/decimal_01.aivi{aivi}

<<< ../../snippets/from_md/stdlib/math/number/decimal_02.aivi{aivi}

Helpers:

| Function | Explanation |
| --- | --- |
| **fromFloat** value<br><pre><code>`Float -> Decimal`</code></pre> | Converts a `Float` into `Decimal` using base-10 rounding rules. |
| **toFloat** value<br><pre><code>`Decimal -> Float`</code></pre> | Converts a `Decimal` into a `Float`. |
| **round** value places<br><pre><code>`Decimal -> Int -> Decimal`</code></pre> | Rounds to `places` decimal digits. |

Example:

<<< ../../snippets/from_md/stdlib/math/number/decimal_03.aivi{aivi}

## Rational

`Rational` is an **opaque native type** for exact fractions (`num/den`).

<<< ../../snippets/from_md/stdlib/math/number/rational_01.aivi{aivi}

<<< ../../snippets/from_md/stdlib/math/number/rational_02.aivi{aivi}

Helpers:

| Function | Explanation |
| --- | --- |
| **normalize** r<br><pre><code>`Rational -> Rational`</code></pre> | Reduces a fraction to lowest terms. |
| **numerator** r<br><pre><code>`Rational -> BigInt`</code></pre> | Returns the numerator. |
| **denominator** r<br><pre><code>`Rational -> BigInt`</code></pre> | Returns the denominator. |

Example:

<<< ../../snippets/from_md/stdlib/math/number/rational_03.aivi{aivi}

## Complex

`Complex` represents values of the form `a + bi`. It is typically a struct of two floats, but domain operations are backed by optimized native implementations.

<<< ../../snippets/from_md/stdlib/math/number/complex_01.aivi{aivi}

<<< ../../snippets/from_md/stdlib/math/number/complex_02.aivi{aivi}

Example:

<<< ../../snippets/from_md/stdlib/math/number/complex_03.aivi{aivi}

## Quaternion

The `Quaternion` domain provides tools for handling **3D rotations** without gimbal lock.

<<< ../../snippets/from_md/stdlib/math/number/quaternion_01.aivi{aivi}

<<< ../../snippets/from_md/stdlib/math/number/quaternion_02.aivi{aivi}

<<< ../../snippets/from_md/stdlib/math/number/quaternion_03.aivi{aivi}

| Function | Explanation |
| --- | --- |
| **fromAxisAngle** axis theta<br><pre><code>`{ x: Float, y: Float, z: Float } -> Float -> Quaternion`</code></pre> | Creates a rotation from axis/angle. |
| **conjugate** q<br><pre><code>`Quaternion -> Quaternion`</code></pre> | Negates the vector part. |
| **magnitude** q<br><pre><code>`Quaternion -> Float`</code></pre> | Returns the quaternion length. |
| **normalize** q<br><pre><code>`Quaternion -> Quaternion`</code></pre> | Returns a unit-length quaternion. |

<<< ../../snippets/from_md/stdlib/math/number/quaternion_04.aivi{aivi}
