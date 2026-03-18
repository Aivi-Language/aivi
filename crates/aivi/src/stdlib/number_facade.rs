pub const MODULE_NAME: &str = "aivi.number";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.number
export domain BigInt, domain Rational, domain Decimal, domain Complex, i
export fromInt, toInt
export fromFloat, toFloat, round
export fromBigInts, normalize, numerator, denominator

use aivi
use aivi.number.bigint (BigInt, domain BigInt)
use aivi.number.decimal (Decimal, domain Decimal)
use aivi.number.rational (Rational, domain Rational)
use aivi.number.complex (Complex, domain Complex)

fromInt : Int -> BigInt
fromInt = value => bigint.fromInt value

toInt : BigInt -> Option Int
toInt = value => bigint.toInt value

fromFloat : Float -> Decimal
fromFloat = value => decimal.fromFloat value

toFloat : Decimal -> Float
toFloat = value => decimal.toFloat value

round : Decimal -> Int -> Decimal
round = value places => decimal.round value places

fromBigInts : BigInt -> BigInt -> Rational
fromBigInts = num den => rational.fromBigInts num den

normalize : Rational -> Rational
normalize = value => rational.normalize value

numerator : Rational -> BigInt
numerator = value => rational.numerator value

denominator : Rational -> BigInt
denominator = value => rational.denominator value

i : Complex
i = { re: 0.0, im: 1.0 }"#;
