# Linear Algebra Domain

<!-- quick-info: {"kind":"module","name":"aivi.linear_algebra"} -->
The `LinearAlgebra` domain collects helpers for vector and matrix calculations that show up in simulation, graphics, optimization, and scientific code.
Use it when you want algebraic operations such as dot products, matrix multiplication, or solving small linear systems as first-class library functions.
<!-- /quick-info -->
<div class="import-badge">use aivi.linear_algebra<span class="domain-badge">domain</span></div>

If [`aivi.vector`](./vector.md) and [`aivi.matrix`](./matrix.md) are about data shapes, `aivi.linear_algebra` is about the calculations you perform with those shapes.
If you prefer a shorter import, `aivi.linalg` re-exports the same API.

## Start here

Choose between the related domains like this:

- use [`aivi.vector`](./vector.md) when you mainly need vector values and helper operations on one vector at a time,
- use [`aivi.matrix`](./matrix.md) when you mainly need matrix values and transform-oriented helpers,
- use `aivi.linear_algebra` when the key job is combining vectors or matrices mathematically.

## What it is for

This domain is useful when you need to:

- compare directions with a dot product
- combine transformations with matrix multiplication
- solve small systems such as `A x = b`

## Overview

`aivi.linear_algebra` works with generic vector and matrix records:

```aivi
use aivi.linear_algebra

basisX = { size: 2, data: [1.0, 0.0] }
basisY = { size: 2, data: [0.0, 1.0] }

system = { rows: 2, cols: 2, data: [1.0, 1.0, 1.0, -1.0] }
rhs = { size: 2, data: [3.0, 1.0] }

alignment = dot basisX basisY
solution = solve2x2 system rhs
```

Here `alignment` is `0.0`, and `solution.data` is `[2.0, 1.0]`.

## Features

- `Vec` values use the shape `{ size: Int, data: List Float }`.
- `Mat` values use the shape `{ rows: Int, cols: Int, data: List Float }`.
- `dot` compares two vectors of the same size and returns one `Float`.
- `matMul` multiplies two matrices when the left column count matches the right row count.
- `solve2x2` solves one 2×2 system and rejects singular matrices.
- `domain LinearAlgebra` adds vector `+`, `-`, and scalar `*` over `Vec`.

## Domain Definition

The exported domain is defined over `Vec` records and provides component-wise addition, component-wise subtraction, and scalar scaling:

| Operator | Type | Meaning |
| --- | --- | --- |
| `a + b` | `Vec -> Vec -> Vec` | Adds corresponding vector components. |
| `a - b` | `Vec -> Vec -> Vec` | Subtracts corresponding vector components. |
| `v * s` | `Vec -> Float -> Vec` | Scales every component of `v` by scalar `s`. |

## Core helpers

| Function | What it does |
| --- | --- |
| **dot** a b<br><code>Vec -> Vec -> Float</code> | Returns the dot product of two vectors. A dot product is a single number that tells you how aligned two directions are. |
| **matMul** a b<br><code>Mat -> Mat -> Mat</code> | Multiplies matrices by combining rows of `a` with columns of `b`. Use this to compose transforms or other matrix-to-matrix calculations. |
| **solve2x2** m v<br><code>Mat -> Vec -> Vec</code> | Solves the 2×2 system `A x = b`. This is a handy small-system helper when you want to recover the unknown vector `x`. |

### Current validation

The current runtime checks the following conditions before computing a result:

- `dot` requires both vectors to have the same size.
- `matMul` requires the left matrix column count to match the right matrix row count.
- `solve2x2` requires a 2×2 matrix, a size-2 vector, and a non-zero determinant.

## Usage Examples

```aivi
use aivi.linear_algebra

v1 = { size: 2, data: [1.0, 2.0] }
v2 = { size: 2, data: [3.0, 4.0] }

alignment = dot v1 v2

identity2 = { rows: 2, cols: 2, data: [1.0, 0.0, 0.0, 1.0] }
transform = { rows: 2, cols: 2, data: [2.0, 3.0, 4.0, 5.0] }

composed = matMul identity2 transform

system = { rows: 2, cols: 2, data: [1.0, 1.0, 1.0, -1.0] }
rhs = { size: 2, data: [3.0, 1.0] }

solution = solve2x2 system rhs
```

In this example, `alignment` is `11.0`, `composed.data` stays `[2.0, 3.0, 4.0, 5.0]`, and `solution.data` becomes `[2.0, 1.0]`.
These cases are covered by `integration-tests/stdlib/aivi/linear_algebra/linear_algebra.aivi`.

## See also

- [`aivi.vector`](./vector.md) for fixed-size vector types such as `Vec2`, `Vec3`, and `Vec4`
- [`aivi.matrix`](./matrix.md) for fixed-size transform matrices and the `×` operator
