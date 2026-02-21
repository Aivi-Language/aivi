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
export panic, not, any, each

use aivi
use aivi.text
use aivi.logic
use aivi.calendar
use aivi.duration
use aivi.color
use aivi.vector

toText : A -> Text
toText = value => text.toText value

Patch A = A -> A

panic : Text -> A
panic = msg => fail msg

not : Bool -> Bool
not = b => b match
  | True  => False
  | False => True

any : (A -> Bool) -> List A -> Bool
any = pred xs => xs match
  | []         => False
  | [x, ...rest] => pred x match
    | True  => True
    | False => any pred rest

each : (A -> B) -> List A -> List B
each = f xs => xs match
  | []            => []
  | [x, ...rest]  => [f x, ...each f rest]"#;
