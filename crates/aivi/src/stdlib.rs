use std::path::PathBuf;

use crate::surface::{parse_modules, Module};

const CORE_SOURCE: &str = r#"
@no_prelude
module aivi = {
  export Unit, Bool, Int, Float, Text, Char, Bytes, DateTime
  export List, Option, Result, Tuple
  export None, Some, Ok, Err, True, False
  export pure, fail, attempt, load

  export text, regex, math, calendar, color
  export bigint, rational, decimal
  export url, console, file, clock, random, channel, concurrent, httpServer, http, https
}
"#;

const PRELUDE_SOURCE: &str = r#"
@no_prelude
module aivi.prelude = {
  export Int, Float, Bool, Text, Char, Bytes
  export List, Option, Result, Tuple

  export domain Calendar
  export domain Duration
  export domain Color
  export domain Vector

  use aivi
  use aivi.text
  use aivi.calendar
  use aivi.duration
  use aivi.color
  use aivi.vector
}
"#;

const TEXT_SOURCE: &str = r#"
@no_prelude
module aivi.text = {
  export Bytes, Encoding, TextError
  export length, isEmpty, isDigit, isAlpha, isAlnum, isSpace, isUpper, isLower
  export contains, startsWith, endsWith, indexOf, lastIndexOf, count, compare
  export slice, split, splitLines, chunk
  export trim, trimStart, trimEnd, padStart, padEnd
  export replace, replaceAll, remove, repeat, reverse, concat
  export toLower, toUpper, capitalize, titleCase, caseFold
  export normalizeNFC, normalizeNFD, normalizeNFKC, normalizeNFKD
  export toBytes, fromBytes, toText, parseInt, parseFloat

  use aivi

  type Encoding = Utf8 | Utf16 | Utf32 | Latin1
  type TextError = InvalidEncoding Encoding

  length : Text -> Int
  length value = text.length value

  isEmpty : Text -> Bool
  isEmpty value = text.isEmpty value

  isDigit : Char -> Bool
  isDigit value = text.isDigit value

  isAlpha : Char -> Bool
  isAlpha value = text.isAlpha value

  isAlnum : Char -> Bool
  isAlnum value = text.isAlnum value

  isSpace : Char -> Bool
  isSpace value = text.isSpace value

  isUpper : Char -> Bool
  isUpper value = text.isUpper value

  isLower : Char -> Bool
  isLower value = text.isLower value

  contains : Text -> Text -> Bool
  contains haystack needle = text.contains haystack needle

  startsWith : Text -> Text -> Bool
  startsWith value prefix = text.startsWith value prefix

  endsWith : Text -> Text -> Bool
  endsWith value suffix = text.endsWith value suffix

  indexOf : Text -> Text -> Option Int
  indexOf haystack needle = text.indexOf haystack needle

  lastIndexOf : Text -> Text -> Option Int
  lastIndexOf haystack needle = text.lastIndexOf haystack needle

  count : Text -> Text -> Int
  count haystack needle = text.count haystack needle

  compare : Text -> Text -> Int
  compare left right = text.compare left right

  slice : Int -> Int -> Text -> Text
  slice start end value = text.slice start end value

  split : Text -> Text -> List Text
  split sep value = text.split sep value

  splitLines : Text -> List Text
  splitLines value = text.splitLines value

  chunk : Int -> Text -> List Text
  chunk size value = text.chunk size value

  trim : Text -> Text
  trim value = text.trim value

  trimStart : Text -> Text
  trimStart value = text.trimStart value

  trimEnd : Text -> Text
  trimEnd value = text.trimEnd value

  padStart : Int -> Text -> Text -> Text
  padStart width fill value = text.padStart width fill value

  padEnd : Int -> Text -> Text -> Text
  padEnd width fill value = text.padEnd width fill value

  replace : Text -> Text -> Text -> Text
  replace value needle replacement = text.replace value needle replacement

  replaceAll : Text -> Text -> Text -> Text
  replaceAll value needle replacement = text.replaceAll value needle replacement

  remove : Text -> Text -> Text
  remove value needle = text.remove value needle

  repeat : Int -> Text -> Text
  repeat count value = text.repeat count value

  reverse : Text -> Text
  reverse value = text.reverse value

  concat : List Text -> Text
  concat values = text.concat values

  toLower : Text -> Text
  toLower value = text.toLower value

  toUpper : Text -> Text
  toUpper value = text.toUpper value

  capitalize : Text -> Text
  capitalize value = text.capitalize value

