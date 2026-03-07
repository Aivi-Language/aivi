# Linear Algebra Domain

<!-- quick-info: {"kind":"module","name":"aivi.linearAlgebra"} -->
The `LinearAlgebra` domain collects helpers for vector and matrix calculations that show up in simulation, graphics, optimization, and scientific code.
Use it when you want algebraic operations such as dot products, matrix multiplication, or solving small linear systems as first-class library functions.
<!-- /quick-info -->
<div class="import-badge">use aivi.linearAlgebra<span class="domain-badge">domain</span></div>

If `aivi.vector` and `aivi.matrix` are about data shapes, `aivi.linearAlgebra` is about the calculations you perform with those shapes.

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
| **dot** a b<br><code>Vec -> Vec -> Float</code> | Returns the dot product of two vectors. |
| **matMul** a b<br><code>Mat -> Mat -> Mat</code> | Multiplies matrices by combining rows of `a` with columns of `b`. |
| **solve2x2** m v<br><code>Mat -> Vec -> Vec</code> | Solves the 2×2 system `m * x = v`. |

## Usage Examples

<<< ../../snippets/from_md/stdlib/math/linear_algebra/usage_examples.aivi{aivi}
