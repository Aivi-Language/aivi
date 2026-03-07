# Linear Algebra Domain

<!-- quick-info: {"kind":"module","name":"aivi.linearAlgebra"} -->
The `LinearAlgebra` domain collects helpers for vector and matrix calculations that show up in simulation, graphics, optimization, and scientific code.
Use it when you want algebraic operations such as dot products, matrix multiplication, or solving small linear systems as first-class library functions.
<!-- /quick-info -->
<div class="import-badge">use aivi.linearAlgebra<span class="domain-badge">domain</span></div>

If `aivi.vector` and `aivi.matrix` are about data shapes, `aivi.linearAlgebra` is about the calculations you perform with those shapes.

## Start here

Choose between the related domains like this:

- use `aivi.vector` when you mainly need vector values and helper operations on one vector at a time,
- use `aivi.matrix` when you mainly need matrix values and transform-oriented helpers,
- use `aivi.linearAlgebra` when the key job is combining vectors or matrices mathematically.

## What it is for

This domain is useful when you need to:

- compare directions with a dot product
- combine transformations with matrix multiplication
- solve small systems like `m * x = v`

## Overview

<<< ../../snippets/from_md/stdlib/math/linear_algebra/overview.aivi{aivi}

## Features

<<< ../../snippets/from_md/stdlib/math/linear_algebra/features.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/math/linear_algebra/domain_definition.aivi{aivi}

## Core helpers

| Function | What it does |
| --- | --- |
| **dot** a b<br><code>Vec -> Vec -> Float</code> | Returns the dot product of two vectors. A dot product is a single number that tells you how aligned two directions are. |
| **matMul** a b<br><code>Mat -> Mat -> Mat</code> | Multiplies matrices by combining rows of `a` with columns of `b`. Use this to compose transforms or other matrix-to-matrix calculations. |
| **solve2x2** m v<br><code>Mat -> Vec -> Vec</code> | Solves the 2×2 system `m * x = v`. This is a handy small-system helper when you want to recover the unknown vector `x`. |

## Usage Examples

<<< ../../snippets/from_md/stdlib/math/linear_algebra/usage_examples.aivi{aivi}
