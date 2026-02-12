pub const MODULE_NAME: &str = "aivi.number.rational";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.number.rational
export normalize, numerator, denominator
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
}"#;
