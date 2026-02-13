# Linear Algebra Domain

The `LinearAlgebra` domain solves massive **Systems of Equations**.

While `Vector` and `Matrix` are for 3D graphics, this domain is for "hard" science and engineering. It answers questions like: "If `3x + 2y = 10` and `x - y = 5`, what are `x` and `y`?"... but for systems with *thousands* of variables.

Whether you're simulating heat flow across a computer chip, calculating structural loads on a bridge, or training a neural network, you are solving systems of linear equations. This domain wraps industrial-grade solvers (like LAPACK) to do the heavy lifting for you.

## Overview

<<< ../../snippets/from_md/05_stdlib/01_math/18_linear_algebra/block_01.aivi{aivi}


## Features

<<< ../../snippets/from_md/05_stdlib/01_math/18_linear_algebra/block_02.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/05_stdlib/01_math/18_linear_algebra/block_03.aivi{aivi}

## Helper Functions

| Function | Explanation |
| --- | --- |
| **dot** a b<br><pre><code>`Vec -> Vec -> Float`</code></pre> | Returns the dot product of two vectors. |
| **matMul** a b<br><pre><code>`Mat -> Mat -> Mat`</code></pre> | Multiplies matrices (rows of `a` by columns of `b`). |
| **solve2x2** m v<br><pre><code>`Mat -> Vec -> Vec`</code></pre> | Solves the system `m * x = v`. |

## Usage Examples

<<< ../../snippets/from_md/05_stdlib/01_math/18_linear_algebra/block_04.aivi{aivi}
