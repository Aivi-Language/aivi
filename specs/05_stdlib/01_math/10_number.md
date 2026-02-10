# Number Domains (BigInt, Rational, Complex)

The `aivi.std.number` family groups numeric domains that sit above `Int` and `Float`:

- `aivi.std.number.bigint` for arbitrary-precision integers
- `aivi.std.number.rational` for exact fractions
- `aivi.std.number.complex` for complex arithmetic

You can import either the facade module or the specific domain module depending on how much you want in scope.

```aivi
// Facade (types + helpers)
use aivi.std.number

// Domain modules (operators + literals)
use aivi.std.number.bigint
use aivi.std.number.rational
use aivi.std.number.complex
```

## Performance and Implementation (Rust)

Numeric domains must lean on established Rust crates to guarantee correctness and speed. Use `num-bigint` for `BigInt`, `num-rational` for `Rational`, `rust_decimal` for `Decimal`, and `num-complex` for `Complex` (or equivalent battle-tested libraries) rather than custom arithmetic.

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

```aivi
fromInt : Int -> BigInt
toInt : BigInt -> Int
```

Example:

```aivi
use aivi.std.number.bigint

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

```aivi
fromFloat : Float -> Decimal
toFloat : Decimal -> Float
round : Decimal -> Int -> Decimal
```

Example:

```aivi
use aivi.std.number.decimal

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

```aivi
normalize : Rational -> Rational
numerator : Rational -> BigInt
denominator : Rational -> BigInt
```

Example:

```aivi
use aivi.std.number.rational

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
use aivi.std.number.complex

z1 = 3.0 + 4.0 * i
z2 = 1.0 - 2.0 * i
sum = z1 + z2
```

## Quaternion

The `Quaternion` domain provides tools for handling **3D rotations** without gimbal lock.

```aivi
use aivi.std.number.quaternion (Quat)

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

```aivi
fromAxisAngle : { x: Float, y: Float, z: Float } -> Float -> Quaternion
fromAxisAngle axis theta = {
  w: cos (theta / 2.0),
  x: axis.x * sin (theta / 2.0),
  y: axis.y * sin (theta / 2.0),
  z: axis.z * sin (theta / 2.0)
}

conjugate : Quaternion -> Quaternion
conjugate q = { w: q.w, x: -q.x, y: -q.y, z: -q.z }

magnitude : Quaternion -> Float
magnitude q = sqrt (q.w * q.w + q.x * q.x + q.y * q.y + q.z * q.z)

normalize : Quaternion -> Quaternion
normalize q = q / magnitude q
```

```aivi
use aivi.std.number.quaternion

axis = { x: 0.0, y: 1.0, z: 0.0 }
spin = fromAxisAngle axis 1.570796

unit = normalize spin
```
