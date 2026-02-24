pub const MODULE_NAME: &str = "aivi.matrix";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.matrix
export Mat2, Mat3, Mat4, Scalar
export identity2, identity3, identity4
export transpose2, transpose3, transpose4
export multiply2, multiply3, multiply4
export Matrix

use aivi
use aivi.vector (Vec2, Vec3, Vec4, transform2, transform3, transform4)

Mat2 = { m00: Float, m01: Float, m10: Float, m11: Float }
Mat3 = { m00: Float, m01: Float, m02: Float, m10: Float, m11: Float, m12: Float, m20: Float, m21: Float, m22: Float }
Mat4 = { m00: Float, m01: Float, m02: Float, m03: Float, m10: Float, m11: Float, m12: Float, m13: Float, m20: Float, m21: Float, m22: Float, m23: Float, m30: Float, m31: Float, m32: Float, m33: Float }
Scalar = Float

identity2 : Mat2
identity2 = { m00: 1.0, m01: 0.0, m10: 0.0, m11: 1.0 }

identity3 : Mat3
identity3 = { m00: 1.0, m01: 0.0, m02: 0.0, m10: 0.0, m11: 1.0, m12: 0.0, m20: 0.0, m21: 0.0, m22: 1.0 }

identity4 : Mat4
identity4 = {
  m00: 1.0, m01: 0.0, m02: 0.0, m03: 0.0,
  m10: 0.0, m11: 1.0, m12: 0.0, m13: 0.0,
  m20: 0.0, m21: 0.0, m22: 1.0, m23: 0.0,
  m30: 0.0, m31: 0.0, m32: 0.0, m33: 1.0
}

transpose2 : Mat2 -> Mat2
transpose2 = m => { m00: m.m00, m01: m.m10, m10: m.m01, m11: m.m11 }

transpose3 : Mat3 -> Mat3
transpose3 = m => {
  m00: m.m00, m01: m.m10, m02: m.m20,
  m10: m.m01, m11: m.m11, m12: m.m21,
  m20: m.m02, m21: m.m12, m22: m.m22
}

transpose4 : Mat4 -> Mat4
transpose4 = m => {
  m00: m.m00, m01: m.m10, m02: m.m20, m03: m.m30,
  m10: m.m01, m11: m.m11, m12: m.m21, m13: m.m31,
  m20: m.m02, m21: m.m12, m22: m.m22, m23: m.m32,
  m30: m.m03, m31: m.m13, m32: m.m23, m33: m.m33
}

multiply2 : Mat2 -> Mat2 -> Mat2
multiply2 = a b => {
  m00: a.m00 * b.m00 + a.m01 * b.m10, m01: a.m00 * b.m01 + a.m01 * b.m11,
  m10: a.m10 * b.m00 + a.m11 * b.m10, m11: a.m10 * b.m01 + a.m11 * b.m11
}

multiply3 : Mat3 -> Mat3 -> Mat3
multiply3 = a b => {
  m00: a.m00 * b.m00 + a.m01 * b.m10 + a.m02 * b.m20,
  m01: a.m00 * b.m01 + a.m01 * b.m11 + a.m02 * b.m21,
  m02: a.m00 * b.m02 + a.m01 * b.m12 + a.m02 * b.m22,
  m10: a.m10 * b.m00 + a.m11 * b.m10 + a.m12 * b.m20,
  m11: a.m10 * b.m01 + a.m11 * b.m11 + a.m12 * b.m21,
  m12: a.m10 * b.m02 + a.m11 * b.m12 + a.m12 * b.m22,
  m20: a.m20 * b.m00 + a.m21 * b.m10 + a.m22 * b.m20,
  m21: a.m20 * b.m01 + a.m21 * b.m11 + a.m22 * b.m21,
  m22: a.m20 * b.m02 + a.m21 * b.m12 + a.m22 * b.m22
}

multiply4 : Mat4 -> Mat4 -> Mat4
multiply4 = a b => {
  m00: a.m00 * b.m00 + a.m01 * b.m10 + a.m02 * b.m20 + a.m03 * b.m30,
  m01: a.m00 * b.m01 + a.m01 * b.m11 + a.m02 * b.m21 + a.m03 * b.m31,
  m02: a.m00 * b.m02 + a.m01 * b.m12 + a.m02 * b.m22 + a.m03 * b.m32,
  m03: a.m00 * b.m03 + a.m01 * b.m13 + a.m02 * b.m23 + a.m03 * b.m33,
  m10: a.m10 * b.m00 + a.m11 * b.m10 + a.m12 * b.m20 + a.m13 * b.m30,
  m11: a.m10 * b.m01 + a.m11 * b.m11 + a.m12 * b.m21 + a.m13 * b.m31,
  m12: a.m10 * b.m02 + a.m11 * b.m12 + a.m12 * b.m22 + a.m13 * b.m32,
  m13: a.m10 * b.m03 + a.m11 * b.m13 + a.m12 * b.m23 + a.m13 * b.m33,
  m20: a.m20 * b.m00 + a.m21 * b.m10 + a.m22 * b.m20 + a.m23 * b.m30,
  m21: a.m20 * b.m01 + a.m21 * b.m11 + a.m22 * b.m21 + a.m23 * b.m31,
  m22: a.m20 * b.m02 + a.m21 * b.m12 + a.m22 * b.m22 + a.m23 * b.m32,
  m23: a.m20 * b.m03 + a.m21 * b.m13 + a.m22 * b.m23 + a.m23 * b.m33,
  m30: a.m30 * b.m00 + a.m31 * b.m10 + a.m32 * b.m20 + a.m33 * b.m30,
  m31: a.m30 * b.m01 + a.m31 * b.m11 + a.m32 * b.m21 + a.m33 * b.m31,
  m32: a.m30 * b.m02 + a.m31 * b.m12 + a.m32 * b.m22 + a.m33 * b.m32,
  m33: a.m30 * b.m03 + a.m31 * b.m13 + a.m32 * b.m23 + a.m33 * b.m33
}

