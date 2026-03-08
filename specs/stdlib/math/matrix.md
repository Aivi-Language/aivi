# Matrix Domain

<!-- quick-info: {"kind":"module","name":"aivi.matrix"} -->
The `Matrix` domain provides fixed-size square matrices used for transforms, coordinate changes, and other grid-shaped linear algebra operations.
It is especially useful for graphics, geometry, and simulation code where you want to combine multiple transforms into one reusable value.
<!-- /quick-info -->
<div class="import-badge">use aivi.matrix<span class="domain-badge">domain</span></div>

A good mental model is: a matrix encodes a reusable transform. One value can represent scaling, rotation, or a composition of several transforms.
In v0.1, `aivi.matrix` is intentionally focused: it exposes `Mat2`, `Mat3`, and `Mat4`, not arbitrary `Mat M N` sizes.
For vector-specific helpers such as `vec2`, `transformPoint3`, or `transformDir3`, see [Vector](vector.md). For broader algebraic helpers, see [Linear Algebra](linear_algebra.md).

## Overview

<<< ../../snippets/from_md/stdlib/math/matrix/block_01.aivi{aivi}


`use aivi.matrix` brings the named helpers (`identity2`, `multiply4`, ...) and `domain Matrix` into scope.
Import `aivi.vector` as well when you want `Vec2`, `Vec3`, `Vec4`, or vector constructors such as `vec2`.

## Supported matrix types

| Type | Meaning |
| --- | --- |
| `Mat2` | 2×2 matrix with row-major fields `m00`..`m11`. |
| `Mat3` | 3×3 matrix with row-major fields `m00`..`m22`. |
| `Mat4` | 4×4 matrix with row-major fields `m00`..`m33`. |
| `Scalar` | Alias for `Float`; used by scalar scaling such as `mat * 0.5`. |

This module is always about one of those three square sizes. If you need a literal, helper, or operator here, it is specialized for `Mat2`, `Mat3`, or `Mat4`.

## `×` operator overloads

The `Matrix` domain overloads `×` based on the right-hand side type, so the same operator can mean matrix-by-matrix composition or matrix-by-vector transformation.

Read it this way:

- matrix `×` matrix = combine two transforms into one,
- matrix `×` vector = apply the transform to a point or direction.

| Expression | Resolved as | Returns |
| --- | --- | --- |
| `mat2 × mat2` | `multiply2 mat2 mat2` | `Mat2` |
| `mat2 × vec2` | `transform2 mat2 vec2` | `Vec2` |
| `mat3 × mat3` | `multiply3 mat3 mat3` | `Mat3` |
| `mat3 × vec3` | `transform3 mat3 vec3` | `Vec3` |
| `mat4 × mat4` | `multiply4 mat4 mat4` | `Mat4` |
| `mat4 × vec4` | `transform4 mat4 vec4` | `Vec4` |

To use infix `×`, bring `domain Matrix` into scope with `use aivi.matrix` or `use aivi.matrix (domain Matrix, ...)`.
The vector overloads dispatch to `transform2`, `transform3`, and `transform4` from [Vector](vector.md#vector-×-matrix); the matrix overloads dispatch to `multiply2`, `multiply3`, and `multiply4` from this module.

Because matrix multiplication composes transforms, `(a × b) × v` applies `b` first and then `a`.
`×` is reserved for structural products; `*` stays available for scalar scaling, with the matrix on the left: `mat * 0.5`.

## Core helpers

Use the named helpers when you want explicit calls instead of operator syntax.

| Function | What it does |
| --- | --- |
| **identity2**<br><code>Mat2</code> | Identity matrix for 2×2 work. |
| **identity3**<br><code>Mat3</code> | Identity matrix for 3×3 work. |
| **identity4**<br><code>Mat4</code> | Identity matrix for 4×4 work. |
| **transpose2** m<br><code>Mat2 -> Mat2</code> | Swaps rows and columns of a 2×2 matrix. |
| **transpose3** m<br><code>Mat3 -> Mat3</code> | Swaps rows and columns of a 3×3 matrix. |
| **transpose4** m<br><code>Mat4 -> Mat4</code> | Swaps rows and columns of a 4×4 matrix. |
| **multiply2** a b<br><code>Mat2 -> Mat2 -> Mat2</code> | Multiplies two 2×2 matrices. |
| **multiply3** a b<br><code>Mat3 -> Mat3 -> Mat3</code> | Multiplies two 3×3 matrices. |
| **multiply4** a b<br><code>Mat4 -> Mat4 -> Mat4</code> | Multiplies two 4×4 matrices. |

## Sigil constructors

For concise literals, use the structured `~mat` sigil.

<<< ../../snippets/from_md/stdlib/math/matrix/block_02.aivi{aivi}


You can write the cells on one line or across multiple lines. The formatter rewrites the literal as aligned rows, which is the style shown above because it is easiest to scan.
The sigil accepts only 4, 9, or 16 cells, inferring `Mat2`, `Mat3`, or `Mat4` from that square size.

## Usage Examples

<<< ../../snippets/from_md/stdlib/math/matrix/block_03.aivi{aivi}


`combined` and `explicit` are the same matrix. `result` first scales `point` and then rotates it.

## Related domains

- See [Vector](vector.md) for `Vec2`/`Vec3`/`Vec4`, vector constructors, and helpers such as `transformPoint3` and `transformDir3`.
- See [Linear Algebra](linear_algebra.md) when the main job is algebraic operations over vectors and matrices rather than storing reusable transforms.
