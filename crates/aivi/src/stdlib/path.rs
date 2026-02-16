pub const MODULE_NAME: &str = "aivi.path";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.path
export domain Path
export Path
export parse, toString, isAbsolute, parent, fileName, normalize

use aivi

Path = { absolute: Bool, segments: List Text }

isAbsolute : Path -> Bool
isAbsolute = p => p.absolute

append : List A -> List A -> List A
append = left right => left ?
  | [] => right
  | [x, ...xs] => [x, ...append xs right]

revAppend : List A -> List A -> List A
revAppend = xs ys => xs ?
  | [] => ys
  | [x, ...rest] => revAppend rest [x, ...ys]

reverse : List A -> List A
reverse = xs => revAppend xs []

isEmpty : List A -> Bool
isEmpty = xs => xs ?
  | [] => True
  | _ => False

normalizeAcc : Bool -> List Text -> List Text -> List Text
normalizeAcc = absolute acc segments => segments ?
  | [] => acc
  | [s, ...rest] => if s == "" || s == "." then normalizeAcc absolute acc rest else if s == ".." then (acc ? | [] => if absolute then normalizeAcc absolute [] rest else normalizeAcc absolute ["..", ...acc] rest | [a, ...accTail] => if a == ".." then normalizeAcc absolute ["..", ...acc] rest else normalizeAcc absolute accTail rest) else normalizeAcc absolute [s, ...acc] rest

normalizeSegments : Bool -> List Text -> List Text
normalizeSegments = absolute segments => reverse (normalizeAcc absolute [] segments)

normalize : Path -> Path
normalize = p => { ...p, segments: normalizeSegments p.absolute p.segments }

parse : Text -> Path
parse = raw => {
  cleaned = text.trim raw
  cleaned = text.replaceAll "\\" "/" cleaned
  absolute = text.startsWith "/" cleaned
  parts = text.split "/" cleaned
  { absolute: absolute, segments: normalizeSegments absolute parts }
}

joinSegments : List Text -> Text
joinSegments = segments => segments ?
  | [] => ""
  | [x] => x
  | [x, ...xs] => text.concat [x, "/", joinSegments xs]

toString : Path -> Text
toString = p =>
  if isEmpty p.segments then (if p.absolute then "/" else ".") else
  (if p.absolute then text.concat ["/", joinSegments p.segments] else joinSegments p.segments)

init : List A -> List A
init = xs => xs ?
  | [] => []
  | [_] => []
  | [x, ...rest] => [x, ...init rest]

parent : Path -> Option Path
parent = p => if isEmpty p.segments then None else Some { absolute: p.absolute, segments: init p.segments }

last : List A -> Option A
last = xs => xs ?
  | [] => None
  | [x] => Some x
  | [_, ...rest] => last rest

fileName : Path -> Option Text
fileName = p => last p.segments

domain Path over Path = {
  (/) : Path -> Path -> Path
  (/) = base other =>
    if other.absolute then other else normalize { absolute: base.absolute, segments: append base.segments other.segments }
}"#;
