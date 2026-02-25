pub const MODULE_NAME: &str = "aivi.units";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.units
export Unit, Quantity
export defineUnit, convert, sameUnit
export domain Units

use aivi

Unit = { name: Text, factor: Float }
Quantity = { value: Float, unit: Unit }

defineUnit : Text -> Float -> Unit
defineUnit = name factor => { name: name, factor: factor }

convert : Quantity -> Unit -> Quantity
convert = q target => {
  value: q.value * (q.unit.factor / target.factor)
  unit: target
}

sameUnit : Quantity -> Quantity -> Bool
sameUnit = a b => a.unit.name == b.unit.name

domain Units over Quantity = {
  (+) : Quantity -> Quantity -> Quantity
  (+) = a b => { value: a.value + b.value, unit: a.unit }

  (-) : Quantity -> Quantity -> Quantity
  (-) = a b => { value: a.value - b.value, unit: a.unit }

  (*) : Quantity -> Float -> Quantity
  (*) = q s => { value: q.value * s, unit: q.unit }

  (/) : Quantity -> Float -> Quantity
  (/) = q s => { value: q.value / s, unit: q.unit }
}"#;