  titleCase : Text -> Text
  titleCase value = text.titleCase value

  caseFold : Text -> Text
  caseFold value = text.caseFold value

  normalizeNFC : Text -> Text
  normalizeNFC value = text.normalizeNFC value

  normalizeNFD : Text -> Text
  normalizeNFD value = text.normalizeNFD value

  normalizeNFKC : Text -> Text
  normalizeNFKC value = text.normalizeNFKC value

  normalizeNFKD : Text -> Text
  normalizeNFKD value = text.normalizeNFKD value

  toBytes : Encoding -> Text -> Bytes
  toBytes encoding value = text.toBytes encoding value

  fromBytes : Encoding -> Bytes -> Result TextError Text
  fromBytes encoding value = text.fromBytes encoding value

  toText : A -> Text
  toText value = text.toText value

  parseInt : Text -> Option Int
  parseInt value = text.parseInt value

  parseFloat : Text -> Option Float
  parseFloat value = text.parseFloat value
}
"#;

const REGEX_SOURCE: &str = r#"
@no_prelude
module aivi.regex = {
  export Regex, RegexError, Match
  export compile, test, match, matches, find, findAll, split, replace, replaceAll

  use aivi

  type RegexError = InvalidPattern Text
  type Match = { full: Text, groups: List (Option Text), start: Int, end: Int }

  compile : Text -> Result RegexError Regex
  compile pattern = regex.compile pattern

  test : Regex -> Text -> Bool
  test r value = regex.test r value

  match : Regex -> Text -> Option Match
  match r value = regex.match r value

  matches : Regex -> Text -> List Match
  matches r value = regex.matches r value

  find : Regex -> Text -> Option (Int, Int)
  find r value = regex.find r value

  findAll : Regex -> Text -> List (Int, Int)
  findAll r value = regex.findAll r value

  split : Regex -> Text -> List Text
  split r value = regex.split r value

  replace : Regex -> Text -> Text -> Text
  replace r value replacement = regex.replace r value replacement

  replaceAll : Regex -> Text -> Text -> Text
  replaceAll r value replacement = regex.replaceAll r value replacement
}
"#;

const TESTING_SOURCE: &str = r#"
@no_prelude
module aivi.testing = {
  export assert, assert_eq

  use aivi

  assert : Bool -> Effect Text Unit
  assert ok = if ok then pure Unit else fail "assertion failed"

  assert_eq : A -> A -> Effect Text Unit
  assert_eq a b = if a == b then pure Unit else fail "assert_eq failed"
}
"#;

const UNITS_SOURCE: &str = r#"
@no_prelude
module aivi.units = {
  export Unit, Quantity
  export defineUnit, convert, sameUnit
  export domain Units

  use aivi

  Unit = { name: Text, factor: Float }
  Quantity = { value: Float, unit: Unit }

  defineUnit : Text -> Float -> Unit
  defineUnit name factor = { name: name, factor: factor }

  convert : Quantity -> Unit -> Quantity
  convert q target = {
    value: q.value * (q.unit.factor / target.factor)
    unit: target
  }

  sameUnit : Quantity -> Quantity -> Bool
  sameUnit a b = a.unit.name == b.unit.name

  domain Units over Quantity = {
    (+) : Quantity -> Quantity -> Quantity
    (+) a b = { value: a.value + b.value, unit: a.unit }

    (-) : Quantity -> Quantity -> Quantity
    (-) a b = { value: a.value - b.value, unit: a.unit }

    (*) : Quantity -> Float -> Quantity
    (*) q s = { value: q.value * s, unit: q.unit }

    (/) : Quantity -> Float -> Quantity
    (/) q s = { value: q.value / s, unit: q.unit }
  }
}
"#;

const CALENDAR_SOURCE: &str = r#"
@no_prelude
module aivi.calendar = {
  export Date, DateTime, EndOfMonth
  export isLeapYear, daysInMonth, endOfMonth
  export addDays, addMonths, addYears, negateDelta
  export now
  export domain Calendar

  use aivi

  Date = { year: Int, month: Int, day: Int }
  type EndOfMonth = EndOfMonth

  isLeapYear : Date -> Bool
  isLeapYear value = calendar.isLeapYear value

  daysInMonth : Date -> Int
  daysInMonth value = calendar.daysInMonth value

  endOfMonth : Date -> Date
  endOfMonth value = calendar.endOfMonth value

  addDays : Date -> Int -> Date
  addDays value n = calendar.addDays value n

