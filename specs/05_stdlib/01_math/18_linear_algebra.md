# Linear Algebra Domain

Advanced solvers for **Systems of Equations**.

## What is this?

While the `Vector` and `Matrix` domains handle 3D graphics, this domain is for "hard" science and engineering.
It allows you to solve problems like: "If 3x + 2y = 10 and x - y = 5, what are x and y?"

In engineering, these systems can have thousands of variables (e.g., simulating heat flow across a metal plate).

## Why this exists

Writing a solver for `Ax = b` (Gaussian elimination or LU decomposition) is difficult to make numerically stable. This domain wraps optimized, industrial-grade linear algebra routines (like LAPACK) so you can solve complex systems safely.

## Overview

```aivi
import aivi.std.math.linalg use { solve, eigen }

// Matrix A and Vector b
// Solve for x in: Ax = b
// (Finds the inputs that produce the known output)
let x = solve(A, b)
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