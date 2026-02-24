pub const MODULE_NAME: &str = "aivi.defaults";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.defaults
export ToDefault
export Bool, Int, Float, Text, List, Option

use aivi

class ToDefault A = {
  toDefault: A
}

instance ToDefault Bool = {
  toDefault: False
}

instance ToDefault Int = {
  toDefault: 0
}

instance ToDefault Float = {
  toDefault: 0.0
}

instance ToDefault Text = {
  toDefault: ""
}

instance ToDefault (List A) = {
  toDefault: []
}

instance ToDefault (Option A) = {
  toDefault: None
}
"#;
