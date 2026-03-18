# Numbers

<!-- quick-info: {"kind":"module","name":"aivi.number"} -->
The `aivi.number` page covers the `aivi.number` facade together with the specialised numeric submodules that you import directly when ordinary machine numbers are not enough.
Reach for this family when you need arbitrary-size integers, exact fractions, fixed-point decimal values, complex numbers, or quaternion-based rotations. For everyday `Int` and `Float` helpers such as `sqrt`, `round`, and trigonometry, start with [`aivi.math`](./math.md).
<!-- /quick-info -->
<div class="import-badge">use aivi.number</div>

The facade is the best starting point when you want the common names in one place. Import a submodule directly when you also want its domain operators or literals:

- `aivi.number.bigint` for arbitrary-precision integers and helper conversions; add `use aivi.number.bigint (domain BigInt)` when you want `1n`-style literals
- `aivi.number.rational` for exact fractions and the `fromBigInts` constructor
- `aivi.number.decimal` for fixed-point base-10 arithmetic and `dec`-suffixed literals
- `aivi.number.complex` for complex arithmetic and the `i` constant
- `aivi.number.quaternion` for 3D rotations; it belongs to the number family, but it is imported directly rather than re-exported by the `aivi.number` facade

The facade re-exports `fromInt`, `toInt`, `fromFloat`, `toFloat`, `round`, `fromBigInts`, `normalize`, `numerator`, `denominator`, and `i`. Import the matching direct submodule when you also want that type's domain operators or literal sugar.

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

A common import setup looks like this:

<<< ../../snippets/from_md/stdlib/math/number/choosing_between_decimal_and_rational.aivi{aivi}

## BigInt

`BigInt` is an opaque native type for integers larger than machine `Int` can represent. Import `aivi.number.bigint` for the conversion helpers, and add `use aivi.number.bigint (domain BigInt)` when you want `1n`-style literals.

<<< ../../snippets/from_md/stdlib/math/number/block_01.aivi{aivi}


| Function | What it does |
| --- | --- |
| **fromInt** value<br><code>Int -> BigInt</code> | Converts a machine `Int` into `BigInt`. |
| **toInt** value<br><code>BigInt -> Option Int</code> | Converts a `BigInt` back to `Int`, returning `None` if it would overflow. |
| **absInt** value<br><code>Int -> Int</code> | Returns the absolute value of a machine `Int`; it is mainly useful when normalising signs before converting to `BigInt`. |

## Decimal

`Decimal` is an opaque native type for fixed-point base-10 arithmetic. It is the right choice for totals, balances, and other values that humans expect to read in decimal form. Import `aivi.number.decimal` directly when you want `dec`-suffixed literals and decimal operators.

`fromFloat` accepts only finite `Float` values. If NaN or infinity matters to your algorithm, stay in `Float` or switch to another exact representation before crossing the API boundary.

Division through the `Decimal` domain requires a non-zero divisor. Dividing by zero raises the standard runtime division-by-zero diagnostic instead of aborting the process.

<<< ../../snippets/from_md/stdlib/math/number/block_02.aivi{aivi}


| Function | What it does |
| --- | --- |
| **fromFloat** value<br><code>Float -> Decimal</code> | Converts a finite `Float` to `Decimal` using decimal rounding rules. |
| **toFloat** value<br><code>Decimal -> Float</code> | Converts a `Decimal` to `Float`. |
| **round** value places<br><code>Decimal -> Int -> Decimal</code> | Rounds to the requested number of decimal places; negative `places` values are treated as `0`. |

## Rational

`Rational` is an opaque native type for exact fractions of the form `num/den`. Import `aivi.number.rational` directly when you want exact `+`, `-`, `*`, and `/` on fractions.

Create values with `fromBigInts`; passing `0` as the denominator raises an invalid-argument runtime diagnostic.

Dividing by a zero rational raises the standard runtime division-by-zero diagnostic.

<<< ../../snippets/from_md/stdlib/math/number/block_03.aivi{aivi}


| Function | What it does |
| --- | --- |
| **fromBigInts** num den<br><code>BigInt -> BigInt -> Rational</code> | Creates a `Rational` from a numerator and denominator. The denominator must be non-zero. |
| **normalize** r<br><code>Rational -> Rational</code> | Reduces a fraction to lowest terms. |
| **numerator** r<br><code>Rational -> BigInt</code> | Returns the numerator. |
| **denominator** r<br><code>Rational -> BigInt</code> | Returns the denominator. |

## Complex

`Complex` values are records of the form `{ re: Float, im: Float }`. The module exports the `i` constant and a `Complex` domain with `+`, `-`, `*`, and division by a scalar `Float`.

<<< ../../snippets/from_md/stdlib/math/number/block_04.aivi{aivi}


## Quaternion

`aivi.number.quaternion` models 3D rotations without the gimbal-lock issues that can show up with Euler angles. `fromAxisAngle` takes an axis record and an angle in radians, and it normalizes the axis for you before building the quaternion.

<<< ../../snippets/from_md/stdlib/math/number/block_05.aivi{aivi}


The direct module also provides quaternion `+`, `-`, `*`, and division by a scalar `Float` through the `Quaternion` domain.

| Function | What it does |
| --- | --- |
| **fromAxisAngle** axis theta<br><code>{ x: Float, y: Float, z: Float } -> Float -> Quaternion</code> | Creates a rotation from an axis and an angle in radians. |
| **conjugate** q<br><code>Quaternion -> Quaternion</code> | Negates the vector part. |
| **magnitude** q<br><code>Quaternion -> Float</code> | Returns the quaternion length. |
| **normalize** q<br><code>Quaternion -> Quaternion</code> | Returns a unit-length quaternion, or the original value when its magnitude is zero. |
