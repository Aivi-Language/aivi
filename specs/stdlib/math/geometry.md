# Geometry Domain

<!-- quick-info: {"kind":"module","name":"aivi.geometry"} -->
The `Geometry` domain provides small geometry types and helpers for coordinate-based work in v0.1.
Use it when you want named points, segments, rays, and polygons plus basic operations such as point arithmetic, 2D distance, segment midpoints, and polygon area.
<!-- /quick-info -->
<div class="import-badge">use aivi.geometry<span class="domain-badge">domain</span></div>

`aivi.geometry` pairs naturally with [`aivi.vector`](vector.md) and [`aivi.matrix`](matrix.md): geometry gives names to shapes and positions, while vectors and matrices handle reusable directions and transforms.

## What it is for

Reach for this domain when you need to:

- model 2D or 3D positions with explicit `Point2` and `Point3` types,
- represent line-like values such as `Line2`, `Segment2`, and `Ray3`,
- compute a 2D point-to-point distance or a segment midpoint,
- compute the area enclosed by a polygon's vertex list.

The current surface is intentionally small. `Ray3` is available as a data type, but ray-casting, hit-testing, and collision helpers are not part of `aivi.geometry` yet.

## Overview

<<< ../../snippets/from_md/stdlib/math/geometry/block_01.aivi{aivi}


`gap` evaluates to `5.0`, `mid` evaluates to `{ x: 3.0, y: 1.0 }`, and `size` evaluates to `6.0`.

## Features

| Item | Meaning |
| --- | --- |
| `Point2` / `Point3` | 2D and 3D points with named coordinates. |
| `Line2` | An infinite 2D line with an `origin` point and a `direction` stored as a `Point2`-shaped record. |
| `Segment2` | A finite 2D line segment with `start` and `end` points. |
| `Ray3` | A 3D ray with an `origin` point and a `dir` stored as a `Point3`-shaped record. |
| `Polygon` | A polygon described by `{ vertices: List Point2 }`. |
| `domain Geometry` | Overloads `+` and `-` for `Point2` and `Point3`. |

If you need vector-specific operations such as normalization, dot products, or matrix transforms, switch to [`aivi.vector`](vector.md) and [`aivi.matrix`](matrix.md).

## Domain Definition

If you are new to operator overloading in AIVI, see [Domains](../../syntax/domains.md) first. `aivi.geometry` defines `domain Geometry` for `Point2` and `Point3` only:

<<< ../../snippets/from_md/stdlib/math/geometry/block_02.aivi{aivi}


## Convenience constructors

You can always build values with full record syntax, but the short constructors are easier to read in examples, tests, and small calculations.

<<< ../../snippets/from_md/stdlib/math/geometry/block_03.aivi{aivi}


| Constructor | Type | Equivalent |
| --- | --- | --- |
| `point2 x y` | `Float -> Float -> Point2` | `{ x: x, y: y }` |
| `point3 x y z` | `Float -> Float -> Float -> Point3` | `{ x: x, y: y, z: z }` |
| `line2 ox oy dx dy` | `Float -> Float -> Float -> Float -> Line2` | `{ origin: point2 ox oy, direction: point2 dx dy }` |
| `segment2 x1 y1 x2 y2` | `Float -> Float -> Float -> Float -> Segment2` | `{ start: point2 x1 y1, end: point2 x2 y2 }` |
| `ray3 ox oy oz dx dy dz` | `Float -> Float -> Float -> Float -> Float -> Float -> Ray3` | `{ origin: point3 ox oy oz, dir: point3 dx dy dz }` |

## Common helpers

| Function | What it does |
| --- | --- |
| **distance** a b<br><code>Point2 -> Point2 -> Float</code> | Returns the Euclidean distance between two 2D points. |
| **midpoint** segment<br><code>Segment2 -> Point2</code> | Returns the point halfway between a segment's start and end. |
| **area** polygon<br><code>Polygon -> Float</code> | Returns the non-negative polygon area computed from the vertex loop. Clockwise and counter-clockwise vertex order both produce the same magnitude. |

## Usage Examples

The typical flow is: construct named geometry values, then call helpers to ask concrete questions about them.

<<< ../../snippets/from_md/stdlib/math/geometry/block_04.aivi{aivi}

