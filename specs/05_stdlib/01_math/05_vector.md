# Vector Domain

The `Vector` domain handles 2D and 3D vectors (`Vec2`, `Vec3`), the fundamental atoms of spatial math.

A **Vector** is just a number with a direction. It's the difference between saying "10 miles" (Scalar) and "10 miles North" (Vector).
*   **Position**: "Where am I?" (Point)
*   **Velocity**: "Where am I going?" (Movement)
*   **Force**: "What's pushing me?" (Physics)

Graphics and physics use vectors for clean math (`v1 + v2`) and benefit from hardware acceleration (SIMD).

## Overview

<<< ../../snippets/from_md/05_stdlib/01_math/05_vector/block_01.aivi{aivi}


## Features

<<< ../../snippets/from_md/05_stdlib/01_math/05_vector/block_02.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/05_stdlib/01_math/05_vector/block_03.aivi{aivi}

## Helper Functions

| Function | Explanation |
| --- | --- |
| **magnitude** v<br><pre><code>`Vec2 -> Float`</code></pre> | Returns the Euclidean length of `v`. |
| **normalize** v<br><pre><code>`Vec2 -> Vec2`</code></pre> | Returns a unit vector in the direction of `v`. |
| **dot** a b<br><pre><code>`Vec2 -> Vec2 -> Float`</code></pre> | Returns the dot product of `a` and `b`. |
| **cross** a b<br><pre><code>`Vec3 -> Vec3 -> Vec3`</code></pre> | Returns the 3D cross product orthogonal to `a` and `b`. |

## Usage Examples

<<< ../../snippets/from_md/05_stdlib/01_math/05_vector/block_04.aivi{aivi}
