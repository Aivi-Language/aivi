````markdown
# Geometry Domain

<!-- quick-info: {"kind":"module","name":"aivi.geometry"} -->
The `Geometry` domain creates shapes (`Sphere`, `Ray`, `Rect`) and checks if they touch.

This is the "physical" side of math. While `Vector` handles movement, `Geometry` handles **stuff**.
*   "Did I click the button?" (Point vs Rect)
*   "Did the bullet hit the player?" (Ray vs Cylinder)
*   "Is the tank inside the base?" (Point vs Polygon)

Almost every visual application needs to know when two things collide. This domain gives you standard shapes and highly optimized algorithms to check for intersections instantly.

<!-- /quick-info -->
## Overview

<<< ../../snippets/from_md/05_stdlib/01_math/15_geometry/block_01.aivi{aivi}


## Features

<<< ../../snippets/from_md/05_stdlib/01_math/15_geometry/block_02.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/05_stdlib/01_math/15_geometry/block_03.aivi{aivi}

## Short Constructors

Instead of verbose nested record literals, use convenience constructors:

<<< ../../snippets/from_md/05_stdlib/01_math/15_geometry/block_05.aivi{aivi}

| Constructor | Type | Equivalent |
| --- | --- | --- |
| `point2 x y` | `Float -> Float -> Point2` | `{ x: x, y: y }` |
| `point3 x y z` | `Float -> Float -> Float -> Point3` | `{ x: x, y: y, z: z }` |
| `line2 ox oy dx dy` | `Float -> Float -> Float -> Float -> Line2` | `{ origin: point2 ox oy, direction: point2 dx dy }` |
| `segment2 x1 y1 x2 y2` | `Float -> ... -> Segment2` | `{ start: point2 x1 y1, end: point2 x2 y2 }` |
| `ray3 ox oy oz dx dy dz` | `Float -> ... -> Ray` | `{ origin: point3 ox oy oz, dir: point3 dx dy dz }` |

## Helper Functions

| Function | Explanation |
| --- | --- |
| **distance** a b<br><pre><code>`Point2 -> Point2 -> Float`</code></pre> | Returns the Euclidean distance between two 2D points. |
| **distance** a b<br><pre><code>`Point3 -> Point3 -> Float`</code></pre> | Returns the Euclidean distance between two 3D points. |
| **midpoint** segment<br><pre><code>`Segment2 -> Point2`</code></pre> | Returns the center point of a line segment. |
| **area** polygon<br><pre><code>`Polygon -> Float`</code></pre> | Returns the signed area (positive for counter-clockwise winding). |
| **intersect** ray shape<br><pre><code>`Ray -> Shape -> Bool`</code></pre> | Tests whether a ray intersects a shape. |

## Usage Examples

<<< ../../snippets/from_md/05_stdlib/01_math/15_geometry/block_04.aivi{aivi}

````
