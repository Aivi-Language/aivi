pub const MODULE_NAME: &str = "aivi.geometry";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.geometry = {
  export Point2, Point3, Line2, Segment2, Polygon
  export distance, midpoint, area
  export domain Geometry

  use aivi
  use aivi.math (sqrt, abs)

  Point2 = { x: Float, y: Float }
  Point3 = { x: Float, y: Float, z: Float }
  Line2 = { origin: Point2, direction: Point2 }
  Segment2 = { start: Point2, end: Point2 }
  Polygon = { vertices: List Point2 }

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

  distance : Point2 -> Point2 -> Float
  distance a b = {
    dx = a.x - b.x
    dy = a.y - b.y
    sqrt (dx * dx + dy * dy)
  }

  midpoint : Segment2 -> Point2
  midpoint seg = { x: (seg.start.x + seg.end.x) / 2.0, y: (seg.start.y + seg.end.y) / 2.0 }

  areaLoop : Point2 -> Point2 -> List Point2 -> Float -> Float
  areaLoop first prev rest acc = rest ?
    | [] => acc + (prev.x * first.y - first.x * prev.y)
    | [p, ...ps] => areaLoop first p ps (acc + (prev.x * p.y - p.x * prev.y))

  area : Polygon -> Float
  area poly = poly.vertices ?
    | [] => 0.0
    | [first, ...rest] => abs (areaLoop first first rest 0.0) / 2.0
}
"#;
