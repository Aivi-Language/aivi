# Matrix Domain

<!-- quick-info: {"kind":"module","name":"aivi.matrix"} -->
The `Matrix` domain provides fixed-size matrices used for transforms, coordinate changes, and other grid-shaped linear algebra operations.
It is especially useful for graphics, geometry, and simulation code where you want to combine multiple transforms into one reusable value.
<!-- /quick-info -->
<div class="import-badge">use aivi.matrix<span class="domain-badge">domain</span></div>

A good mental model is: a matrix encodes a transformation you can reuse. One value can represent scaling, rotation, translation, or a combination of them.

## Overview

<<< ../../snippets/from_md/stdlib/math/matrix/sigil_constructors.aivi{aivi}

## Features

<<< ../../snippets/from_md/stdlib/math/matrix/features.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/math/matrix/domain_definition.aivi{aivi}

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

`×` is reserved for structural products; `*` stays available for scalar scaling.

Requires `use aivi.matrix (domain Matrix)` or `use aivi.matrix`, plus `use aivi.vector` if you want the `Vec*` types in scope.

## Core helpers

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

<<< ../../snippets/from_md/stdlib/math/matrix/sigil_constructors.aivi{aivi}

Rows are separated by newlines and columns by whitespace. The formatter aligns columns for readability, so the literal stays easy to scan like a small grid. The sigil infers `Mat2`, `Mat3`, or `Mat4` from the row and column count.

## Usage Examples

<<< ../../snippets/from_md/stdlib/math/matrix/usage_examples.aivi{aivi}
