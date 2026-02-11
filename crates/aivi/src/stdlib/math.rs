pub const MODULE_NAME: &str = "aivi.math";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.math = {
  export pi, tau, e, inf, nan, phi, sqrt2, ln2, ln10
  export Angle, radians, degrees, toRadians, toDegrees
  export abs, sign, copysign, min, max, minAll, maxAll, clamp, sum, sumInt
  export floor, ceil, trunc, round, fract, modf, frexp, ldexp
  export pow, sqrt, cbrt, hypot, exp, exp2, expm1, log, log10, log2, log1p
  export sin, cos, tan, asin, acos, atan, atan2
  export sinh, cosh, tanh, asinh, acosh, atanh
  export gcd, lcm, gcdAll, lcmAll, factorial, comb, perm, divmod, modPow
  export isFinite, isInf, isNaN, nextAfter, ulp, fmod, remainder
  export BigInt

  use aivi
  use aivi.number (BigInt)

  Angle = { radians: Float }

  pi = math.pi
  tau = math.tau
  e = math.e
  inf = math.inf
  nan = math.nan
  phi = math.phi
  sqrt2 = math.sqrt2
  ln2 = math.ln2
  ln10 = math.ln10

  radians : Float -> Angle
  radians value = { radians: value }

  degrees : Float -> Angle
  degrees value = { radians: value * (pi / 180.0) }

  toRadians : Angle -> Float
  toRadians angle = angle.radians

  toDegrees : Angle -> Float
  toDegrees angle = angle.radians * (180.0 / pi)

  abs : A -> A
  abs value = math.abs value

  sign : Float -> Float
  sign value = math.sign value

  copysign : Float -> Float -> Float
  copysign mag sign = math.copysign mag sign

  min : Float -> Float -> Float
  min a b = math.min a b

  max : Float -> Float -> Float
  max a b = math.max a b

  minAll : List Float -> Option Float
  minAll values = math.minAll values

  maxAll : List Float -> Option Float
  maxAll values = math.maxAll values

  clamp : Float -> Float -> Float -> Float
  clamp low high value = math.clamp low high value

  sum : List Float -> Float
  sum values = math.sum values

  sumInt : List Int -> Int
  sumInt values = math.sumInt values

  floor : Float -> Float
  floor value = math.floor value

  ceil : Float -> Float
  ceil value = math.ceil value

  trunc : Float -> Float
  trunc value = math.trunc value

  round : Float -> Float
  round value = math.round value

  fract : Float -> Float
  fract value = math.fract value

  modf : Float -> (Float, Float)
  modf value = math.modf value

  frexp : Float -> (Float, Int)
  frexp value = math.frexp value

  ldexp : Float -> Int -> Float
  ldexp mantissa exponent = math.ldexp mantissa exponent

  pow : Float -> Float -> Float
  pow base exp = math.pow base exp

  sqrt : Float -> Float
  sqrt value = math.sqrt value

  cbrt : Float -> Float
  cbrt value = math.cbrt value

  hypot : Float -> Float -> Float
  hypot x y = math.hypot x y

  exp : Float -> Float
  exp value = math.exp value

  exp2 : Float -> Float
  exp2 value = math.exp2 value

  expm1 : Float -> Float
  expm1 value = math.expm1 value

  log : Float -> Float
  log value = math.log value

  log10 : Float -> Float
  log10 value = math.log10 value

  log2 : Float -> Float
  log2 value = math.log2 value

  log1p : Float -> Float
  log1p value = math.log1p value

  sin : Angle -> Float
  sin angle = math.sin angle

  cos : Angle -> Float
  cos angle = math.cos angle

  tan : Angle -> Float
  tan angle = math.tan angle

  asin : Float -> Angle
  asin value = math.asin value

  acos : Float -> Angle
  acos value = math.acos value

  atan : Float -> Angle
  atan value = math.atan value

  atan2 : Float -> Float -> Angle
  atan2 y x = math.atan2 y x

  sinh : Float -> Float
  sinh value = math.sinh value

  cosh : Float -> Float
  cosh value = math.cosh value

  tanh : Float -> Float
  tanh value = math.tanh value

  asinh : Float -> Float
  asinh value = math.asinh value

  acosh : Float -> Float
  acosh value = math.acosh value

  atanh : Float -> Float
  atanh value = math.atanh value

  gcd : Int -> Int -> Int
  gcd a b = math.gcd a b

  lcm : Int -> Int -> Int
  lcm a b = math.lcm a b

  gcdAll : List Int -> Option Int
  gcdAll values = math.gcdAll values

  lcmAll : List Int -> Option Int
  lcmAll values = math.lcmAll values

  factorial : Int -> BigInt
  factorial value = math.factorial value

  comb : Int -> Int -> BigInt
  comb n k = math.comb n k

  perm : Int -> Int -> BigInt
  perm n k = math.perm n k

  divmod : Int -> Int -> (Int, Int)
  divmod a b = math.divmod a b

  modPow : Int -> Int -> Int -> Int
  modPow base exp modulus = math.modPow base exp modulus

  isFinite : Float -> Bool
  isFinite value = math.isFinite value

  isInf : Float -> Bool
  isInf value = math.isInf value

  isNaN : Float -> Bool
  isNaN value = math.isNaN value

  nextAfter : Float -> Float -> Float
  nextAfter from to = math.nextAfter from to

  ulp : Float -> Float
  ulp value = math.ulp value

  fmod : Float -> Float -> Float
  fmod a b = math.fmod a b

  remainder : Float -> Float -> Float
  remainder a b = math.remainder a b
}
"#;
