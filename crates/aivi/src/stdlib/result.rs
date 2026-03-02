pub const MODULE_NAME: &str = "aivi.result";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.result
export isOk, isErr
export getOrElse, getOrElseLazy
export map, mapErr, flatMap
export toOption, flatten, orElse
export fromOption

use aivi

isOk : Result E A -> Bool
isOk = res => res match
  | Ok _  => True
  | Err _ => False

isErr : Result E A -> Bool
isErr = res => res match
  | Ok _  => False
  | Err _ => True

getOrElse : A -> Result E A -> A
getOrElse = default res => res match
  | Ok x  => x
  | Err _ => default

getOrElseLazy : (E -> A) -> Result E A -> A
getOrElseLazy = f res => res match
  | Ok x  => x
  | Err e => f e

map : (A -> B) -> Result E A -> Result E B
map = f res => res match
  | Ok x  => Ok (f x)
  | Err e => Err e

mapErr : (E -> F) -> Result E A -> Result F A
mapErr = f res => res match
  | Ok x  => Ok x
  | Err e => Err (f e)

flatMap : (A -> Result E B) -> Result E A -> Result E B
flatMap = f res => res match
  | Ok x  => f x
  | Err e => Err e

toOption : Result E A -> Option A
toOption = res => res match
  | Ok x  => Some x
  | Err _ => None

flatten : Result E (Result E A) -> Result E A
flatten = res => res match
  | Ok inner => inner
  | Err e    => Err e

orElse : Result E A -> Result E A -> Result E A
orElse = fallback res => res match
  | Ok x  => Ok x
  | Err _ => fallback

fromOption : E -> Option A -> Result E A
fromOption = err opt => opt match
  | Some x => Ok x
  | None   => Err err
"#;
