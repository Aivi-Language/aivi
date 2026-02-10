# Number Domains (BigInt, Rational, Complex)

The `aivi.number` family groups numeric domains that sit above `Int` and `Float`:

- `aivi.number.bigint` for arbitrary-precision integers
- `aivi.number.rational` for exact fractions
- `aivi.number.complex` for complex arithmetic

You can use either the facade module or the specific domain module depending on how much you want in scope.

```aivi
// Facade (types + helpers)
use aivi.number

// Domain modules (operators + literals)
use aivi.number.bigint
use aivi.number.rational
use aivi.number.complex
```


## BigInt

`BigInt` is an **opaque native type** for arbitrary-precision integers.

```aivi
// Native type (backed by Rust BigInt or similar)
type BigInt
```

```aivi
domain BigInt over BigInt = {
  (+) : BigInt -> BigInt -> BigInt
  (-) : BigInt -> BigInt -> BigInt
  (*) : BigInt -> BigInt -> BigInt

  1n = fromInt 1
}
```

Helpers:

| Function | Explanation |
| --- | --- |
| **fromInt** value<br><pre><code>`Int -> BigInt`</code></pre> | Converts a machine `Int` into `BigInt`. |
| **toInt** value<br><pre><code>`BigInt -> Int`</code></pre> | Converts a `BigInt` to `Int` (may overflow in implementations). |

Example:

```aivi
use aivi.number.bigint

huge = 10_000_000_000_000_000_000_000n
sum = huge + 1n
```

## Decimal

`Decimal` is an **opaque native type** for fixed-point arithmetic (base-10), suitable for financial calculations where `Float` precision errors are unacceptable.

```aivi
// Native type (backed by Rust Decimal or similar)
type Decimal
```

```aivi
domain Decimal over Decimal = {
  (+) : Decimal -> Decimal -> Decimal
  (-) : Decimal -> Decimal -> Decimal
  (*) : Decimal -> Decimal -> Decimal
  (/) : Decimal -> Decimal -> Decimal

  // Literal suffix 'dec'
  1.0dec = fromFloat 1.0
}
```

Helpers:

| Function | Explanation |
| --- | --- |
| **fromFloat** value<br><pre><code>`Float -> Decimal`</code></pre> | Converts a `Float` into `Decimal` using base-10 rounding rules. |
| **toFloat** value<br><pre><code>`Decimal -> Float`</code></pre> | Converts a `Decimal` into a `Float`. |
| **round** value places<br><pre><code>`Decimal -> Int -> Decimal`</code></pre> | Rounds to `places` decimal digits. |

Example:

```aivi
use aivi.number.decimal

price = 19.99dec
tax = price * 0.2dec
total = price + tax
```

## Rational

`Rational` is an **opaque native type** for exact fractions (`num/den`).

```aivi
// Native type (backed by Rust Rational or similar)
type Rational
```

```aivi
domain Rational over Rational = {
  (+) : Rational -> Rational -> Rational
  (-) : Rational -> Rational -> Rational
  (*) : Rational -> Rational -> Rational
  (/) : Rational -> Rational -> Rational
}
```

Helpers:

| Function | Explanation |
| --- | --- |
| **normalize** r<br><pre><code>`Rational -> Rational`</code></pre> | Reduces a fraction to lowest terms. |
| **numerator** r<br><pre><code>`Rational -> BigInt`</code></pre> | Returns the numerator. |
| **denominator** r<br><pre><code>`Rational -> BigInt`</code></pre> | Returns the denominator. |

Example:

```aivi
use aivi.number.rational

// exact 1/2
half = normalize (fromInt 1 / fromInt 2) 
sum = half + half
```

## Complex

`Complex` represents values of the form `a + bi`. It is typically a struct of two floats, but domain operations are backed by optimized native implementations.

```aivi
Complex = { re: Float, im: Float }
i : Complex
```

```aivi
domain Complex over Complex = {
  (+) : Complex -> Complex -> Complex
  (+) a b = { re: a.re + b.re, im: a.im + b.im }

  (-) : Complex -> Complex -> Complex
  (-) a b = { re: a.re - b.re, im: a.im - b.im }

  (*) : Complex -> Complex -> Complex
  (*) a b = {
    re: a.re * b.re - a.im * b.im,
    im: a.re * b.im + a.im * b.re
  }

  (/) : Complex -> Float -> Complex
  (/) z s = { re: z.re / s, im: z.im / s }
}
```

Example:

```aivi
use aivi.number.complex

z1 = 3.0 + 4.0 * i
z2 = 1.0 - 2.0 * i
sum = z1 + z2
```

## Quaternion

The `Quaternion` domain provides tools for handling **3D rotations** without gimbal lock.

```aivi
use aivi.number.quaternion (Quat)

// Rotate 90 degrees around the Y (up) axis
q1 = Quat.fromEuler(0.0, 90.0, 0.0)

// The "identity" quaternion means "no rotation"
q2 = Quat.identity()

// Smoothly transition halfway between "no rotation" and "90 degrees"
interpolated = Quat.slerp(q1, q2, 0.5)
```

```aivi
Quaternion = { w: Float, x: Float, y: Float, z: Float }
```

```aivi
domain Quaternion over Quaternion = {
  (+) : Quaternion -> Quaternion -> Quaternion
  (+) a b = { w: a.w + b.w, x: a.x + b.x, y: a.y + b.y, z: a.z + b.z }

  (-) : Quaternion -> Quaternion -> Quaternion
  (-) a b = { w: a.w - b.w, x: a.x - b.x, y: a.y - b.y, z: a.z - b.z }

  (*) : Quaternion -> Quaternion -> Quaternion
  (*) a b = {
    w: a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
    x: a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
    y: a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
    z: a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w
  }

  (/) : Quaternion -> Float -> Quaternion
  (/) q s = { w: q.w / s, x: q.x / s, y: q.y / s, z: q.z / s }
}
```

| Function | Explanation |
| --- | --- |
| **fromAxisAngle** axis theta<br><pre><code>`{ x: Float, y: Float, z: Float } -> Float -> Quaternion`</code></pre> | Creates a rotation from axis/angle. |
| **conjugate** q<br><pre><code>`Quaternion -> Quaternion`</code></pre> | Negates the vector part. |
| **magnitude** q<br><pre><code>`Quaternion -> Float`</code></pre> | Returns the quaternion length. |
| **normalize** q<br><pre><code>`Quaternion -> Quaternion`</code></pre> | Returns a unit-length quaternion. |

```aivi
use aivi.number.quaternion

axis = { x: 0.0, y: 1.0, z: 0.0 }
spin = fromAxisAngle axis 1.570796

unit = normalize spin
```
