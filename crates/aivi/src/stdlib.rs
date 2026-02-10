use std::path::PathBuf;

use crate::surface::{parse_modules, Module};

const CORE_SOURCE: &str = r#"
@no_prelude
module aivi.std.core = {
  export Unit, Bool, Int, Float, Text, Char
  export List, Option, Result, Tuple
  export None, Some, Ok, Err, True, False
  export pure, fail, attempt, load, print
}
"#;

const UNITS_SOURCE: &str = r#"
@no_prelude
module aivi.std.core.units = {
  export Unit, Quantity
  export defineUnit, convert, sameUnit
  export domain Units

  use aivi.std.core

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

const UNITS_FACADE_SOURCE: &str = r#"
@no_prelude
module aivi.std.units = {
  export Unit, Quantity
  export defineUnit, convert, sameUnit
  export domain Units

  use aivi.std.core.units
}
"#;

const REGEX_SOURCE: &str = r#"
@no_prelude
module aivi.std.core.regex = {
  export Regex, compile, test

  use aivi.std.core

  Regex = Text

  compile : Text -> Regex
  compile pattern = pattern

  test : Regex -> Text -> Bool
  test pattern text = pattern == text
}
"#;

const REGEX_FACADE_SOURCE: &str = r#"
@no_prelude
module aivi.std.regex = {
  export Regex, compile, test

  use aivi.std.core.regex
}
"#;

const TESTING_SOURCE: &str = r#"
@no_prelude
module aivi.std.core.testing = {
  export assert, assert_eq

  use aivi.std.core

  assert : Bool -> Effect Text Unit
  assert ok = if ok then pure Unit else fail "assertion failed"

  assert_eq : A -> A -> Effect Text Unit
  assert_eq a b = if a == b then pure Unit else fail "assert_eq failed"
}
"#;

const TESTING_FACADE_SOURCE: &str = r#"
@no_prelude
module aivi.std.testing = {
  export assert, assert_eq

  use aivi.std.core.testing
}
"#;

const NUMBER_FACADE_SOURCE: &str = r#"
@no_prelude
module aivi.std.number = {
  export BigInt, Rational, Complex, i
  export fromInt, toInt, absInt, gcdInt, normalize

  use aivi.std.number.bigint (BigInt, fromInt, toInt, absInt)
  use aivi.std.number.rational (Rational, gcdInt, normalize)
  use aivi.std.number.complex (Complex, i)
}
"#;

const BIGINT_SOURCE: &str = r#"
@no_prelude
module aivi.std.number.bigint = {
  export BigInt, fromInt, toInt, absInt
  export domain BigInt

  use aivi.std.core

  BigInt = { sign: Int, limbs: List Int }

  absInt : Int -> Int
  absInt n = if n < 0 then -n else n

  toInt : BigInt -> Int
  toInt b = b.limbs ?
    | [] => 0
    | [x, ..._] => b.sign * x

  fromInt : Int -> BigInt
  fromInt n = {
    sign: if n < 0 then -1 else 1
    limbs: [absInt n]
  }

  domain BigInt over BigInt = {
    (+) : BigInt -> BigInt -> BigInt
    (+) a b = fromInt (toInt a + toInt b)

    (-) : BigInt -> BigInt -> BigInt
    (-) a b = fromInt (toInt a - toInt b)

    (*) : BigInt -> BigInt -> BigInt
    (*) a b = fromInt (toInt a * toInt b)

    1n = fromInt 1
  }
}
"#;

const RATIONAL_SOURCE: &str = r#"
@no_prelude
module aivi.std.number.rational = {
  export Rational, gcdInt, normalize
  export domain Rational

  use aivi.std.core
  use aivi.std.number.bigint (BigInt, fromInt, toInt, absInt)

  BigInt = { sign: Int, limbs: List Int }
  Rational = { num: BigInt, den: BigInt }

  gcdInt : Int -> Int -> Int
  gcdInt a b = if b == 0 then a else gcdInt b (a % b)

  normalize : Rational -> Rational
  normalize r = {
    num = toInt r.num
    den = toInt r.den
    sign = if den < 0 then -1 else 1
    absDen = absInt den
    d = gcdInt (absInt num) absDen
    {
      num: fromInt ((num / d) * sign)
      den: fromInt (absDen / d)
    }
  }

