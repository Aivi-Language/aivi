# Matrix Domain

The `Matrix` domain provides grids of numbers (`Mat3`, `Mat4`) used primarily for **Transformations**.

Think of a Matrix as a "teleporter instruction set" for points. A single 4x4 grid can bundle up a complex recipe of movements: "Rotate 30 degrees, scale up by 200%, and move 5 units left."

Manually calculating the new position of a 3D point after it's been rotated, moved, and scaled is incredibly complex algebra. Matrices simplify this to `Point * Matrix`. They are the mathematical engine behind every 3D game and renderer.

## Overview

<<< ../../snippets/from_md/05_stdlib/01_math/09_matrix/block_01.aivi{aivi}


## Features

<<< ../../snippets/from_md/05_stdlib/01_math/09_matrix/block_02.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/05_stdlib/01_math/09_matrix/block_03.aivi{aivi}

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

<<< ../../snippets/from_md/05_stdlib/01_math/09_matrix/block_04.aivi{aivi}