  addMonths : Date -> Int -> Date
  addMonths value n = calendar.addMonths value n

  addYears : Date -> Int -> Date
  addYears value n = calendar.addYears value n

  negateDelta : Delta -> Delta
  negateDelta delta = delta ?
    | Day n => Day (-n)
    | Month n => Month (-n)
    | Year n => Year (-n)
    | End => End

  now : Effect DateTime
  now = clock.now Unit

  domain Calendar over Date = {
    type Delta = Day Int | Month Int | Year Int | End EndOfMonth

    (+) : Date -> Delta -> Date
    (+) date (Day n) = addDays date n
    (+) date (Month n) = addMonths date n
    (+) date (Year n) = addYears date n
    (+) date End = endOfMonth date

    (-) : Date -> Delta -> Date
    (-) date delta = date + (negateDelta delta)

    1d = Day 1
    1m = Month 1
    1y = Year 1
    eom = End
  }
}
"#;

const DURATION_SOURCE: &str = r#"
@no_prelude
module aivi.duration = {
  export Span, negateDelta
  export domain Duration

  use aivi

  Span = { millis: Int }

  negateDelta : Delta -> Delta
  negateDelta delta = delta ?
    | Millisecond n => Millisecond (-n)
    | Second n => Second (-n)
    | Minute n => Minute (-n)
    | Hour n => Hour (-n)

  domain Duration over Span = {
    type Delta = Millisecond Int | Second Int | Minute Int | Hour Int

    (+) : Span -> Delta -> Span
    (+) span (Millisecond n) = { millis: span.millis + n }
    (+) span (Second n) = { millis: span.millis + n * 1000 }
    (+) span (Minute n) = { millis: span.millis + n * 60000 }
    (+) span (Hour n) = { millis: span.millis + n * 3600000 }

    (-) : Span -> Delta -> Span
    (-) span delta = span + (negateDelta delta)

    (+) : Span -> Span -> Span
    (+) s1 s2 = { millis: s1.millis + s2.millis }

    1ms = Millisecond 1
    1s = Second 1
    1min = Minute 1
    1h = Hour 1
  }
}
"#;

const COLOR_SOURCE: &str = r#"
@no_prelude
module aivi.color = {
  export Rgb, Hsl, Hex
  export adjustLightness, adjustSaturation, adjustHue
  export toRgb, toHsl, toHex
  export negateDelta
  export domain Color

  use aivi

  Rgb = { r: Int, g: Int, b: Int }
  Hsl = { h: Float, s: Float, l: Float }
  Hex = Text

  adjustLightness : Rgb -> Int -> Rgb
  adjustLightness value amount = color.adjustLightness value amount

  adjustSaturation : Rgb -> Int -> Rgb
  adjustSaturation value amount = color.adjustSaturation value amount

  adjustHue : Rgb -> Int -> Rgb
  adjustHue value amount = color.adjustHue value amount

  toRgb : Hsl -> Rgb
  toRgb value = color.toRgb value

  toHsl : Rgb -> Hsl
  toHsl value = color.toHsl value

  toHex : Rgb -> Hex
  toHex value = color.toHex value

  negateDelta : Delta -> Delta
  negateDelta delta = delta ?
    | Lightness n => Lightness (-n)
    | Saturation n => Saturation (-n)
    | Hue n => Hue (-n)

  domain Color over Rgb = {
    type Delta = Lightness Int | Saturation Int | Hue Int

    (+) : Rgb -> Delta -> Rgb
    (+) col (Lightness n) = adjustLightness col n
    (+) col (Saturation n) = adjustSaturation col n
    (+) col (Hue n) = adjustHue col n

    (-) : Rgb -> Delta -> Rgb
    (-) col delta = col + (negateDelta delta)

    1l = Lightness 1
    1s = Saturation 1
    1h = Hue 1
  }
}
"#;

const VECTOR_SOURCE: &str = r#"
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

const MATH_SOURCE: &str = r#"
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

const URL_SOURCE: &str = r#"
@no_prelude
module aivi.url = {
  export domain Url
  export Url
  export parse, toString

  use aivi

  Url = { protocol: Text, host: Text, port: Option Int, path: Text, query: List (Text, Text), hash: Option Text }

  parse : Text -> Result Text Url
  parse value = url.parse value

  toString : Url -> Text
  toString value = url.toString value

  filter : (A -> Bool) -> List A -> List A
  filter predicate items = items ?
    | [] => []
    | [x, ...xs] => if predicate x then [x, ...filter predicate xs] else filter predicate xs

