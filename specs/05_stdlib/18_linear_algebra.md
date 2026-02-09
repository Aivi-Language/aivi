# Standard Library: Linear Algebra Domain

## Module

```aivi
module aivi.std.linear_algebra = {
  export domain LinearAlgebra
  export Vec, Mat
  export dot, matMul, solve2x2
}
```

## Types

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