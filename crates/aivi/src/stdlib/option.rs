pub const MODULE_NAME: &str = "aivi.option";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.option
export isSome, isNone
export getOrElse, getOrElseLazy
export toList, toResult

use aivi

isSome : Option A -> Bool
isSome = opt => opt match
  | Some _ => True
  | None   => False

isNone : Option A -> Bool
isNone = opt => opt match
  | Some _ => False
  | None   => True

getOrElse : A -> Option A -> A
getOrElse = default opt => opt match
  | Some x => x
  | None   => default

getOrElseLazy : (Unit -> A) -> Option A -> A
getOrElseLazy = f opt => opt match
  | Some x => x
  | None   => f Unit

toList : Option A -> List A
toList = opt => opt match
  | Some x => [x]
  | None   => []

toResult : E -> Option A -> Result E A
toResult = err opt => opt match
  | Some x => Ok x
  | None   => Err err
"#;