  append : List A -> List A -> List A
  append left right = left ?
    | [] => right
    | [x, ...xs] => [x, ...append xs right]

  filterKey : Text -> (Text, Text) -> Bool
  filterKey key pair = pair ?
    | (k, _) => k != key

  domain Url over Url = {
    (+) : Url -> (Text, Text) -> Url
    (+) value (key, v) = { ...value, query: append value.query [(key, v)] }

    (-) : Url -> Text -> Url
    (-) value key = {
      ...value,
      query: filter (filterKey key) value.query
    }
  }
}
"#;

const CONSOLE_SOURCE: &str = r#"
@no_prelude
module aivi.console = {
  export log, println, print, error, readLine

  use aivi

  log : Text -> Effect Text Unit
  log value = console.log value

  println : Text -> Effect Text Unit
  println value = console.println value

  print : Text -> Effect Text Unit
  print value = console.print value

  error : Text -> Effect Text Unit
  error value = console.error value

  readLine : Effect Text (Result Text Text)
  readLine = console.readLine Unit
}
"#;

const FILE_SOURCE: &str = r#"
@no_prelude
module aivi.file = {
  export FileStats
  export open, readAll, close
  export readText, writeText, exists, stat, delete

  use aivi

  FileStats = { size: Int, created: Int, modified: Int, isFile: Bool, isDirectory: Bool }

  open : Text -> Effect Text FileHandle
  open path = file.open path

  readAll : FileHandle -> Effect Text Text
  readAll handle = file.readAll handle

  close : FileHandle -> Effect Text Unit
  close handle = file.close handle

  readText : Text -> Effect Text (Result Text Text)
  readText path = attempt (file.read path)

  writeText : Text -> Text -> Effect Text (Result Text Unit)
  writeText path contents = attempt (file.write_text path contents)

  exists : Text -> Effect Text Bool
  exists path = file.exists path

  stat : Text -> Effect Text (Result Text FileStats)
  stat path = attempt (file.stat path)

  delete : Text -> Effect Text (Result Text Unit)
  delete path = attempt (file.delete path)
}
"#;

const BIGINT_SOURCE: &str = r#"
@no_prelude
module aivi.number.bigint = {
  export BigInt, fromInt, toInt, absInt
  export domain BigInt

  use aivi

  absInt : Int -> Int
  absInt n = if n < 0 then -n else n

  fromInt : Int -> BigInt
  fromInt value = bigint.fromInt value

  toInt : BigInt -> Int
  toInt value = bigint.toInt value

  domain BigInt over BigInt = {
    (+) : BigInt -> BigInt -> BigInt
    (+) a b = bigint.add a b

    (-) : BigInt -> BigInt -> BigInt
    (-) a b = bigint.sub a b

    (*) : BigInt -> BigInt -> BigInt
    (*) a b = bigint.mul a b

    1n = fromInt 1
  }
}
"#;

const RATIONAL_SOURCE: &str = r#"
@no_prelude
module aivi.number.rational = {
  export Rational, normalize, numerator, denominator
  export domain Rational

  use aivi
  use aivi.number.bigint (BigInt)

  fromBigInts : BigInt -> BigInt -> Rational
  fromBigInts num den = rational.fromBigInts num den

  normalize : Rational -> Rational
  normalize value = rational.normalize value

  numerator : Rational -> BigInt
  numerator value = rational.numerator value

  denominator : Rational -> BigInt
  denominator value = rational.denominator value

  domain Rational over Rational = {
    (+) : Rational -> Rational -> Rational
    (+) a b = rational.add a b

    (-) : Rational -> Rational -> Rational
    (-) a b = rational.sub a b

    (*) : Rational -> Rational -> Rational
    (*) a b = rational.mul a b

    (/) : Rational -> Rational -> Rational
    (/) a b = rational.div a b
  }
}
"#;

const DECIMAL_SOURCE: &str = r#"
@no_prelude
module aivi.number.decimal = {
  export Decimal, fromFloat, toFloat, round
  export domain Decimal

  use aivi

  fromFloat : Float -> Decimal
  fromFloat value = decimal.fromFloat value

  toFloat : Decimal -> Float
  toFloat value = decimal.toFloat value

  round : Decimal -> Int -> Decimal
  round value places = decimal.round value places

