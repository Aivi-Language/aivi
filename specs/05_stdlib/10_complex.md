# Standard Library: Complex Domain

## Module

```aivi
module aivi.std.complex = {
  export domain Complex
  export Complex
  export fromPolar, conjugate, magnitude, phase
}
```

## Types

```aivi
Complex = { re: Float, im: Float }

Scalar = Float
```

## Domain Definition

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
  
  (/) : Complex -> Scalar -> Complex
  (/) z s = { re: z.re / s, im: z.im / s }
}
```

## Helper Functions

```aivi
fromPolar : Float -> Float -> Complex
fromPolar r theta = { re: r * cos theta, im: r * sin theta }

conjugate : Complex -> Complex
conjugate z = { re: z.re, im: -z.im }

magnitude : Complex -> Float
magnitude z = sqrt (z.re * z.re + z.im * z.im)

phase : Complex -> Float
phase z = atan2 z.im z.re
```

## Usage Examples

```aivi
use aivi.std.complex

z1 = { re: 3.0, im: 4.0 }
z2 = fromPolar 1.0 1.570796

sum = z1 + z2
product = z1 * z2
unit = z1 / magnitude z1
```