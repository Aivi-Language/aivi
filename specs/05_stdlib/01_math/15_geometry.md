# Geometry Domain

The `Geometry` domain creates shapes (`Sphere`, `Ray`, `Rect`) and checks if they touch.

This is the "physical" side of math. While `Vector` handles movement, `Geometry` handles **stuff**.
*   "Did I click the button?" (Point vs Rect)
*   "Did the bullet hit the player?" (Ray vs Cylinder)
*   "Is the tank inside the base?" (Point vs Polygon)

Almost every visual application needs to know when two things collide. This domain gives you standard shapes and highly optimized algorithms to check for intersections instantly.

## Overview

<<< ../../snippets/from_md/05_stdlib/01_math/15_geometry/block_01.aivi{aivi}


## Features

<<< ../../snippets/from_md/05_stdlib/01_math/15_geometry/block_02.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/05_stdlib/01_math/15_geometry/block_03.aivi{aivi}

## Helper Functions

| Function | Explanation |
| --- | --- |
| **distance** a b<br><pre><code>`Point2 -> Point2 -> Float`</code></pre> | Returns the Euclidean distance between two 2D points. |
| **midpoint** segment<br><pre><code>`Segment2 -> Point2`</code></pre> | Returns the center point of a line segment. |
| **area** polygon<br><pre><code>`Polygon -> Float`</code></pre> | Returns the signed area (positive for counter-clockwise winding). |

## Usage Examples

<<< ../../snippets/from_md/05_stdlib/01_math/15_geometry/block_04.aivi{aivi}
