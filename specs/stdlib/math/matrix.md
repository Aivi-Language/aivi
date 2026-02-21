# Matrix Domain

<!-- quick-info: {"kind":"module","name":"aivi.matrix"} -->
The `Matrix` domain provides grids of numbers (`Mat3`, `Mat4`) used primarily for **Transformations**.

Think of a Matrix as a "teleporter instruction set" for points. A single 4x4 grid can bundle up a complex recipe of movements: "Rotate 30 degrees, scale up by 200%, and move 5 units left."

Manually calculating the new position of a 3D point after it's been rotated, moved, and scaled is incredibly complex algebra. Matrices simplify this to `Point * Matrix`. They are the mathematical engine behind every 3D game and renderer.

<!-- /quick-info -->
<div class="import-badge">use aivi.matrix<span class="domain-badge">domain</span></div>

## Overview

<<< ../../snippets/from_md/stdlib/math/matrix/sigil_constructors.aivi{aivi}


## Features

<<< ../../snippets/from_md/stdlib/math/matrix/features.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/math/matrix/domain_definition.aivi{aivi}

## `×` Operator Overloads

The `Matrix` domain provides two overloads of `×` per carrier, selected by the RHS type (see [Domains: Within-Domain Operator Overloads](../../syntax/domains.md#within-domain-operator-overloads-rhs-typed)):

| Expression | Resolved as | Returns |
| --- | --- | --- |
| `mat2 × mat2` | `multiply2 mat2 mat2` | `Mat2` |
| `mat2 × vec2` | `transform2 mat2 vec2` | `Vec2` |
| `mat3 × mat3` | `multiply3 mat3 mat3` | `Mat3` |
| `mat3 × vec3` | `transform3 mat3 vec3` | `Vec3` |
| `mat4 × mat4` | `multiply4 mat4 mat4` | `Mat4` |
| `mat4 × vec4` | `transform4 mat4 vec4` | `Vec4` |

**Convention**: `×` is for structural products; `*` remains for scalar scaling.

Requires `use aivi.matrix (domain Matrix)` (or `use aivi.matrix`) and `use aivi.vector` for the `Vec*` types to be in scope.

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

## Sigil Constructors

For concise matrix literals, use the `~mat` structured sigil:

<<< ../../snippets/from_md/stdlib/math/matrix/sigil_constructors.aivi{aivi}


Rows are separated by newlines; columns by spaces (any whitespace). The formatter aligns columns for readability. The sigil infers `Mat2`, `Mat3`, or `Mat4` from the row/column count.

## Usage Examples

<<< ../../snippets/from_md/stdlib/math/matrix/usage_examples.aivi{aivi}
