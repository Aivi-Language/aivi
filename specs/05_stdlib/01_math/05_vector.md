# Vector Domain

The `Vector` domain handles 2D and 3D vectors (`Vec2`, `Vec3`), the fundamental atoms of spatial math.

A **Vector** is just a number with a direction. It's the difference between saying "10 miles" (Scalar) and "10 miles North" (Vector).
*   **Position**: "Where am I?" (Point)
*   **Velocity**: "Where am I going?" (Movement)
*   **Force**: "What's pushing me?" (Physics)

Graphics, physicsows for clean math (`v1 + v2`) and is often hardware-accelerated (SIMD) for speed.

## Overview

```aivi
use aivi.std.math.vector (Vec2, Vec3)

// Define using the `v2` tag
v1 = (1.0, 2.0)v2
v2 = (3.0, 4.0)v2

// Add components parallelly
v3 = v1 + v2 // (4.0, 6.0)
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
