# Numbers

<!-- quick-info: {"kind":"module","name":"aivi.number"} -->
The `aivi.number` family groups numeric types that go beyond ordinary machine `Int` and `Float` values.
Use it when you need exact fractions, arbitrary-size integers, decimal arithmetic for money, or specialised representations such as complex numbers and quaternions.
<!-- /quick-info -->
<div class="import-badge">use aivi.number</div>

The facade module brings the numeric family into one place:

- `aivi.number.bigint` for arbitrary-precision integers
- `aivi.number.rational` for exact fractions
- `aivi.number.decimal` for fixed-point base-10 arithmetic
- `aivi.number.complex` for complex arithmetic
- `aivi.number.quaternion` for quaternion arithmetic

Import the facade when you want several of these at once, or import a specific module when you want a tighter namespace.

## Choosing between `Decimal` and `Rational`

Both types avoid the usual “floating-point surprise” problems, but they are optimized for different kinds of work.

| | **Decimal** | **Rational** |
| --- | --- | --- |
| **Representation** | Fixed-point base-10 (like `123.45`) | Exact fraction `numerator / denominator` (BigInt-backed) |
| **Best for** | Financial math, user-facing decimal values | Exact symbolic or algebraic math |
| **Precision model** | Fixed decimal places, rounds at boundaries | Exact until you explicitly convert or round |
| **Performance trade-off** | Faster and bounded | Slower because numerators and denominators can grow |
| **Typical win** | `0.1 + 0.2 == 0.3` | `1/3 * 3 == 1` |

Rule of thumb: use `Decimal` for money and business values, and `Rational` for calculations where any rounding would be misleading.

<<< ../../snippets/from_md/stdlib/math/number/choosing_between_decimal_and_rational.aivi{aivi}

## BigInt

`BigInt` is an opaque native type for integers that are larger than machine `Int` can represent.

<<< ../../snippets/from_md/stdlib/math/number/bigint_01.aivi{aivi}

<<< ../../snippets/from_md/stdlib/math/number/bigint_02.aivi{aivi}

| Function | What it does |
| --- | --- |
| **fromInt** value<br><code>Int -> BigInt</code> | Converts a machine `Int` into `BigInt`. |
| **toInt** value<br><code>BigInt -> Option Int</code> | Converts a `BigInt` back to `Int`, returning `None` if it would overflow. |

<<< ../../snippets/from_md/stdlib/math/number/bigint_03.aivi{aivi}

## Decimal

`Decimal` is an opaque native type for fixed-point base-10 arithmetic. It is the right choice for totals, balances, and other values that humans expect to read in decimal form.

<<< ../../snippets/from_md/stdlib/math/number/decimal_01.aivi{aivi}

<<< ../../snippets/from_md/stdlib/math/number/decimal_02.aivi{aivi}

| Function | What it does |
| --- | --- |
| **fromFloat** value<br><code>Float -> Decimal</code> | Converts a `Float` to `Decimal` using decimal rounding rules. |
| **toFloat** value<br><code>Decimal -> Float</code> | Converts a `Decimal` to `Float`. |
| **round** value places<br><code>Decimal -> Int -> Decimal</code> | Rounds to the requested number of decimal places. |

<<< ../../snippets/from_md/stdlib/math/number/decimal_03.aivi{aivi}

## Rational

`Rational` is an opaque native type for exact fractions of the form `num/den`.

<<< ../../snippets/from_md/stdlib/math/number/rational_01.aivi{aivi}

<<< ../../snippets/from_md/stdlib/math/number/rational_02.aivi{aivi}

| Function | What it does |
| --- | --- |
| **normalize** r<br><code>Rational -> Rational</code> | Reduces a fraction to lowest terms. |
| **numerator** r<br><code>Rational -> BigInt</code> | Returns the numerator. |
| **denominator** r<br><code>Rational -> BigInt</code> | Returns the denominator. |

<<< ../../snippets/from_md/stdlib/math/number/rational_03.aivi{aivi}

## Complex

`Complex` represents values of the form `a + bi`. It is useful for signal processing, electrical engineering, and algorithms that naturally work in two perpendicular components.

<<< ../../snippets/from_md/stdlib/math/number/complex_01.aivi{aivi}

<<< ../../snippets/from_md/stdlib/math/number/complex_02.aivi{aivi}

<<< ../../snippets/from_md/stdlib/math/number/complex_03.aivi{aivi}

## Quaternion

The `Quaternion` domain represents 3D rotations without the gimbal-lock issues that can show up with Euler angles.

<<< ../../snippets/from_md/stdlib/math/number/quaternion_01.aivi{aivi}

<<< ../../snippets/from_md/stdlib/math/number/quaternion_02.aivi{aivi}

<<< ../../snippets/from_md/stdlib/math/number/quaternion_03.aivi{aivi}

| Function | What it does |
| --- | --- |
| **fromAxisAngle** axis theta<br><code>{ x: Float, y: Float, z: Float } -> Float -> Quaternion</code> | Creates a rotation from an axis and angle. |
| **conjugate** q<br><code>Quaternion -> Quaternion</code> | Negates the vector part. |
| **magnitude** q<br><code>Quaternion -> Float</code> | Returns the quaternion length. |
| **normalize** q<br><code>Quaternion -> Quaternion</code> | Returns a unit-length quaternion. |

<<< ../../snippets/from_md/stdlib/math/number/quaternion_04.aivi{aivi}
