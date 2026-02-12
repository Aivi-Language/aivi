pub const MODULE_NAME: &str = "aivi.number.decimal";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.number.decimal
export fromFloat, toFloat, round
export domain Decimal
export 1dec

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

  1dec = fromFloat 1
}
"#;
