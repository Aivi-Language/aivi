# Linear Algebra Domain

<!-- quick-info: {"kind":"module","name":"aivi.linearAlgebra"} -->
The `LinearAlgebra` domain solves massive **Systems of Equations**.

While `Vector` and `Matrix` are for 3D graphics, this domain is for "hard" science and engineering. It answers questions like: "If `3x + 2y = 10` and `x - y = 5`, what are `x` and `y`?"... but for systems with *thousands* of variables.

Whether you're simulating heat flow across a computer chip, calculating structural loads on a bridge, or training a neural network, you are solving systems of linear equations. This domain wraps industrial-grade solvers (like LAPACK) to do the heavy lifting for you.

<!-- /quick-info -->
<div class="import-badge">use aivi.linearAlgebra<span class="domain-badge">domain</span></div>

## Overview

<<< ../../snippets/from_md/stdlib/math/linear_algebra/overview.aivi{aivi}


## Features

<<< ../../snippets/from_md/stdlib/math/linear_algebra/features.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/math/linear_algebra/domain_definition.aivi{aivi}

## Helper Functions

| Function | Explanation |
| --- | --- |
| **dot** a b<br><code>Vec -> Vec -> Float</code> | Returns the dot product of two vectors. |
| **matMul** a b<br><code>Mat -> Mat -> Mat</code> | Multiplies matrices (rows of `a` by columns of `b`). |
| **solve2x2** m v<br><code>Mat -> Vec -> Vec</code> | Solves the system `m * x = v`. |

## Usage Examples

<<< ../../snippets/from_md/stdlib/math/linear_algebra/usage_examples.aivi{aivi}
