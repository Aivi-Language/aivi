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

Numeric domains must lean on established Rust crates to guarantee correctness and speed. Use `num-bigint` for `BigInt`, `num-rational` for `Rational`, and `num-complex` for `Complex` (or equivalent battle-tested libraries) rather than custom arithmetic.

## BigInt

`BigInt` grows as needed, limited only by memory. Use it for large integers that overflow `Int`.

```aivi
BigInt = { sign: Int, limbs: List Int }
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

## Rational

`Rational` stores exact fractions (`num/den`) and keeps results normalized.

```aivi
Rational = { num: BigInt, den: BigInt }
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
```

Example:

```aivi
use aivi.std.number.rational

half = normalize { num: fromInt 1, den: fromInt 2 }
sum = half + half
```

## Complex

`Complex` represents values of the form `a + bi`.

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
