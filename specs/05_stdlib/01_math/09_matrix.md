# Matrix Domain

The `Matrix` domain provides grids of numbers (`Mat3`, `Mat4`) used primarily for **Transformations**.

Think of a Matrix as a "teleporter instruction set" for points. A single 4x4 grid can bundle up a complex recipe of movements: "Rotate 30 degrees, scale up by 200%, and move 5 units left."

Manually calculating the new position of a 3D point after it's been rotated, moved, and scaled is incredibly complex algebra. Matrices simplify this to `Point * Matrix`. They are the mathematical engine behind every 3D game and renderer.

## Overview

```aivi
use aivi.matrix (Mat4)

// A generic "identity" matrix (does nothing)
m = Mat4.identity()

// Create a instruction to move 10 units X
translation = Mat4.translate(10.0, 0.0, 0.0)

// Combine them
m_prime = m * translation
```


## Features

```aivi
Mat2 = { m00: Float, m01: Float, m10: Float, m11: Float }
Mat3 = {
  m00: Float, m01: Float, m02: Float,
  m10: Float, m11: Float, m12: Float,
  m20: Float, m21: Float, m22: Float
}
Mat4 = {
  m00: Float, m01: Float, m02: Float, m03: Float,
  m10: Float, m11: Float, m12: Float, m13: Float,
  m20: Float, m21: Float, m22: Float, m23: Float,
  m30: Float, m31: Float, m32: Float, m33: Float
}

Scalar = Float
```

## Domain Definition

```aivi
domain Matrix over Mat2 = {
  (+) : Mat2 -> Mat2 -> Mat2
  (+) a b = {
    m00: a.m00 + b.m00, m01: a.m01 + b.m01,
    m10: a.m10 + b.m10, m11: a.m11 + b.m11
  }
  
  (-) : Mat2 -> Mat2 -> Mat2
  (-) a b = {
    m00: a.m00 - b.m00, m01: a.m01 - b.m01,
    m10: a.m10 - b.m10, m11: a.m11 - b.m11
  }
  
  (*) : Mat2 -> Scalar -> Mat2
  (*) m s = {
    m00: m.m00 * s, m01: m.m01 * s,
    m10: m.m10 * s, m11: m.m11 * s
  }
}

domain Matrix over Mat3 = {
  (+) : Mat3 -> Mat3 -> Mat3
  (+) a b = {
    m00: a.m00 + b.m00, m01: a.m01 + b.m01, m02: a.m02 + b.m02,
    m10: a.m10 + b.m10, m11: a.m11 + b.m11, m12: a.m12 + b.m12,
    m20: a.m20 + b.m20, m21: a.m21 + b.m21, m22: a.m22 + b.m22
  }
  
  (-) : Mat3 -> Mat3 -> Mat3
  (-) a b = {
    m00: a.m00 - b.m00, m01: a.m01 - b.m01, m02: a.m02 - b.m02,
    m10: a.m10 - b.m10, m11: a.m11 - b.m11, m12: a.m12 - b.m12,
    m20: a.m20 - b.m20, m21: a.m21 - b.m21, m22: a.m22 - b.m22
  }
  
  (*) : Mat3 -> Scalar -> Mat3
  (*) m s = {
    m00: m.m00 * s, m01: m.m01 * s, m02: m.m02 * s,
    m10: m.m10 * s, m11: m.m11 * s, m12: m.m12 * s,
    m20: m.m20 * s, m21: m.m21 * s, m22: m.m22 * s
  }
}

domain Matrix over Mat4 = {
  (+) : Mat4 -> Mat4 -> Mat4
  (+) a b = {
    m00: a.m00 + b.m00, m01: a.m01 + b.m01, m02: a.m02 + b.m02, m03: a.m03 + b.m03,
    m10: a.m10 + b.m10, m11: a.m11 + b.m11, m12: a.m12 + b.m12, m13: a.m13 + b.m13,
    m20: a.m20 + b.m20, m21: a.m21 + b.m21, m22: a.m22 + b.m22, m23: a.m23 + b.m23,
    m30: a.m30 + b.m30, m31: a.m31 + b.m31, m32: a.m32 + b.m32, m33: a.m33 + b.m33
  }
  
  (-) : Mat4 -> Mat4 -> Mat4
  (-) a b = {
    m00: a.m00 - b.m00, m01: a.m01 - b.m01, m02: a.m02 - b.m02, m03: a.m03 - b.m03,
    m10: a.m10 - b.m10, m11: a.m11 - b.m11, m12: a.m12 - b.m12, m13: a.m13 - b.m13,
    m20: a.m20 - b.m20, m21: a.m21 - b.m21, m22: a.m22 - b.m22, m23: a.m23 - b.m23,
    m30: a.m30 - b.m30, m31: a.m31 - b.m31, m32: a.m32 - b.m32, m33: a.m33 - b.m33
  }
  
  (*) : Mat4 -> Scalar -> Mat4
  (*) m s = {
    m00: m.m00 * s, m01: m.m01 * s, m02: m.m02 * s, m03: m.m03 * s,
    m10: m.m10 * s, m11: m.m11 * s, m12: m.m12 * s, m13: m.m13 * s,
    m20: m.m20 * s, m21: m.m21 * s, m22: m.m22 * s, m23: m.m23 * s,
    m30: m.m30 * s, m31: m.m31 * s, m32: m.m32 * s, m33: m.m33 * s
  }
}
```

## Helper Functions

| Function | Explanation |
| --- | --- |
| **identity2**<br><pre><code>`Mat2`</code></pre> | Identity matrix for 2x2. |
| **identity3**<br><pre><code>`Mat3`</code></pre> | Identity matrix for 3x3. |
| **identity4**<br><pre><code>`Mat4`</code></pre> | Identity matrix for 4x4. |
| **transpose2** m<br><pre><code>`Mat2 -> Mat2`</code></pre> | Flips rows and columns of a 2x2. |
| **transpose3** m<br><pre><code>`Mat3 -> Mat3`</code></pre> | Flips rows and columns of a 3x3. |
| **transpose4** m<br><pre><code>`Mat4 -> Mat4`</code></pre> | Flips rows and columns of a 4x4. |
| **multiply2** a b<br><pre><code>`Mat2 -> Mat2 -> Mat2`</code></pre> | Multiplies two 2x2 matrices. |
| **multiply3** a b<br><pre><code>`Mat3 -> Mat3 -> Mat3`</code></pre> | Multiplies two 3x3 matrices. |
| **multiply4** a b<br><pre><code>`Mat4 -> Mat4 -> Mat4`</code></pre> | Multiplies two 4x4 matrices. |

## Usage Examples

```aivi
use aivi.matrix

scale2 = { m00: 2.0, m01: 0.0, m10: 0.0, m11: 2.0 }
rotate2 = { m00: 0.0, m01: -1.0, m10: 1.0, m11: 0.0 }

combined = multiply2 scale2 rotate2
unit = combined * 0.5
```
