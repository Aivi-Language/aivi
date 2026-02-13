pub const MODULE_NAME: &str = "aivi.ui.layout";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.ui.layout
export UnitVal, Length, Percentage
export domain Layout

use aivi

// Underlying representation (implementation detail).
UnitVal = { val: Float }

domain Layout over UnitVal = {
  // Typed UI/layout units. These are also used by CSS-style records.
  type Length = Px Float | Em Float | Rem Float | Vh Float | Vw Float
  type Percentage = Pct Float

  // Literals
  1px = Px 1.0
  1em = Em 1.0
  1rem = Rem 1.0
  1vh = Vh 1.0
  1vw = Vw 1.0
  1% = Pct 1.0

  // Arithmetic within same unit type
  (+) : Length -> Length -> Length
  (+) (Px a) (Px b) = Px (a + b)

  (-) : Length -> Length -> Length
  (-) (Px a) (Px b) = Px (a - b)
}
"#;
