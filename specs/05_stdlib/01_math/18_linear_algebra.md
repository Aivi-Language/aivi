# Linear Algebra Domain

The `LinearAlgebra` domain solves massive **Systems of Equations**.

While `Vector` and `Matrix` are for 3D graphics, this domain is for "hard" science and engineering. It answers questions like: "If `3x + 2y = 10` and `x - y = 5`, what are `x` and `y`?"... but for systems with *thousands* of variables.

Whether you're simulating heat flow across a computer chip, calculating structural loads on a bridge, or training a neural network, you are solving systems of linear equations. This domain wraps industrial-grade solvers (like LAPACK) to do the heavy lifting for you.

## Overview

```aivi
use aivi.linalg (solve, eigen)

// Matrix A and Vector b
// Solve for x in: Ax = b
// (Finds the inputs that produce the known output)
x = solve(A, b)
```


## Features

```aivi
Vec = { size: Int, data: List Float }
Mat = { rows: Int, cols: Int, data: List Float }
```

## Domain Definition

```aivi
domain LinearAlgebra over Vec = {
  (+) : Vec -> Vec -> Vec
  (+) a b = { size: a.size, data: zipWith (+) a.data b.data }
  
  (-) : Vec -> Vec -> Vec
  (-) a b = { size: a.size, data: zipWith (-) a.data b.data }
  
  (*) : Vec -> Float -> Vec
  (*) v s = { size: v.size, data: map (\x -> x * s) v.data }
}
```

## Helper Functions

| Function | Explanation |
| --- | --- |
| **dot** a b<br><pre><code>`Vec -> Vec -> Float`</code></pre> | Returns the dot product of two vectors. |
| **matMul** a b<br><pre><code>`Mat -> Mat -> Mat`</code></pre> | Multiplies matrices (rows of `a` by columns of `b`). |
| **solve2x2** m v<br><pre><code>`Mat -> Vec -> Vec`</code></pre> | Solves the system `m * x = v`. |

## Usage Examples

```aivi
use aivi.linear_algebra

v1 = { size: 3, data: [1.0, 2.0, 3.0] }
v2 = { size: 3, data: [4.0, 5.0, 6.0] }

prod = dot v1 v2
```
