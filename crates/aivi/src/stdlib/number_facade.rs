pub const MODULE_NAME: &str = "aivi.number";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.number
export domain BigInt, domain Rational, domain Decimal, domain Complex, i
export fromInt, toInt
export fromFloat, toFloat, round
export fromBigInts, normalize, numerator, denominator

use aivi.number.bigint (BigInt, domain BigInt)
use aivi.number.decimal (Decimal, domain Decimal)
use aivi.number.rational (Rational, domain Rational)
use aivi.number.complex (Complex, domain Complex)
use aivi.number.bigint (fromInt, toInt)
use aivi.number.decimal (fromFloat, toFloat, round)
use aivi.number.rational (fromBigInts, normalize, numerator, denominator)
use aivi.number.complex (i)"#;
