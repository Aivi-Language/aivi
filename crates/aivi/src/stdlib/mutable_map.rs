pub const MODULE_NAME: &str = "aivi.mutableMap";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.mutableMap
export MutableMap

use aivi

// Effect-scoped mutable map backed by an immutable Map under the hood.
// All operations are effectful: you can only read/write inside do Effect { ... } blocks.

// Create a MutableMap from an existing immutable Map.
create : Map k v -> Effect e (MutableMap k v)
create = MutableMap.create

// Create an empty MutableMap.
empty : Effect e (MutableMap k v)
empty = MutableMap.empty Unit

// Effectfully look up a key, returns Option.
get : k -> MutableMap k v -> Effect e (Option v)
get = MutableMap.get

// Effectfully look up a key with a default.
getOrElse : k -> v -> MutableMap k v -> Effect e v
getOrElse = MutableMap.getOrElse

// Effectfully insert a key-value pair.
insert : k -> v -> MutableMap k v -> Effect e Unit
insert = MutableMap.insert

// Effectfully remove a key.
remove : k -> MutableMap k v -> Effect e Unit
remove = MutableMap.remove

// Effectfully check if a key exists.
has : k -> MutableMap k v -> Effect e Bool
has = MutableMap.has

// Effectfully get the number of entries.
size : MutableMap k v -> Effect e Int
size = MutableMap.size

// Snapshot the mutable map into an immutable Map.
freeze : MutableMap k v -> Effect e (Map k v)
freeze = MutableMap.freeze

// Effectfully get all keys.
keys : MutableMap k v -> Effect e (List k)
keys = MutableMap.keys

// Effectfully get all values.
values : MutableMap k v -> Effect e (List v)
values = MutableMap.values

// Atomically apply a pure function to the underlying Map.
modify : (Map k v -> Map k v) -> MutableMap k v -> Effect e Unit
modify = MutableMap.modify
"#;
