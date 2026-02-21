pub const MODULE_NAME: &str = "aivi.map";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.map
export empty, size, has, get, insert, update, remove
export map, mapWithKey, keys, values, entries
export fromList, toList, union, getOrElse
export alter, mergeWith, filterWithKey, foldWithKey

use aivi

empty : Map K V
empty = Map.empty

size : Map K V -> Int
size = m => Map.size m

has : K -> Map K V -> Bool
has = k m => Map.has k m

get : K -> Map K V -> Option V
get = k m => Map.get k m

insert : K -> V -> Map K V -> Map K V
insert = k v m => Map.insert k v m

update : K -> (V -> V) -> Map K V -> Map K V
update = k f m => Map.update k f m

remove : K -> Map K V -> Map K V
remove = k m => Map.remove k m

map : (V -> V2) -> Map K V -> Map K V2
map = f m => Map.map f m

mapWithKey : (K -> V -> V2) -> Map K V -> Map K V2
mapWithKey = f m => Map.mapWithKey f m

keys : Map K V -> List K
keys = m => Map.keys m

values : Map K V -> List V
values = m => Map.values m

entries : Map K V -> List (K, V)
entries = m => Map.entries m

fromList : List (K, V) -> Map K V
fromList = xs => Map.fromList xs

toList : Map K V -> List (K, V)
toList = m => Map.toList m

union : Map K V -> Map K V -> Map K V
union = a b => Map.union a b

getOrElse : K -> V -> Map K V -> V
getOrElse = k def m => Map.get k m match
  | Some v => v
  | None   => def

alter : K -> (Option V -> Option V) -> Map K V -> Map K V
alter = k f m => Map.alter k f m

mergeWith : (K -> V -> V -> V) -> Map K V -> Map K V -> Map K V
mergeWith = f a b => Map.mergeWith f a b

filterWithKey : (K -> V -> Bool) -> Map K V -> Map K V
filterWithKey = f m => Map.filterWithKey f m

foldWithKey : (B -> K -> V -> B) -> B -> Map K V -> B
foldWithKey = f init m => Map.foldWithKey f init m
"#;
