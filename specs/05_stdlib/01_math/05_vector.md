````markdown
# Vector Domain

<!-- quick-info: {"kind":"module","name":"aivi.vector"} -->
The `Vector` domain handles 2D and 3D vectors (`Vec2`, `Vec3`), the fundamental atoms of spatial math.

A **Vector** is just a number with a direction. It's the difference between saying "10 miles" (Scalar) and "10 miles North" (Vector).
*   **Position**: "Where am I?" (Point)
*   **Velocity**: "Where am I going?" (Movement)
*   **Force**: "What's pushing me?" (Physics)

Graphics and physics use vectors for clean math (`v1 + v2`) and benefit from hardware acceleration (SIMD).

<!-- /quick-info -->
## Overview

<<< ../../snippets/from_md/05_stdlib/01_math/05_vector/block_01.aivi{aivi}


## Features

<<< ../../snippets/from_md/05_stdlib/01_math/05_vector/block_02.aivi{aivi}

## Short Constructors

Instead of writing full record literals, use convenience constructors:

<<< ../../snippets/from_md/05_stdlib/01_math/05_vector/block_05.aivi{aivi}

| Constructor | Type | Equivalent |
| --- | --- | --- |
| `vec2 x y` | `Float -> Float -> Vec2` | `{ x: x, y: y }` |
| `vec3 x y z` | `Float -> Float -> Vec3` | `{ x: x, y: y, z: z }` |
| `vec4 x y z w` | `Float -> Float -> Float -> Float -> Vec4` | `{ x: x, y: y, z: z, w: w }` |

Full record syntax is always available as well: `Vec2 { x: 1.0, y: 2.0 }`.

## Domain Definition

<<< ../../snippets/from_md/05_stdlib/01_math/05_vector/block_03.aivi{aivi}

## Helper Functions

| Function | Explanation |
| --- | --- |
| **magnitude** v<br><pre><code>`Vec2 -> Float`</code></pre> | Returns the Euclidean length of `v`. |
| **magnitude** v<br><pre><code>`Vec3 -> Float`</code></pre> | Returns the Euclidean length of `v`. |
| **normalize** v<br><pre><code>`Vec2 -> Vec2`</code></pre> | Returns a unit vector in the direction of `v`. |
| **normalize** v<br><pre><code>`Vec3 -> Vec3`</code></pre> | Returns a unit vector in the direction of `v`. |
| **dot** a b<br><pre><code>`Vec2 -> Vec2 -> Float`</code></pre> | Returns the dot product (scalar projection). |
| **dot** a b<br><pre><code>`Vec3 -> Vec3 -> Float`</code></pre> | Returns the dot product (scalar projection). |
| **cross** a b<br><pre><code>`Vec3 -> Vec3 -> Vec3`</code></pre> | Returns the 3D cross product orthogonal to `a` and `b`. |
| **lerp** a b t<br><pre><code>`Vec2 -> Vec2 -> Float -> Vec2`</code></pre> | Linear interpolation between `a` and `b` at parameter `t`. |
| **lerp** a b t<br><pre><code>`Vec3 -> Vec3 -> Float -> Vec3`</code></pre> | Linear interpolation between `a` and `b` at parameter `t`. |
| **distance** a b<br><pre><code>`Vec2 -> Vec2 -> Float`</code></pre> | Euclidean distance between two points. |
| **distance** a b<br><pre><code>`Vec3 -> Vec3 -> Float`</code></pre> | Euclidean distance between two points. |
| **negate** v<br><pre><code>`Vec2 -> Vec2`</code></pre> | Returns `-v`. |
| **negate** v<br><pre><code>`Vec3 -> Vec3`</code></pre> | Returns `-v`. |

## Vector × Matrix

These functions bridge vectors and matrices (see [Matrix](09_matrix.md)):

| Function | Explanation |
| --- | --- |
| **transform2** m v<br><pre><code>`Mat2 -> Vec2 -> Vec2`</code></pre> | Multiplies a 2×2 matrix by a 2D vector. |
| **transform3** m v<br><pre><code>`Mat3 -> Vec3 -> Vec3`</code></pre> | Multiplies a 3×3 matrix by a 3D vector. |
| **transform4** m v<br><pre><code>`Mat4 -> Vec4 -> Vec4`</code></pre> | Multiplies a 4×4 matrix by a 4D vector (homogeneous). |
| **transformPoint3** m p<br><pre><code>`Mat4 -> Vec3 -> Vec3`</code></pre> | Transforms a 3D point by a 4×4 matrix (w=1, perspective divide). |
| **transformDir3** m d<br><pre><code>`Mat4 -> Vec3 -> Vec3`</code></pre> | Transforms a 3D direction by a 4×4 matrix (w=0, no translation). |

## Usage Examples

<<< ../../snippets/from_md/05_stdlib/01_math/05_vector/block_04.aivi{aivi}

````
