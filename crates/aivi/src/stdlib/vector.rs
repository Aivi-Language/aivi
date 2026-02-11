pub const MODULE_NAME: &str = "aivi.vector";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.vector = {
  export Vec2, Vec3, Vec4
  export magnitude, normalize, dot, cross
  export domain Vector

  use aivi
  use aivi.math (sqrt)

  Vec2 = { x: Float, y: Float }
  Vec3 = { x: Float, y: Float, z: Float }
  Vec4 = { x: Float, y: Float, z: Float, w: Float }

  magnitude : Vec2 -> Float
  magnitude v = sqrt (v.x * v.x + v.y * v.y)

  normalize : Vec2 -> Vec2
  normalize v = {
    len = magnitude v
    if len == 0.0 then v else { x: v.x / len, y: v.y / len }
  }

  dot : Vec2 -> Vec2 -> Float
  dot a b = a.x * b.x + a.y * b.y

  cross : Vec3 -> Vec3 -> Vec3
  cross a b = {
    x: a.y * b.z - a.z * b.y
    y: a.z * b.x - a.x * b.z
    z: a.x * b.y - a.y * b.x
  }

  domain Vector over Vec2 = {
    (+) : Vec2 -> Vec2 -> Vec2
    (+) v1 v2 = { x: v1.x + v2.x, y: v1.y + v2.y }

    (-) : Vec2 -> Vec2 -> Vec2
    (-) v1 v2 = { x: v1.x - v2.x, y: v1.y - v2.y }

    (*) : Vec2 -> Float -> Vec2
    (*) v s = { x: v.x * s, y: v.y * s }

    (/) : Vec2 -> Float -> Vec2
    (/) v s = { x: v.x / s, y: v.y / s }
  }

  domain Vector over Vec3 = {
    (+) : Vec3 -> Vec3 -> Vec3
    (+) v1 v2 = { x: v1.x + v2.x, y: v1.y + v2.y, z: v1.z + v2.z }

    (-) : Vec3 -> Vec3 -> Vec3
    (-) v1 v2 = { x: v1.x - v2.x, y: v1.y - v2.y, z: v1.z - v2.z }

    (*) : Vec3 -> Float -> Vec3
    (*) v s = { x: v.x * s, y: v.y * s, z: v.z * s }

    (/) : Vec3 -> Float -> Vec3
    (/) v s = { x: v.x / s, y: v.y / s, z: v.z / s }
  }
}
"#;
