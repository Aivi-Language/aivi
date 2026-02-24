pub const MODULE_NAME: &str = "aivi.number.quaternion";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.number.quaternion
export fromAxisAngle, conjugate, magnitude, normalize
export Quaternion

use aivi
use aivi.math (sqrt, sin, cos, radians)

Quaternion = { w: Float, x: Float, y: Float, z: Float }

fromAxisAngle : { x: Float, y: Float, z: Float } -> Float -> Quaternion
fromAxisAngle = axis theta => {
  axisLen = sqrt (axis.x * axis.x + axis.y * axis.y + axis.z * axis.z)
  axisUnit = if axisLen == 0.0 then { x: 0.0, y: 0.0, z: 0.0 } else {
    x: axis.x / axisLen
    y: axis.y / axisLen
    z: axis.z / axisLen
  }
  half = theta / 2.0
  s = sin (radians half)
  c = cos (radians half)
  { w: c, x: axisUnit.x * s, y: axisUnit.y * s, z: axisUnit.z * s }
}

conjugate : Quaternion -> Quaternion
conjugate = q => { w: q.w, x: -q.x, y: -q.y, z: -q.z }

magnitude : Quaternion -> Float
magnitude = q => sqrt (q.w * q.w + q.x * q.x + q.y * q.y + q.z * q.z)

normalize : Quaternion -> Quaternion
normalize = q => {
  m = magnitude q
  if m == 0.0 then q else q / m
}

domain Quaternion over Quaternion = {
  (+) : Quaternion -> Quaternion -> Quaternion
  (+) = a b => { w: a.w + b.w, x: a.x + b.x, y: a.y + b.y, z: a.z + b.z }

  (-) : Quaternion -> Quaternion -> Quaternion
  (-) = a b => { w: a.w - b.w, x: a.x - b.x, y: a.y - b.y, z: a.z - b.z }

  (*) : Quaternion -> Quaternion -> Quaternion
  (*) = a b => {
    w: a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z
    x: a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y
    y: a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x
    z: a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w
  }

  (/) : Quaternion -> Float -> Quaternion
  (/) = q s => { w: q.w / s, x: q.x / s, y: q.y / s, z: q.z / s }
}"#;