  domain Rational over Rational = {
    (+) : Rational -> Rational -> Rational
    (+) a b = normalize {
      num: fromInt (toInt a.num * toInt b.den + toInt b.num * toInt a.den)
      den: fromInt (toInt a.den * toInt b.den)
    }

    (-) : Rational -> Rational -> Rational
    (-) a b = normalize {
      num: fromInt (toInt a.num * toInt b.den - toInt b.num * toInt a.den)
      den: fromInt (toInt a.den * toInt b.den)
    }

    (*) : Rational -> Rational -> Rational
    (*) a b = normalize {
      num: fromInt (toInt a.num * toInt b.num)
      den: fromInt (toInt a.den * toInt b.den)
    }

    (/) : Rational -> Rational -> Rational
    (/) a b = normalize {
      num: fromInt (toInt a.num * toInt b.den)
      den: fromInt (toInt a.den * toInt b.num)
    }
  }
}
"#;

const COMPLEX_SOURCE: &str = r#"
@no_prelude
module aivi.std.number.complex = {
  export Complex, i
  export domain Complex

  use aivi.std.core

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

const PRELUDE_SOURCE: &str = r#"
@no_prelude
module aivi.prelude = {
  export Unit, Bool, Int, Float, Text, Char
  export List, Option, Result, Tuple
  export None, Some, Ok, Err, True, False
  export Eq, Ord, Show, Num
  export Functor, Applicative, Monad
  export pure, fail, attempt, load, print

  export domain Calendar
  export domain Duration
  export domain Color
  export domain Vector

  use aivi.std.core
  use aivi.std.calendar
  use aivi.std.duration
  use aivi.std.color
  use aivi.std.vector

  class Eq A = {
    eq: A -> A -> Bool
  }

  class Ord A = {
    lt: A -> A -> Bool
    lte: A -> A -> Bool
  }

  class Show A = {
    show: A -> Text
  }

  class Num A = {
    add: A -> A -> A
    sub: A -> A -> A
    mul: A -> A -> A
    neg: A -> A
  }

  class Functor (F *) = {
    map: F A -> (A -> B) -> F B
  }

  class Applicative (F *) = {
    pure: A -> F A
    apply: F (A -> B) -> F A -> F B
  }

  class Monad (M *) = {
    pure: A -> M A
    flatMap: M A -> (A -> M B) -> M B
  }
}
"#;

pub fn embedded_stdlib_modules() -> Vec<Module> {
    let mut modules = Vec::new();
    modules.extend(parse_embedded("aivi.std.core", CORE_SOURCE));
    modules.extend(parse_embedded("aivi.std.core.units", UNITS_SOURCE));
    modules.extend(parse_embedded("aivi.std.units", UNITS_FACADE_SOURCE));
    modules.extend(parse_embedded("aivi.std.core.regex", REGEX_SOURCE));
    modules.extend(parse_embedded("aivi.std.regex", REGEX_FACADE_SOURCE));
    modules.extend(parse_embedded("aivi.std.core.testing", TESTING_SOURCE));
    modules.extend(parse_embedded("aivi.std.testing", TESTING_FACADE_SOURCE));
    modules.extend(parse_embedded("aivi.std.number.bigint", BIGINT_SOURCE));
    modules.extend(parse_embedded("aivi.std.number.rational", RATIONAL_SOURCE));
    modules.extend(parse_embedded("aivi.std.number.complex", COMPLEX_SOURCE));
    modules.extend(parse_embedded("aivi.std.number", NUMBER_FACADE_SOURCE));
    modules.extend(parse_embedded("aivi.prelude", PRELUDE_SOURCE));
    modules
}

pub fn embedded_stdlib_source(module_name: &str) -> Option<&'static str> {
    match module_name {
        "aivi.std.core" => Some(CORE_SOURCE),
        "aivi.std.core.units" => Some(UNITS_SOURCE),
        "aivi.std.units" => Some(UNITS_FACADE_SOURCE),
        "aivi.std.core.regex" => Some(REGEX_SOURCE),
        "aivi.std.regex" => Some(REGEX_FACADE_SOURCE),
        "aivi.std.core.testing" => Some(TESTING_SOURCE),
        "aivi.std.testing" => Some(TESTING_FACADE_SOURCE),
        "aivi.std.number" => Some(NUMBER_FACADE_SOURCE),
        "aivi.std.number.bigint" => Some(BIGINT_SOURCE),
        "aivi.std.number.rational" => Some(RATIONAL_SOURCE),
        "aivi.std.number.complex" => Some(COMPLEX_SOURCE),
        "aivi.prelude" => Some(PRELUDE_SOURCE),
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
