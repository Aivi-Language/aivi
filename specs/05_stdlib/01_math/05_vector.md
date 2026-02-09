# Vector Domain

The `Vector` domain handles 2D and 3D vectors (`Vec2`, `Vec3`), the fundamental atoms of spatial math.

## What is a Vector?

A **Vector** represents a quantity that has both a magnitude (size) and a direction.
*   **Position**: "At point (10, 50)"
*   **Velocity**: "Moving North at 50 mph"
*   **Force**: "Pushing down with 10 Newtons"

In AIVI, vectors are first-class citizens. You can add them, scale them, and measure angles between them.

## Why this exists

Graphics, physics engines, and game logic run on vectors. While you *could* store `x`, `y`, and `z` variables separately, code becomes unreadable quickly. A native Vector domain allows for clean math (`v1 + v2`) and is often hardware-accelerated (SIMD) for speed.

## Overview

```aivi
import aivi.std.math.vector use { Vec2, Vec3 }

// Define using the `v2` tag
let v1 = (1.0, 2.0)`v2`
let v2 = (3.0, 4.0)`v2`

// Add components parallelly
let v3 = v1 + v2 // (4.0, 6.0)
```

## Features

```aivi
Vec2 = { x: Float, y: Float }
Vec3 = { x: Float, y: Float, z: Float }
Vec4 = { x: Float, y: Float, z: Float, w: Float }

Scalar = Float
```

## Domain Definition

```aivi
domain Vector over Vec2 = {
  (+) : Vec2 -> Vec2 -> Vec2
  (+) v1 v2 = { x: v1.x + v2.x, y: v1.y + v2.y }
  
  (-) : Vec2 -> Vec2 -> Vec2
  (-) v1 v2 = { x: v1.x - v2.x, y: v1.y - v2.y }
  
  (*) : Vec2 -> Scalar -> Vec2
  (*) v s = { x: v.x * s, y: v.y * s }
  
  (/) : Vec2 -> Scalar -> Vec2
  (/) v s = { x: v.x / s, y: v.y / s }
}

domain Vector over Vec3 = {
  (+) : Vec3 -> Vec3 -> Vec3
  (+) v1 v2 = { x: v1.x + v2.x, y: v1.y + v2.y, z: v1.z + v2.z }
  
  (-) : Vec3 -> Vec3 -> Vec3
  (-) v1 v2 = { x: v1.x - v2.x, y: v1.y - v2.y, z: v1.z - v2.z }
  
  (*) : Vec3 -> Scalar -> Vec3
  (*) v s = { x: v.x * s, y: v.y * s, z: v.z * s }
  
  (/) : Vec3 -> Scalar -> Vec3
  (/) v s = { x: v.x / s, y: v.y / s, z: v.z / s }
}
```

## Helper Functions

```aivi
magnitude : Vec2 -> Float
magnitude { x, y } = sqrt (x * x + y * y)

normalize : Vec2 -> Vec2
normalize v = v / magnitude v

dot : Vec2 -> Vec2 -> Float
dot v1 v2 = v1.x * v2.x + v1.y * v2.y

cross : Vec3 -> Vec3 -> Vec3
cross v1 v2 = {
  x: v1.y * v2.z - v1.z * v2.y
  y: v1.z * v2.x - v1.x * v2.z
  z: v1.x * v2.y - v1.y * v2.x
}
```

## Usage Examples

```aivi
use aivi.std.vector

position = { x: 10.0, y: 20.0 }
velocity = { x: 1.0, y: 0.5 }

newPos = position + velocity * 0.016  // 60fps frame
direction = normalize velocity
```