domain Matrix over Mat2 = {
  (+) : Mat2 -> Mat2 -> Mat2
  (+) = a b => { m00: a.m00 + b.m00, m01: a.m01 + b.m01, m10: a.m10 + b.m10, m11: a.m11 + b.m11 }

  (-) : Mat2 -> Mat2 -> Mat2
  (-) = a b => { m00: a.m00 - b.m00, m01: a.m01 - b.m01, m10: a.m10 - b.m10, m11: a.m11 - b.m11 }

  (*) : Mat2 -> Scalar -> Mat2
  (*) = m s => { m00: m.m00 * s, m01: m.m01 * s, m10: m.m10 * s, m11: m.m11 * s }

  (×) : Mat2 -> Mat2 -> Mat2
  (×) = a b => multiply2 a b

  (×) : Mat2 -> Vec2 -> Vec2
  (×) = m v => transform2 m v
}

domain Matrix over Mat3 = {
  (+) : Mat3 -> Mat3 -> Mat3
  (+) = a b => {
    m00: a.m00 + b.m00, m01: a.m01 + b.m01, m02: a.m02 + b.m02,
    m10: a.m10 + b.m10, m11: a.m11 + b.m11, m12: a.m12 + b.m12,
    m20: a.m20 + b.m20, m21: a.m21 + b.m21, m22: a.m22 + b.m22
  }

  (-) : Mat3 -> Mat3 -> Mat3
  (-) = a b => {
    m00: a.m00 - b.m00, m01: a.m01 - b.m01, m02: a.m02 - b.m02,
    m10: a.m10 - b.m10, m11: a.m11 - b.m11, m12: a.m12 - b.m12,
    m20: a.m20 - b.m20, m21: a.m21 - b.m21, m22: a.m22 - b.m22
  }

  (*) : Mat3 -> Scalar -> Mat3
  (*) = m s => {
    m00: m.m00 * s, m01: m.m01 * s, m02: m.m02 * s,
    m10: m.m10 * s, m11: m.m11 * s, m12: m.m12 * s,
    m20: m.m20 * s, m21: m.m21 * s, m22: m.m22 * s
  }

  (×) : Mat3 -> Mat3 -> Mat3
  (×) = a b => multiply3 a b

  (×) : Mat3 -> Vec3 -> Vec3
  (×) = m v => transform3 m v
}

domain Matrix over Mat4 = {
  (+) : Mat4 -> Mat4 -> Mat4
  (+) = a b => {
    m00: a.m00 + b.m00, m01: a.m01 + b.m01, m02: a.m02 + b.m02, m03: a.m03 + b.m03,
    m10: a.m10 + b.m10, m11: a.m11 + b.m11, m12: a.m12 + b.m12, m13: a.m13 + b.m13,
    m20: a.m20 + b.m20, m21: a.m21 + b.m21, m22: a.m22 + b.m22, m23: a.m23 + b.m23,
    m30: a.m30 + b.m30, m31: a.m31 + b.m31, m32: a.m32 + b.m32, m33: a.m33 + b.m33
  }

  (-) : Mat4 -> Mat4 -> Mat4
  (-) = a b => {
    m00: a.m00 - b.m00, m01: a.m01 - b.m01, m02: a.m02 - b.m02, m03: a.m03 - b.m03,
    m10: a.m10 - b.m10, m11: a.m11 - b.m11, m12: a.m12 - b.m12, m13: a.m13 - b.m13,
    m20: a.m20 - b.m20, m21: a.m21 - b.m21, m22: a.m22 - b.m22, m23: a.m23 - b.m23,
    m30: a.m30 - b.m30, m31: a.m31 - b.m31, m32: a.m32 - b.m32, m33: a.m33 - b.m33
  }

  (*) : Mat4 -> Scalar -> Mat4
  (*) = m s => {
    m00: m.m00 * s, m01: m.m01 * s, m02: m.m02 * s, m03: m.m03 * s,
    m10: m.m10 * s, m11: m.m11 * s, m12: m.m12 * s, m13: m.m13 * s,
    m20: m.m20 * s, m21: m.m21 * s, m22: m.m22 * s, m23: m.m23 * s,
    m30: m.m30 * s, m31: m.m31 * s, m32: m.m32 * s, m33: m.m33 * s
  }

  (×) : Mat4 -> Mat4 -> Mat4
  (×) = a b => multiply4 a b

  (×) : Mat4 -> Vec4 -> Vec4
  (×) = m v => transform4 m v
}"#;
