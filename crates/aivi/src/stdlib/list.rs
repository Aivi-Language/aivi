pub const MODULE_NAME: &str = "aivi.list";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.list
export empty, isEmpty, length, reverse
export foldr, scanl
export take, drop, takeWhile, dropWhile, partition, find, findMap
export at, indexOf, zip, zipWith, unzip, intersperse, chunk, dedup, uniqueBy
export any, all, elem, dropLast, last
export traverse_, sequence_

use aivi

empty : List A
empty = List.empty

isEmpty : List A -> Bool
isEmpty = xs => List.isEmpty xs

length : List A -> Int
length = xs => List.length xs

reverse : List A -> List A
reverse = xs => List.reverse xs

foldr : (A -> B -> B) -> B -> List A -> B
foldr = f init xs => List.foldr f init xs

scanl : (B -> A -> B) -> B -> List A -> List B
scanl = f init xs => List.scanl f init xs

take : Int -> List A -> List A
take = count xs => List.take count xs

drop : Int -> List A -> List A
drop = count xs => List.drop count xs

takeWhile : (A -> Bool) -> List A -> List A
takeWhile = pred xs => List.takeWhile pred xs

dropWhile : (A -> Bool) -> List A -> List A
dropWhile = pred xs => List.dropWhile pred xs

partition : (A -> Bool) -> List A -> (List A, List A)
partition = pred xs => List.partition pred xs

find : (A -> Bool) -> List A -> Option A
find = pred xs => List.find pred xs

findMap : (A -> Option B) -> List A -> Option B
findMap = f xs => List.findMap f xs

at : Int -> List A -> Option A
at = idx xs => List.at idx xs

indexOf : A -> List A -> Option Int
indexOf = needle xs => List.indexOf needle xs

zip : List A -> List B -> List (A, B)
zip = left right => List.zip left right

zipWith : (A -> B -> C) -> List A -> List B -> List C
zipWith = f left right => List.zipWith f left right

unzip : List (A, B) -> (List A, List B)
unzip = pairs => List.unzip pairs

intersperse : A -> List A -> List A
intersperse = sep xs => List.intersperse sep xs

chunk : Int -> List A -> List (List A)
chunk = size xs => List.chunk size xs

dedup : List A -> List A
dedup = xs => List.dedup xs

uniqueBy : (A -> B) -> List A -> List A
uniqueBy = f xs => List.uniqueBy f xs

any : (A -> Bool) -> List A -> Bool
any = pred xs => List.any pred xs

all : (A -> Bool) -> List A -> Bool
all = pred xs => List.all pred xs

elem : A -> List A -> Bool
elem = needle xs => List.elem needle xs

dropLast : List A -> List A
dropLast = xs => List.dropLast xs

last : List A -> Option A
last = xs => List.last xs

traverse_ : (A -> Effect E B) -> List A -> Effect E Unit
traverse_ = f xs => xs match
  | []           => pure Unit
  | [x, ...rest] => do Effect {
    _ <- f x
    traverse_ f rest
  }

sequence_ : List (Effect E A) -> Effect E Unit
sequence_ = xs => {
  run = x => x
  traverse_ run xs
}

"#;
