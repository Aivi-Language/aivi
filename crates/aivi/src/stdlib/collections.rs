pub const MODULE_NAME: &str = "aivi.collections";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.collections = {
  export Map, Set, Queue, Deque, Heap
  export domain Collections

  use aivi

  domain Collections over Map k v = {
    (++) : Map k v -> Map k v -> Map k v
    (++) left right = Map.union left right
  }

  domain Collections over Set a = {
    (++) : Set a -> Set a -> Set a
    (++) left right = Set.union left right
  }
}
"#;
