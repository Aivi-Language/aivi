# Geometry Domain

The `Geometry` domain creates shapes (`Sphere`, `Ray`, `Rect`) and checks if they touch.

This is the "physical" side of math. While `Vector` handles movement, `Geometry` handles **stuff**.
*   "Did I click the button?" (Point vs Rect)
*   "Did the bullet hit the player?" (Ray vs Cylinder)
*   "Is the tank inside the base?" (Point vs Polygon)

Almost every visual application needs to know when two things collide. This domain gives you standard shapes and highly optimized algorithms to check for intersections instantly.

## Overview

```aivi
use aivi.std.math.geometry (Ray, Sphere, intersect)

// A ray firing forwards from origin
ray = Ray(origin: {x:0, y:0, z:0}, dir: {x:0, y:0, z:1})

// A sphere 5 units away
sphere = Sphere(center: {x:0, y:0, z:5}, radius: 1.0)

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
