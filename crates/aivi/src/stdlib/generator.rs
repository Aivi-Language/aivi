pub const MODULE_NAME: &str = "aivi.generator";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.generator
export Generator
export foldl, toList, fromList, map, filter, range

use aivi

Generator A = (R -> A -> R) -> R -> R

foldl : (B -> A -> B) -> B -> Generator A -> B
foldl = step init gen => gen step init

revAppend : List A -> List A -> List A
revAppend = xs acc => xs match
  | []        => acc
  | [h, ...t] => revAppend t [h, ...acc]

reverse : List A -> List A
reverse = xs => revAppend xs []

consRev : List A -> A -> List A
consRev = acc x => [x, ...acc]

toList : Generator A -> List A
toList = gen =>
  // Build in reverse via cons, then reverse once (linear).
  reverse (foldl consRev [] gen)

fromList : List A -> Generator A
fromList = xs => k => z => xs match
  | []        => z
  | [h, ...t] => fromList t k (k z h)

map : (A -> B) -> Generator A -> Generator B
mapStep : (A -> B) -> (R -> B -> R) -> R -> A -> R
mapStep = f k acc a => k acc (f a)

map = f gen => k => z => gen (mapStep f k) z

filter : (A -> Bool) -> Generator A -> Generator A
filterStep : (A -> Bool) -> (R -> A -> R) -> R -> A -> R
filterStep = pred k acc a => if pred a then k acc a else acc

filter = pred gen => k => z => gen (filterStep pred k) z

range : Int -> Int -> Generator Int
range = start end => k => z =>
  if start >= end then z else range (start + 1) end k (k z start)
"#;
