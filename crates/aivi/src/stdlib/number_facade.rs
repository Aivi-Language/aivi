pub const MODULE_NAME: &str = "aivi.number";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.number
export domain BigInt, domain Rational, domain Decimal, domain Complex, i
export fromInt, toInt
export fromFloat, toFloat, round
export normalize, numerator, denominator

use aivi.number.bigint (BigInt, fromInt, toInt)
use aivi.number.decimal (Decimal, fromFloat, toFloat, round)
use aivi.number.rational (Rational, normalize, numerator, denominator)
use aivi.number.complex (Complex, i)"#;
