# Vector Domain

<!-- quick-info: {"kind":"module","name":"aivi.vector"} -->
The `Vector` domain provides fixed-size 2D, 3D, and 4D vectors for spatial math.
Use it for positions, directions, velocities, forces, normals, or any other value that is naturally “a magnitude with components.”
<!-- /quick-info -->
<div class="import-badge">use aivi.vector<span class="domain-badge">domain</span></div>

Vectors show up anywhere geometry and motion meet. They let you write the math directly instead of unpacking `{ x, y, z }` fields by hand.
`use aivi.vector` brings the vector types, constructors, named helpers, and `domain Vector` into scope. If you import selectively, include `domain Vector` whenever you want `+`, `-`, `*`, and `/` to work on vectors.

## Overview

<<< ../../snippets/from_md/stdlib/math/vector/block_01.aivi{aivi}


## Features

<<< ../../snippets/from_md/stdlib/math/vector/features.aivi{aivi}

## Supported vector types

This module is always about one of these fixed-size shapes:

| Type | Typical use |
| --- | --- |
| `Vec2` | 2D points, screen-space positions, velocities, and other planar values. |
| `Vec3` | 3D points, directions, normals, and simulation state in world space. |
| `Vec4` | Homogeneous coordinates, packed 4-lane data, or values you want to transform with `Mat4`. |

Not every named helper is implemented for every size. The tables below list the exact supported combinations.

## Convenience constructors

The short constructors are handy when you want readable examples or compact call sites.

<<< ../../snippets/from_md/stdlib/math/vector/short_constructors.aivi{aivi}

| Constructor | Type | Equivalent |
| --- | --- | --- |
| `vec2 x y` | `Float -> Float -> Vec2` | `{ x: x, y: y }` |
| `vec3 x y z` | `Float -> Float -> Float -> Vec3` | `{ x: x, y: y, z: z }` |
| `vec4 x y z w` | `Float -> Float -> Float -> Float -> Vec4` | `{ x: x, y: y, z: z, w: w }` |

Plain record literals are always available too when the expected shape is already clear: `{ x: 1.0, y: 2.0 }`.

## Domain Definition

<<< ../../snippets/from_md/stdlib/math/vector/domain_definition.aivi{aivi}

`domain Vector` is defined for `Vec2`, `Vec3`, and `Vec4`. Each carrier supports `(+)`, `(-)`, scalar `(*) : VecN -> Float -> VecN`, and `(/) : VecN -> Float -> VecN`.

For matrix-vector transforms, use `×` from the `Matrix` domain; see [Matrix: `×` operator overloads](matrix.md#×-operator-overloads). For matrix-by-matrix composition or transform-building helpers, continue with [Matrix](matrix.md). For broader algebraic helpers that combine vectors and matrices, see [Linear Algebra](linear_algebra.md).

## Core helpers

Use these named helpers when you want an explicit call instead of operator syntax, or when the operation is not part of `domain Vector`.

| Function | What it does |
| --- | --- |
| **magnitude** v<br><code>Vec2 -> Float</code> | Returns the Euclidean length of `v`. |
| **magnitude** v<br><code>Vec3 -> Float</code> | Returns the Euclidean length of `v`. |
| **magnitude** v<br><code>Vec4 -> Float</code> | Returns the Euclidean length of `v`. |
| **normalize** v<br><code>Vec2 -> Vec2</code> | Returns a unit vector pointing in the same direction as `v`. If `v` is the zero vector, it is returned unchanged. |
| **normalize** v<br><code>Vec3 -> Vec3</code> | Returns a unit vector pointing in the same direction as `v`. If `v` is the zero vector, it is returned unchanged. |
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

## Matrix × Vector bridges

These helpers connect vector math to matrix math. They live in `aivi.vector` because they return vectors, and they are also the concrete functions that back the `×` operator when the right-hand side is a vector.

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
