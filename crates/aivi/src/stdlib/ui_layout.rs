pub const MODULE_NAME: &str = "aivi.ui.layout";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.ui.layout
export UnitVal, Length, Percentage
export domain Layout

use aivi

// Underlying representation (implementation detail).
UnitVal = { val: Int }

domain Layout over UnitVal = {
  // Typed UI/layout units. These are also used by CSS-style records.
  Length = Px Int | Em Int | Rem Int | Vh Int | Vw Int
  Percentage = Pct Int

  // Literals
  1px = Px 1
  1em = Em 1
  1rem = Rem 1
  1vh = Vh 1
  1vw = Vw 1
  1% = Pct 1
}
"#;