  domain Decimal over Decimal = {
    (+) : Decimal -> Decimal -> Decimal
    (+) a b = decimal.add a b

    (-) : Decimal -> Decimal -> Decimal
    (-) a b = decimal.sub a b

    (*) : Decimal -> Decimal -> Decimal
    (*) a b = decimal.mul a b

    (/) : Decimal -> Decimal -> Decimal
    (/) a b = decimal.div a b
  }
}
"#;

const COMPLEX_SOURCE: &str = r#"
@no_prelude
module aivi.number.complex = {
  export Complex, i
  export domain Complex

  use aivi

  Complex = { re: Float, im: Float }

  i : Complex
  i = { re: 0.0, im: 1.0 }

  domain Complex over Complex = {
    (+) : Complex -> Complex -> Complex
    (+) a b = { re: a.re + b.re, im: a.im + b.im }

    (-) : Complex -> Complex -> Complex
    (-) a b = { re: a.re - b.re, im: a.im - b.im }

    (*) : Complex -> Complex -> Complex
    (*) a b = {
      re: a.re * b.re - a.im * b.im
      im: a.re * b.im + a.im * b.re
    }

    (/) : Complex -> Float -> Complex
    (/) z s = { re: z.re / s, im: z.im / s }
  }
}
"#;

const NUMBER_FACADE_SOURCE: &str = r#"
@no_prelude
module aivi.number = {
  export BigInt, Rational, Decimal, Complex, i
  export fromInt, toInt
  export fromFloat, toFloat, round
  export normalize, numerator, denominator

  use aivi.number.bigint (BigInt, fromInt, toInt)
  use aivi.number.decimal (Decimal, fromFloat, toFloat, round)
  use aivi.number.rational (Rational, normalize, numerator, denominator)
  use aivi.number.complex (Complex, i)
}
"#;

const NETWORK_HTTP_SERVER_SOURCE: &str = r#"
@no_prelude
module aivi.net.http_server = {
  export Header, Request, Response, ServerConfig
  export HttpError, WsError, WsMessage, ServerReply
  export Server, WebSocket
  export listen, stop, wsRecv, wsSend, wsClose

  use aivi

  Header = { name: Text, value: Text }
  Request = { method: Text, path: Text, headers: List Header, body: List Int, remoteAddr: Option Text }
  Response = { status: Int, headers: List Header, body: List Int }
  ServerConfig = { address: Text }
  HttpError = { message: Text }
  WsError = { message: Text }

  type WsMessage = TextMsg Text | BinaryMsg (List Int) | Ping | Pong | Close
  type ServerReply = Http Response | Ws (WebSocket -> Effect WsError Unit)

  listen : ServerConfig -> (Request -> Effect HttpError ServerReply) -> Resource Server
  listen config handler = resource {
    server <- httpServer.listen config handler
    yield server
    _ <- httpServer.stop server
  }

  stop : Server -> Effect HttpError Unit
  stop server = httpServer.stop server

  wsRecv : WebSocket -> Effect WsError WsMessage
  wsRecv socket = httpServer.ws_recv socket

  wsSend : WebSocket -> WsMessage -> Effect WsError Unit
  wsSend socket msg = httpServer.ws_send socket msg

  wsClose : WebSocket -> Effect WsError Unit
  wsClose socket = httpServer.ws_close socket
}
"#;

const NETWORK_HTTP_SOURCE: &str = r#"
@no_prelude
module aivi.net.http = {
  export Header, Request, Response, Error
  export get, post, fetch

  use aivi
  use aivi.url (Url)

  Header = { name: Text, value: Text }
  Request = { method: Text, url: Url, headers: List Header, body: Option Text }
  Response = { status: Int, headers: List Header, body: Text }
  Error = { message: Text }

  get : Url -> Effect Error (Result Error Response)
  get url = http.get url

  post : Url -> Text -> Effect Error (Result Error Response)
  post url body = http.post url body

  fetch : Request -> Effect Error (Result Error Response)
  fetch request = http.fetch request
}
"#;

const NETWORK_HTTPS_SOURCE: &str = r#"
@no_prelude
module aivi.net.https = {
  export Header, Request, Response, Error
  export get, post, fetch

  use aivi
  use aivi.url (Url)

  Header = { name: Text, value: Text }
  Request = { method: Text, url: Url, headers: List Header, body: Option Text }
  Response = { status: Int, headers: List Header, body: Text }
  Error = { message: Text }

  get : Url -> Effect Error (Result Error Response)
  get url = https.get url

  post : Url -> Text -> Effect Error (Result Error Response)
  post url body = https.post url body

