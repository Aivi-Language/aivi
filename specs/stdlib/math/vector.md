# Vector Domain

<!-- quick-info: {"kind":"module","name":"aivi.vector"} -->
The `Vector` domain provides fixed-size 2D, 3D, and 4D vectors for spatial math.
Use it for positions, directions, velocities, forces, normals, or any other value that is naturally “a magnitude with components.”
<!-- /quick-info -->
<div class="import-badge">use aivi.vector<span class="domain-badge">domain</span></div>

Vectors show up anywhere geometry and motion meet. They let you write the math directly instead of unpacking `{ x, y, z }` fields by hand.

## Overview

<<< ../../snippets/from_md/stdlib/math/vector/overview.aivi{aivi}

## Features

<<< ../../snippets/from_md/stdlib/math/vector/features.aivi{aivi}

## Convenience constructors

The short constructors are handy when you want readable examples or compact call sites.

<<< ../../snippets/from_md/stdlib/math/vector/short_constructors.aivi{aivi}

| Constructor | Type | Equivalent |
| --- | --- | --- |
| `vec2 x y` | `Float -> Float -> Vec2` | `{ x: x, y: y }` |
| `vec3 x y z` | `Float -> Float -> Float -> Vec3` | `{ x: x, y: y, z: z }` |
| `vec4 x y z w` | `Float -> Float -> Float -> Float -> Vec4` | `{ x: x, y: y, z: z, w: w }` |

Full record syntax is always available too: `Vec2 { x: 1.0, y: 2.0 }`.

## Domain Definition

<<< ../../snippets/from_md/stdlib/math/vector/domain_definition.aivi{aivi}

`domain Vector` is defined for `Vec2`, `Vec3`, and `Vec4`. Each carrier supports `(+)`, `(-)`, scalar `(*) : VecN -> Float -> VecN`, and `(/) : VecN -> Float -> VecN`.

For matrix-vector transforms, use `×` from the `Matrix` domain; see [Matrix: `×` operator overloads](matrix.md#×-operator-overloads).

## Core helpers

| Function | What it does |
| --- | --- |
| **magnitude** v<br><code>Vec2 -> Float</code> | Returns the Euclidean length of `v`. |
| **magnitude** v<br><code>Vec3 -> Float</code> | Returns the Euclidean length of `v`. |
| **normalize** v<br><code>Vec2 -> Vec2</code> | Returns a unit vector pointing in the same direction as `v`. |
| **normalize** v<br><code>Vec3 -> Vec3</code> | Returns a unit vector pointing in the same direction as `v`. |
| **dot** a b<br><code>Vec2 -> Vec2 -> Float</code> | Returns the dot product of `a` and `b`. |
| **dot** a b<br><code>Vec3 -> Vec3 -> Float</code> | Returns the dot product of `a` and `b`. |
| **cross** a b<br><code>Vec3 -> Vec3 -> Vec3</code> | Returns the 3D cross product orthogonal to `a` and `b`. |
| **lerp** a b t<br><code>Vec2 -> Vec2 -> Float -> Vec2</code> | Linearly interpolates between `a` and `b` at parameter `t`. |
| **lerp** a b t<br><code>Vec3 -> Vec3 -> Float -> Vec3</code> | Linearly interpolates between `a` and `b` at parameter `t`. |
| **distance** a b<br><code>Vec2 -> Vec2 -> Float</code> | Returns the Euclidean distance between two points. |
| **distance** a b<br><code>Vec3 -> Vec3 -> Float</code> | Returns the Euclidean distance between two points. |
| **negate** v<br><code>Vec2 -> Vec2</code> | Returns `-v`. |
| **negate** v<br><code>Vec3 -> Vec3</code> | Returns `-v`. |
| **negate** v<br><code>Vec4 -> Vec4</code> | Returns `-v`. |

## Vector × Matrix

These helpers connect vector math to matrix math. They are also the concrete functions that back the `×` operator when the right-hand side is a vector.

| Function | What it does |
| --- | --- |
| **transform2** m v<br><code>Mat2 -> Vec2 -> Vec2</code> | Multiplies a 2×2 matrix by a 2D vector. |
| **transform3** m v<br><code>Mat3 -> Vec3 -> Vec3</code> | Multiplies a 3×3 matrix by a 3D vector. |
| **transform4** m v<br><code>Mat4 -> Vec4 -> Vec4</code> | Multiplies a 4×4 matrix by a 4D vector. |
| **transformPoint3** m p<br><code>Mat4 -> Vec3 -> Vec3</code> | Transforms a 3D point using `w = 1` and perspective divide. |
| **transformDir3** m d<br><code>Mat4 -> Vec3 -> Vec3</code> | Transforms a 3D direction using `w = 0`, so translation is ignored. |

> Use `×` when the `Matrix` domain is active and you want infix notation. Use the named helpers when you want an explicit, module-qualified call.

## Usage Examples

<<< ../../snippets/from_md/stdlib/math/vector/usage_examples.aivi{aivi}
