# Standard Library: Quaternion Domain

## Module

```aivi
module aivi.std.quaternion = {
  export domain Quaternion
  export Quaternion
  export fromAxisAngle, conjugate, magnitude, normalize
}
```

## Types

```aivi
Quaternion = { w: Float, x: Float, y: Float, z: Float }

Scalar = Float
```

## Domain Definition

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
  
  (/) : Quaternion -> Scalar -> Quaternion
  (/) q s = { w: q.w / s, x: q.x / s, y: q.y / s, z: q.z / s }
}
```

## Helper Functions

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

## Usage Examples

```aivi
use aivi.std.quaternion

axis = { x: 0.0, y: 1.0, z: 0.0 }
spin = fromAxisAngle axis 1.570796

unit = normalize spin
```