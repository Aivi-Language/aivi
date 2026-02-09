# Geometry Domain

Primitives for shapes (`Sphere`, `Ray`, `Rect`) and collision detection.

## What is this?

This domain deals with the "physical" side of math: shapes and where they are. It differs from the `Vector` domain (which handles raw direction/magnitude) by introducing **surfaces** and **volumes**.

Common questions this domain answers:
*   "Did the user click on this button?" (Point vs Rectangle overlap)
*   "Did the laser hit the enemy?" (Ray vs Sphere intersection)
*   "What is the center of this polygon?"

## Why this exists

Almost every visual application needs to check if two things touch. Providing standard definitions for shapes like `Ray` and `AABB` (Axis-Aligned Bounding Box) allows for highly optimized intersection code that powers games and UI hit-testing.

## Overview

```aivi
import aivi.std.math.geometry use { Ray, Sphere, intersect }

// A ray firing forwards from origin
let ray = Ray(origin: {x:0, y:0, z:0}, dir: {x:0, y:0, z:1})

// A sphere 5 units away
let sphere = Sphere(center: {x:0, y:0, z:5}, radius: 1.0)

if intersect(ray, sphere) {
    print("Hit!")
}
```

## Features

```aivi
Point2 = { x: Float, y: Float }
Point3 = { x: Float, y: Float, z: Float }
Line2 = { origin: Point2, direction: Point2 }
Segment2 = { start: Point2, end: Point2 }
Polygon = { vertices: List Point2 }
```

## Domain Definition

```aivi
domain Geometry over Point2 = {
  (+) : Point2 -> Point2 -> Point2
  (+) a b = { x: a.x + b.x, y: a.y + b.y }
  
  (-) : Point2 -> Point2 -> Point2
  (-) a b = { x: a.x - b.x, y: a.y - b.y }
}

domain Geometry over Point3 = {
  (+) : Point3 -> Point3 -> Point3
  (+) a b = { x: a.x + b.x, y: a.y + b.y, z: a.z + b.z }
  
  (-) : Point3 -> Point3 -> Point3
  (-) a b = { x: a.x - b.x, y: a.y - b.y, z: a.z - b.z }
}
```

## Helper Functions

```aivi
distance : Point2 -> Point2 -> Float
distance a b = sqrt ((a.x - b.x) * (a.x - b.x) + (a.y - b.y) * (a.y - b.y))

midpoint : Segment2 -> Point2
midpoint s = { x: (s.start.x + s.end.x) / 2.0, y: (s.start.y + s.end.y) / 2.0 }

area : Polygon -> Float
area poly = polygonArea poly.vertices
```

## Usage Examples

```aivi
use aivi.std.geometry

p1 = { x: 0.0, y: 0.0 }
p2 = { x: 3.0, y: 4.0 }

d = distance p1 p2
center = midpoint { start: p1, end: p2 }
```