pub const MODULE_NAME: &str = "aivi.result";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.result
export isOk, isErr
export getOrElse, getOrElseLazy
export mapErr
export toOption
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

mapErr : (E -> F) -> Result E A -> Result F A
mapErr = f res => res match
  | Ok x  => Ok x
  | Err e => Err (f e)

toOption : Result E A -> Option A
toOption = res => res match
  | Ok x  => Some x
  | Err _ => None

fromOption : E -> Option A -> Result E A
fromOption = err opt => opt match
  | Some x => Ok x
  | None   => Err err
"#;
