pub const MODULE_NAME: &str = "aivi.prelude";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.prelude
export Int, Float, Bool, Text, Char, Bytes
export List, Option, Result, Tuple, Patch
export ToText, toText

export domain Calendar
export domain Duration
export domain Color
export domain Vector

use aivi
use aivi.text
use aivi.logic
use aivi.calendar
use aivi.duration
use aivi.color
use aivi.vector

toText : A -> Text
toText = value => text.toText value

Patch A = A -> A"#;
