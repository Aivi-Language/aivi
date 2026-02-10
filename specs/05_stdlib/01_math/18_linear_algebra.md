# Linear Algebra Domain

The `LinearAlgebra` domain solves massive **Systems of Equations**.

While `Vector` and `Matrix` are for 3D graphics, this domain is for "hard" science and engineering. It answers questions like: "If `3x + 2y = 10` and `x - y = 5`, what are `x` and `y`?"... but for systems with *thousands* of variables.

Whether you're simulating heat flow across a computer chip, calculating structural loads on a bridge, or training a neural network, you are solving systems of linear equations. This domain wraps industrial-grade solvers (like LAPACK) to do the heavy lifting for you.

## Overview

```aivi
use aivi.std.math.linalg (solve, eigen)

// Matrix A and Vector b
// Solve for x in: Ax = b
// (Finds the inputs that produce the known output)
x = solve(A, b)
```

## Performance and Implementation (Rust)

Linear algebra is expected to be backed by established BLAS/LAPACK bindings. Prefer crates like `ndarray-linalg` or `nalgebra` with `blas-src`/`lapack-src` rather than custom solvers to ensure high performance and numerical stability.

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

```aivi
dot : Vec -> Vec -> Float
dot a b = sum (zipWith (*) a.data b.data)

matMul : Mat -> Mat -> Mat
matMul a b = { rows: a.rows, cols: b.cols, data: matMulRaw a b }

solve2x2 : Mat -> Vec -> Vec
solve2x2 m v = solve2x2Raw m v
```

## Usage Examples

```aivi
use aivi.std.linear_algebra

v1 = { size: 3, data: [1.0, 2.0, 3.0] }
v2 = { size: 3, data: [4.0, 5.0, 6.0] }

prod = dot v1 v2
```
