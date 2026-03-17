pub const MODULE_NAME: &str = "aivi.generator";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.generator
export Generator
export toList, fromList, range
export map, filter, reduce

use aivi

Generator A = (R -> A -> R) -> R -> R

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
  reverse (gen consRev [])

fromList : List A -> Generator A
fromList = xs => k => z => xs match
  | []        => z
  | [h, ...t] => fromList t k (k z h)

range : Int -> Int -> Generator Int
range = start end => k => z =>
  if start >= end then z else range (start + 1) end k (k z start)

map : (A -> B) -> Generator A -> Generator B
map = f gen => k => z => {
  step = acc a => k acc (f a)
  gen step z
}

filter : (A -> Bool) -> Generator A -> Generator A
filter = pred gen => k => z => {
  step = acc a => if pred a then k acc a else acc
  gen step z
}

reduce : (B -> A -> B) -> B -> Generator A -> B
reduce = f init gen => gen f init
"#;
