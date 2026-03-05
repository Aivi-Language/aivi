pub const MODULE_NAME: &str = "aivi.generator";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.generator
export Generator
export toList, fromList, range

use aivi
use aivi.logic

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

instance Functor (Generator A) = given (A: Any) {
  map: f gen => k => z => gen (acc a => k acc (f a)) z
}

instance Filterable (Generator A) = given (A: Any) {
  filter: pred gen => k => z => gen (acc a => if pred a then k acc a else acc) z
}

instance Foldable (Generator A) = given (A: Any) {
  reduce: f init gen => gen f init
}
"#;
