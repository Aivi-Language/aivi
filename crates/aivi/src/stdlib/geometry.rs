pub const MODULE_NAME: &str = "aivi.geometry";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.geometry
export Point2, Point3, Line2, Segment2, Ray3, Polygon
export point2, point3, line2, segment2, ray3
export distance, midpoint, area
export Geometry

use aivi
use aivi.math (sqrt, abs)

Point2 = { x: Float, y: Float }
Point3 = { x: Float, y: Float, z: Float }
Line2 = { origin: Point2, direction: Point2 }
Segment2 = { start: Point2, end: Point2 }
Ray3 = { origin: Point3, dir: Point3 }
Polygon = { vertices: List Point2 }

point2 : Float -> Float -> Point2
point2 = x y => { x: x, y: y }

point3 : Float -> Float -> Float -> Point3
point3 = x y z => { x: x, y: y, z: z }

line2 : Float -> Float -> Float -> Float -> Line2
line2 = ox oy dx dy => { origin: { x: ox, y: oy }, direction: { x: dx, y: dy } }

segment2 : Float -> Float -> Float -> Float -> Segment2
segment2 = sx sy ex ey => { start: { x: sx, y: sy }, end: { x: ex, y: ey } }

ray3 : Float -> Float -> Float -> Float -> Float -> Float -> Ray3
ray3 = ox oy oz dx dy dz => { origin: { x: ox, y: oy, z: oz }, dir: { x: dx, y: dy, z: dz } }

domain Geometry over Point2 = {
  (+) : Point2 -> Point2 -> Point2
  (+) = a b => { x: a.x + b.x, y: a.y + b.y }

  (-) : Point2 -> Point2 -> Point2
  (-) = a b => { x: a.x - b.x, y: a.y - b.y }
}

domain Geometry over Point3 = {
  (+) : Point3 -> Point3 -> Point3
  (+) = a b => { x: a.x + b.x, y: a.y + b.y, z: a.z + b.z }

  (-) : Point3 -> Point3 -> Point3
  (-) = a b => { x: a.x - b.x, y: a.y - b.y, z: a.z - b.z }
}

distance : Point2 -> Point2 -> Float
distance = a b => {
  dx = a.x - b.x
  dy = a.y - b.y
  sqrt (dx * dx + dy * dy)
}

midpoint : Segment2 -> Point2
midpoint = seg => { x: (seg.start.x + seg.end.x) / 2.0, y: (seg.start.y + seg.end.y) / 2.0 }

areaLoop : Point2 -> Point2 -> List Point2 -> Float -> Float
areaLoop = first prev rest acc => rest match
  | [] => acc + (prev.x * first.y - first.x * prev.y)
  | [p, ...ps] => areaLoop first p ps (acc + (prev.x * p.y - p.x * prev.y))

area : Polygon -> Float
area = poly => poly.vertices match
  | [] => 0.0
  | [first, ...rest] => abs (areaLoop first first rest 0.0) / 2.0"#;
