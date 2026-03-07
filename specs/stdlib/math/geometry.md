# Geometry Domain

<!-- quick-info: {"kind":"module","name":"aivi.geometry"} -->
The `Geometry` domain provides common shape types and spatial queries for tasks such as hit testing, collision checks, and distance measurements.
Use it when your program needs to work with points, segments, rays, polygons, or volumes as meaningful shapes instead of loose numeric records.
<!-- /quick-info -->
<div class="import-badge">use aivi.geometry<span class="domain-badge">domain</span></div>

`aivi.geometry` pairs naturally with `aivi.vector`: vectors describe direction and movement, while geometry describes shapes and the relationships between them.

## What it is for

Reach for this domain when you need to answer questions such as:

- “Did the pointer land inside this rectangle?”
- “How far apart are these two points?”
- “Does this ray hit the shape in front of it?”

## Overview

<<< ../../snippets/from_md/stdlib/math/geometry/overview.aivi{aivi}

## Features

<<< ../../snippets/from_md/stdlib/math/geometry/features.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/math/geometry/domain_definition.aivi{aivi}

## Convenience constructors

You can always build values with full record syntax, but the short constructors are easier to read in examples, tests, and small calculations.

<<< ../../snippets/from_md/stdlib/math/geometry/short_constructors.aivi{aivi}

| Constructor | Type | Equivalent |
| --- | --- | --- |
| `point2 x y` | `Float -> Float -> Point2` | `{ x: x, y: y }` |
| `point3 x y z` | `Float -> Float -> Float -> Point3` | `{ x: x, y: y, z: z }` |
| `line2 ox oy dx dy` | `Float -> Float -> Float -> Float -> Line2` | `{ origin: point2 ox oy, direction: point2 dx dy }` |
| `segment2 x1 y1 x2 y2` | `Float -> ... -> Segment2` | `{ start: point2 x1 y1, end: point2 x2 y2 }` |
| `ray3 ox oy oz dx dy dz` | `Float -> ... -> Ray` | `{ origin: point3 ox oy oz, dir: point3 dx dy dz }` |

## Common helpers

| Function | What it does |
| --- | --- |
| **distance** a b<br><code>Point2 -> Point2 -> Float</code> | Returns the Euclidean distance between two 2D points. |
| **distance** a b<br><code>Point3 -> Point3 -> Float</code> | Returns the Euclidean distance between two 3D points. |
| **midpoint** segment<br><code>Segment2 -> Point2</code> | Returns the point halfway between a segment's start and end. |
| **area** polygon<br><code>Polygon -> Float</code> | Returns the signed polygon area; counter-clockwise winding is positive. |
| **intersect** ray shape<br><code>Ray -> Shape -> Bool</code> | Returns `True` when the ray intersects the given shape. |

## Usage Examples

The examples below show the typical flow: create shape values, then call helpers such as `distance`, `area`, or `intersect` to ask geometric questions.

<<< ../../snippets/from_md/stdlib/math/geometry/usage_examples.aivi{aivi}