  fetch : Request -> Effect Error (Result Error Response)
  fetch request = https.fetch request
}
"#;

const NETWORK_FACADE_SOURCE: &str = r#"
@no_prelude
module aivi.net = {
  export http, https, httpServer

  use aivi
}
"#;

pub fn embedded_stdlib_modules() -> Vec<Module> {
    let mut modules = Vec::new();
    modules.extend(parse_embedded("aivi", CORE_SOURCE));
    modules.extend(parse_embedded("aivi.prelude", PRELUDE_SOURCE));
    modules.extend(parse_embedded("aivi.text", TEXT_SOURCE));
    modules.extend(parse_embedded("aivi.regex", REGEX_SOURCE));
    modules.extend(parse_embedded("aivi.testing", TESTING_SOURCE));
    modules.extend(parse_embedded("aivi.units", UNITS_SOURCE));
    modules.extend(parse_embedded("aivi.calendar", CALENDAR_SOURCE));
    modules.extend(parse_embedded("aivi.duration", DURATION_SOURCE));
    modules.extend(parse_embedded("aivi.color", COLOR_SOURCE));
    modules.extend(parse_embedded("aivi.vector", VECTOR_SOURCE));
    modules.extend(parse_embedded("aivi.math", MATH_SOURCE));
    modules.extend(parse_embedded("aivi.url", URL_SOURCE));
    modules.extend(parse_embedded("aivi.console", CONSOLE_SOURCE));
    modules.extend(parse_embedded("aivi.file", FILE_SOURCE));
    modules.extend(parse_embedded("aivi.number.bigint", BIGINT_SOURCE));
    modules.extend(parse_embedded("aivi.number.rational", RATIONAL_SOURCE));
    modules.extend(parse_embedded("aivi.number.decimal", DECIMAL_SOURCE));
    modules.extend(parse_embedded("aivi.number.complex", COMPLEX_SOURCE));
    modules.extend(parse_embedded("aivi.number", NUMBER_FACADE_SOURCE));
    modules.extend(parse_embedded("aivi.net.http", NETWORK_HTTP_SOURCE));
    modules.extend(parse_embedded("aivi.net.https", NETWORK_HTTPS_SOURCE));
    modules.extend(parse_embedded("aivi.net", NETWORK_FACADE_SOURCE));
    modules.extend(parse_embedded(
        "aivi.net.http_server",
        NETWORK_HTTP_SERVER_SOURCE,
    ));
    modules
}

pub fn embedded_stdlib_source(module_name: &str) -> Option<&'static str> {
    match module_name {
        "aivi" => Some(CORE_SOURCE),
        "aivi.prelude" => Some(PRELUDE_SOURCE),
        "aivi.text" => Some(TEXT_SOURCE),
        "aivi.regex" => Some(REGEX_SOURCE),
        "aivi.testing" => Some(TESTING_SOURCE),
        "aivi.units" => Some(UNITS_SOURCE),
        "aivi.calendar" => Some(CALENDAR_SOURCE),
        "aivi.duration" => Some(DURATION_SOURCE),
        "aivi.color" => Some(COLOR_SOURCE),
        "aivi.vector" => Some(VECTOR_SOURCE),
        "aivi.math" => Some(MATH_SOURCE),
        "aivi.url" => Some(URL_SOURCE),
        "aivi.console" => Some(CONSOLE_SOURCE),
        "aivi.file" => Some(FILE_SOURCE),
        "aivi.number.bigint" => Some(BIGINT_SOURCE),
        "aivi.number.rational" => Some(RATIONAL_SOURCE),
        "aivi.number.decimal" => Some(DECIMAL_SOURCE),
        "aivi.number.complex" => Some(COMPLEX_SOURCE),
        "aivi.number" => Some(NUMBER_FACADE_SOURCE),
        "aivi.net.http" => Some(NETWORK_HTTP_SOURCE),
        "aivi.net.https" => Some(NETWORK_HTTPS_SOURCE),
        "aivi.net" => Some(NETWORK_FACADE_SOURCE),
        "aivi.net.http_server" => Some(NETWORK_HTTP_SERVER_SOURCE),
        _ => None,
    }
}

fn parse_embedded(name: &str, source: &str) -> Vec<Module> {
    let path = PathBuf::from(format!("<embedded:{name}>"));
    let (modules, diagnostics) = parse_modules(path.as_path(), source);
    debug_assert!(
        diagnostics.is_empty(),
        "embedded stdlib module {name} failed to parse"
    );
    modules
}
