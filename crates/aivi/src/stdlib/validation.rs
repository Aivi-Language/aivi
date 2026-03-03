pub const MODULE_NAME: &str = "aivi.validation";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.validation
export Validation, Valid, Invalid
export valid, invalid, isValid, isInvalid
export map, mapError, ap
export getOrElse, fold
export fromResult, toResult
export DecodeError, formatDecodeError

use aivi
use aivi.collections

Validation E A = Valid A | Invalid E

valid : A -> Validation E A
valid = a => Valid a

invalid : E -> Validation E A
invalid = e => Invalid e

isValid : Validation E A -> Bool
isValid = v => v match
  | Valid _   => True
  | Invalid _ => False

isInvalid : Validation E A -> Bool
isInvalid = v => v match
  | Valid _   => False
  | Invalid _ => True

map : (A -> B) -> Validation E A -> Validation E B
map = f v => v match
  | Valid a   => Valid (f a)
  | Invalid e => Invalid e

mapError : (E -> F) -> Validation E A -> Validation F A
mapError = f v => v match
  | Valid a   => Valid a
  | Invalid e => Invalid (f e)

ap : Validation (List E) (A -> B) -> Validation (List E) A -> Validation (List E) B
ap = vf va => (vf, va) match
  | (Valid f, Valid a)        => Valid (f a)
  | (Invalid e1, Invalid e2) => Invalid (e1 ++ e2)
  | (Invalid e, _)           => Invalid e
  | (_, Invalid e)           => Invalid e

getOrElse : A -> Validation E A -> A
getOrElse = default v => v match
  | Valid a   => a
  | Invalid _ => default

fold : (E -> B) -> (A -> B) -> Validation E A -> B
fold = onInvalid onValid v => v match
  | Valid a   => onValid a
  | Invalid e => onInvalid e

fromResult : Result E A -> Validation (List E) A
fromResult = r => r match
  | Ok a  => Valid a
  | Err e => Invalid [e]

toResult : Validation E A -> Result E A
toResult = v => v match
  | Valid a   => Ok a
  | Invalid e => Err e

DecodeError = {
  path: List Text,
  message: Text
}

formatDecodeError : DecodeError -> Text
formatDecodeError = err =>
  "at $." ++ text.join "." err.path ++ ": " ++ err.message
"#;
