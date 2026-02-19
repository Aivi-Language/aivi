pub const MODULE_NAME: &str = "aivi.collections";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.collections
export Map, Set, Queue, Deque, Heap
export domain Collections
export domain MinHeap

use aivi

append : List a -> List a -> List a
append = left right => left match
  | [] => right
  | [x, ...xs] => [x, ...append xs right]

domain Collections over List a = {
  (++) : List a -> List a -> List a
  (++) = left right => append left right
}

domain Collections over Map k v = {
  (++) : Map k v -> Map k v -> Map k v
  (++) = left right => Map.union left right
}

domain Collections over Set a = {
  (++) : Set a -> Set a -> Set a
  (++) = left right => Set.union left right
}

domain MinHeap over Heap a = {
  empty    : Heap a
  empty    = Heap.empty
  push     : a -> Heap a -> Heap a
  push     = Heap.push
  popMin   : Heap a -> Option (a, Heap a)
  popMin   = Heap.popMin
  peekMin  : Heap a -> Option a
  peekMin  = Heap.peekMin
  size     : Heap a -> Int
  size     = Heap.size
  fromList : List a -> Heap a
  fromList = Heap.fromList
}"#;
