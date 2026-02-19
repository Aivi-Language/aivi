pub const MODULE_NAME: &str = "aivi.vector";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.vector
export Vec2, Vec3, Vec4
export vec2, vec3, vec4
export magnitude, normalize, dot, cross
export negate, lerp, distance
export transform2, transform3, transform4, transformPoint3, transformDir3
export domain Vector

use aivi
use aivi.math (sqrt)

Vec2 = { x: Float, y: Float }
Vec3 = { x: Float, y: Float, z: Float }
Vec4 = { x: Float, y: Float, z: Float, w: Float }

// Short constructors
vec2 : Float -> Float -> Vec2
vec2 = x y => { x: x, y: y }

vec3 : Float -> Float -> Float -> Vec3
vec3 = x y z => { x: x, y: y, z: z }

vec4 : Float -> Float -> Float -> Float -> Vec4
vec4 = x y z w => { x: x, y: y, z: z, w: w }

// Magnitude (length)
magnitude : Vec2 -> Float
magnitude = v => sqrt (v.x * v.x + v.y * v.y)

magnitude : Vec3 -> Float
magnitude = v => sqrt (v.x * v.x + v.y * v.y + v.z * v.z)

magnitude : Vec4 -> Float
magnitude = v => sqrt (v.x * v.x + v.y * v.y + v.z * v.z + v.w * v.w)

// Normalize
normalize : Vec2 -> Vec2
normalize = v => {
  len = magnitude v
  if len == 0.0 then v else { x: v.x / len, y: v.y / len }
}

normalize : Vec3 -> Vec3
normalize = v => {
  len = magnitude v
  if len == 0.0 then v else { x: v.x / len, y: v.y / len, z: v.z / len }
}

// Dot product
dot : Vec2 -> Vec2 -> Float
dot = a b => a.x * b.x + a.y * b.y

dot : Vec3 -> Vec3 -> Float
dot = a b => a.x * b.x + a.y * b.y + a.z * b.z

// Cross product (Vec3 only)
cross : Vec3 -> Vec3 -> Vec3
cross = a b => {
  x: a.y * b.z - a.z * b.y
  y: a.z * b.x - a.x * b.z
  z: a.x * b.y - a.y * b.x
}

// Negate
negate : Vec2 -> Vec2
negate = v => { x: 0.0 - v.x, y: 0.0 - v.y }

negate : Vec3 -> Vec3
negate = v => { x: 0.0 - v.x, y: 0.0 - v.y, z: 0.0 - v.z }

negate : Vec4 -> Vec4
negate = v => { x: 0.0 - v.x, y: 0.0 - v.y, z: 0.0 - v.z, w: 0.0 - v.w }

// Linear interpolation
lerp : Vec2 -> Vec2 -> Float -> Vec2
lerp = a b t => {
  x: a.x + (b.x - a.x) * t
  y: a.y + (b.y - a.y) * t
}

lerp : Vec3 -> Vec3 -> Float -> Vec3
lerp = a b t => {
  x: a.x + (b.x - a.x) * t
  y: a.y + (b.y - a.y) * t
  z: a.z + (b.z - a.z) * t
}

// Euclidean distance
distance : Vec2 -> Vec2 -> Float
distance = a b => magnitude { x: b.x - a.x, y: b.y - a.y }

distance : Vec3 -> Vec3 -> Float
distance = a b => magnitude { x: b.x - a.x, y: b.y - a.y, z: b.z - a.z }

// Vector × Matrix bridge functions
// Note: parameter types are structurally inferred from field accesses;
// the signatures match Mat2/Mat3/Mat4 defined in aivi.matrix.
transform2 = m v => {
  x: m.m00 * v.x + m.m01 * v.y
  y: m.m10 * v.x + m.m11 * v.y
}

transform3 = m v => {
  x: m.m00 * v.x + m.m01 * v.y + m.m02 * v.z
  y: m.m10 * v.x + m.m11 * v.y + m.m12 * v.z
  z: m.m20 * v.x + m.m21 * v.y + m.m22 * v.z
}

transform4 = m v => {
  x: m.m00 * v.x + m.m01 * v.y + m.m02 * v.z + m.m03 * v.w
  y: m.m10 * v.x + m.m11 * v.y + m.m12 * v.z + m.m13 * v.w
  z: m.m20 * v.x + m.m21 * v.y + m.m22 * v.z + m.m23 * v.w
  w: m.m30 * v.x + m.m31 * v.y + m.m32 * v.z + m.m33 * v.w
}

// Transform a 3D point by a 4x4 matrix (w=1, perspective divide)
transformPoint3 = m p => {
  invW = 1.0 / (m.m30 * p.x + m.m31 * p.y + m.m32 * p.z + m.m33)
  {
    x: (m.m00 * p.x + m.m01 * p.y + m.m02 * p.z + m.m03) * invW
    y: (m.m10 * p.x + m.m11 * p.y + m.m12 * p.z + m.m13) * invW
    z: (m.m20 * p.x + m.m21 * p.y + m.m22 * p.z + m.m23) * invW
  }
}

// Transform a 3D direction by a 4x4 matrix (w=0, no translation)
transformDir3 = m d => {
  x: m.m00 * d.x + m.m01 * d.y + m.m02 * d.z
  y: m.m10 * d.x + m.m11 * d.y + m.m12 * d.z
  z: m.m20 * d.x + m.m21 * d.y + m.m22 * d.z
}

domain Vector over Vec2 = {
  (+) : Vec2 -> Vec2 -> Vec2
  (+) = v1 v2 => { x: v1.x + v2.x, y: v1.y + v2.y }

  (-) : Vec2 -> Vec2 -> Vec2
  (-) = v1 v2 => { x: v1.x - v2.x, y: v1.y - v2.y }

  (*) : Vec2 -> Float -> Vec2
  (*) = v s => { x: v.x * s, y: v.y * s }

  (/) : Vec2 -> Float -> Vec2
  (/) = v s => { x: v.x / s, y: v.y / s }
}

domain Vector over Vec3 = {
  (+) : Vec3 -> Vec3 -> Vec3
  (+) = v1 v2 => { x: v1.x + v2.x, y: v1.y + v2.y, z: v1.z + v2.z }

  (-) : Vec3 -> Vec3 -> Vec3
  (-) = v1 v2 => { x: v1.x - v2.x, y: v1.y - v2.y, z: v1.z - v2.z }

  (*) : Vec3 -> Float -> Vec3
  (*) = v s => { x: v.x * s, y: v.y * s, z: v.z * s }

  (/) : Vec3 -> Float -> Vec3
  (/) = v s => { x: v.x / s, y: v.y / s, z: v.z / s }
}

domain Vector over Vec4 = {
  (+) : Vec4 -> Vec4 -> Vec4
  (+) = v1 v2 => { x: v1.x + v2.x, y: v1.y + v2.y, z: v1.z + v2.z, w: v1.w + v2.w }

  (-) : Vec4 -> Vec4 -> Vec4
  (-) = v1 v2 => { x: v1.x - v2.x, y: v1.y - v2.y, z: v1.z - v2.z, w: v1.w - v2.w }

  (*) : Vec4 -> Float -> Vec4
  (*) = v s => { x: v.x * s, y: v.y * s, z: v.z * s, w: v.w * s }

  (/) : Vec4 -> Float -> Vec4
  (/) = v s => { x: v.x / s, y: v.y / s, z: v.z / s, w: v.w / s }
}"#;

