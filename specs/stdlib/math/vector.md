# Vector Domain

<!-- quick-info: {"kind":"module","name":"aivi.vector"} -->
The `Vector` domain handles 2D and 3D vectors (`Vec2`, `Vec3`), the fundamental atoms of spatial math.

A **Vector** is just a number with a direction. It's the difference between saying "10 miles" (Scalar) and "10 miles North" (Vector).
*   **Position**: "Where am I?" (Point)
*   **Velocity**: "Where am I going?" (Movement)
*   **Force**: "What's pushing me?" (Physics)

Graphics and physics use vectors for clean math (`v1 + v2`) and benefit from hardware acceleration (SIMD).

<!-- /quick-info -->
<div class="import-badge">use aivi.vector<span class="domain-badge">domain</span></div>

## Overview

<<< ../../snippets/from_md/stdlib/math/vector/overview.aivi{aivi}


## Features

<<< ../../snippets/from_md/stdlib/math/vector/features.aivi{aivi}

## Short Constructors

Instead of writing full record literals, use convenience constructors:

<<< ../../snippets/from_md/stdlib/math/vector/short_constructors.aivi{aivi}

| Constructor | Type | Equivalent |
| --- | --- | --- |
| `vec2 x y` | `Float -> Float -> Vec2` | `{ x: x, y: y }` |
| `vec3 x y z` | `Float -> Float -> Vec3` | `{ x: x, y: y, z: z }` |
| `vec4 x y z w` | `Float -> Float -> Float -> Float -> Vec4` | `{ x: x, y: y, z: z, w: w }` |

Full record syntax is always available as well: `Vec2 { x: 1.0, y: 2.0 }`.

## Domain Definition

<<< ../../snippets/from_md/stdlib/math/vector/domain_definition.aivi{aivi}

`domain Vector` is defined for `Vec2`, `Vec3`, and `Vec4`. Each carrier gets `(+)`, `(-)`, `(*) : VecN -> Float -> VecN` (scalar scale), and `(/) : VecN -> Float -> VecN`.

For matrix-vector transforms, use `×` from the `Matrix` domain (see [Matrix: `×` Operator Overloads](matrix.md#×-operator-overloads)).

## Helper Functions

| Function | Explanation |
| --- | --- |
| **magnitude** v<br><code>Vec2 -> Float</code> | Returns the Euclidean length of `v`. |
| **magnitude** v<br><code>Vec3 -> Float</code> | Returns the Euclidean length of `v`. |
| **normalize** v<br><code>Vec2 -> Vec2</code> | Returns a unit vector in the direction of `v`. |
| **normalize** v<br><code>Vec3 -> Vec3</code> | Returns a unit vector in the direction of `v`. |
| **dot** a b<br><code>Vec2 -> Vec2 -> Float</code> | Returns the dot product (scalar projection). |
| **dot** a b<br><code>Vec3 -> Vec3 -> Float</code> | Returns the dot product (scalar projection). |
| **cross** a b<br><code>Vec3 -> Vec3 -> Vec3</code> | Returns the 3D cross product orthogonal to `a` and `b`. |
| **lerp** a b t<br><code>Vec2 -> Vec2 -> Float -> Vec2</code> | Linear interpolation between `a` and `b` at parameter `t`. |
| **lerp** a b t<br><code>Vec3 -> Vec3 -> Float -> Vec3</code> | Linear interpolation between `a` and `b` at parameter `t`. |
| **distance** a b<br><code>Vec2 -> Vec2 -> Float</code> | Euclidean distance between two points. |
| **distance** a b<br><code>Vec3 -> Vec3 -> Float</code> | Euclidean distance between two points. |
| **negate** v<br><code>Vec2 -> Vec2</code> | Returns `-v`. |
| **negate** v<br><code>Vec3 -> Vec3</code> | Returns `-v`. |
| **negate** v<br><code>Vec4 -> Vec4</code> | Returns `-v`. |

## Vector × Matrix

These functions bridge vectors and matrices (see [Matrix](matrix.md)). They are also the implementations behind the `×` domain operator in the `Matrix` domain when the RHS is a vector type:

| Function | Explanation |
| --- | --- |
| **transform2** m v<br><code>Mat2 -> Vec2 -> Vec2</code> | Multiplies a 2×2 matrix by a 2D vector. |
| **transform3** m v<br><code>Mat3 -> Vec3 -> Vec3</code> | Multiplies a 3×3 matrix by a 3D vector. |
| **transform4** m v<br><code>Mat4 -> Vec4 -> Vec4</code> | Multiplies a 4×4 matrix by a 4D vector (homogeneous). |
| **transformPoint3** m p<br><code>Mat4 -> Vec3 -> Vec3</code> | Transforms a 3D point by a 4×4 matrix (w=1, perspective divide). |
| **transformDir3** m d<br><code>Mat4 -> Vec3 -> Vec3</code> | Transforms a 3D direction by a 4×4 matrix (w=0, no translation). |

> The `×` operator in the `Matrix` domain desugars to `transform4 m v` (or `transform2`/`transform3` for the respective sizes). Use `×` in domain-active contexts; use these functions directly when calling from module-qualified paths.

## Usage Examples

<<< ../../snippets/from_md/stdlib/math/vector/usage_examples.aivi{aivi}
