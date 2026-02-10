# Geometry Domain

The `Geometry` domain creates shapes (`Sphere`, `Ray`, `Rect`) and checks if they touch.

This is the "physical" side of math. While `Vector` handles movement, `Geometry` handles **stuff**.
*   "Did I click the button?" (Point vs Rect)
*   "Did the bullet hit the player?" (Ray vs Cylinder)
*   "Is the tank inside the base?" (Point vs Polygon)

Almost every visual application needs to know when two things collide. This domain gives you standard shapes and highly optimized algorithms to check for intersections instantly.

## Overview

```aivi
use aivi.geometry (Ray, Sphere, intersect)

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

| Function | Explanation |
| --- | --- |
| **distance** a b<br><pre><code>`Point2 -> Point2 -> Float`</code></pre> | Returns the Euclidean distance between two 2D points. |
| **midpoint** segment<br><pre><code>`Segment2 -> Point2`</code></pre> | Returns the center point of a line segment. |
| **area** polygon<br><pre><code>`Polygon -> Float`</code></pre> | Returns the signed area (positive for counter-clockwise winding). |

## Usage Examples

```aivi
use aivi.geometry

p1 = { x: 0.0, y: 0.0 }
p2 = { x: 3.0, y: 4.0 }

d = distance p1 p2
center = midpoint { start: p1, end: p2 }
```
