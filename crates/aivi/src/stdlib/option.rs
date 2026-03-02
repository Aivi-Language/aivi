pub const MODULE_NAME: &str = "aivi.option";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.option
export isSome, isNone
export getOrElse, getOrElseLazy
export map, flatMap, filter
export toList, toResult
export flatten, orElse

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

map : (A -> B) -> Option A -> Option B
map = f opt => opt match
  | Some x => Some (f x)
  | None   => None

flatMap : (A -> Option B) -> Option A -> Option B
flatMap = f opt => opt match
  | Some x => f x
  | None   => None

filter : (A -> Bool) -> Option A -> Option A
filter = pred opt => opt match
  | Some x => if pred x then Some x else None
  | None   => None

toList : Option A -> List A
toList = opt => opt match
  | Some x => [x]
  | None   => []

toResult : E -> Option A -> Result E A
toResult = err opt => opt match
  | Some x => Ok x
  | None   => Err err

flatten : Option (Option A) -> Option A
flatten = opt => opt match
  | Some inner => inner
  | None       => None

orElse : Option A -> Option A -> Option A
orElse = fallback opt => opt match
  | Some x => Some x
  | None   => fallback
"#;
