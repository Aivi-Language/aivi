pub const MODULE_NAME: &str = "aivi.tree";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.tree
export Tree
export node, leaf
export dfsPreorder, dfsPostorder, bfs
export fromListBy

use aivi
use aivi.collections (Queue)

Tree A = Node A (List (Tree A))

node : A -> List (Tree A) -> Tree A
node = value children => Node value children

leaf : A -> Tree A
leaf = value => Node value []

valueOf : Tree A -> A
valueOf = t => t ?
  | Node v _ => v

childrenOf : Tree A -> List (Tree A)
childrenOf = t => t ?
  | Node _ cs => cs

append : List A -> List A -> List A
append = xs ys => xs ?
  | [] => ys
  | [h, ...t] => [h, ...append t ys]

reverse : List A -> List A
reverse = xs => reverseGo xs []

reverseGo = xs acc => xs ?
  | [] => acc
  | [h, ...t] => reverseGo t [h, ...acc]

// Depth-first preorder traversal
// Returns node values in the order: node, then children left-to-right.
dfsPreorder : Tree A -> List A
dfsPreorder = tree => dfsPreorderGo [tree] []

dfsPreorderGo : List (Tree A) -> List A -> List A
dfsPreorderGo = stack outRev => stack ?
  | [] => reverse outRev
  | [t, ...rest] => {
    // push children in reverse so leftmost is visited first
    childrenRev = reverse (childrenOf t)
    stack2 = append childrenRev rest
    dfsPreorderGo stack2 [valueOf t, ...outRev]
  }

// Depth-first postorder traversal
// Returns node values after all children.
dfsPostorder : Tree A -> List A
dfsPostorder = tree => dfsPostorderGo [tree] [] []

// We simulate recursion with two stacks: one for work, one for output nodes.
dfsPostorderGo : List (Tree A) -> List (Tree A) -> List A -> List A
dfsPostorderGo = work outStack outRev => work ?
  | [] => {
    // pop outStack into outRev
    dfsPostorderDrain outStack outRev
  }
  | [t, ...rest] => {
    work2 = append t.children rest
    dfsPostorderGo work2 [t, ...outStack] outRev
  }

dfsPostorderDrain : List (Tree A) -> List A -> List A
dfsPostorderDrain = outStack outRev => outStack ?
  | [] => reverse outRev
  | [t, ...rest] => dfsPostorderDrain rest [valueOf t, ...outRev]

// Breadth-first traversal
bfs : Tree A -> List A
bfs = tree => bfsLoop (Queue.enqueue tree Queue.empty) []

bfsLoop : Queue (Tree A) -> List A -> List A
bfsLoop = q outRev => (Queue.dequeue q) ?
  | None => reverse outRev
  | Some (t, q2) => {
    q3 = bfsEnqueueChildren (childrenOf t) q2
    bfsLoop q3 [valueOf t, ...outRev]
  }

bfsEnqueueChildren : List (Tree A) -> Queue (Tree A) -> Queue (Tree A)
bfsEnqueueChildren = children q => children ?
  | [] => q
  | [c, ...rest] => bfsEnqueueChildren rest (Queue.enqueue c q)

// Build a rooted tree from a flat list with (id, parentId) relations.
//
// Contract:
// - idFn extracts a stable id for each item.
// - parentIdFn returns None for root nodes.
// Returns:
// - None if no root exists or multiple roots exist.
// - Some tree if exactly one root exists.
fromListBy : (A -> K) -> (A -> Option K) -> List A -> Option (Tree A)
fromListBy = idFn parentIdFn items => {
  // Build children map: parentId -> List A (children)
  childrenMap = fromListChildrenMap idFn parentIdFn items Map.empty
  roots = rootsFromList idFn parentIdFn items []
  roots ?
    | [] => None
    | [root] => Some (buildTree idFn (idFn root) root childrenMap)
    | _ => None
}

fromListChildrenMap : (A -> K) -> (A -> Option K) -> List A -> Map K (List A) -> Map K (List A)
fromListChildrenMap = idFn parentIdFn items acc => items ?
  | [] => acc
  | [x, ...rest] => {
    pidOpt = parentIdFn x
    acc2 = pidOpt ?
      | None => acc
      | Some pid => mapPush pid x acc
    fromListChildrenMap idFn parentIdFn rest acc2
  }

mapPush : K -> A -> Map K (List A) -> Map K (List A)
mapPush = key value m => (Map.get key m) ?
  | None => Map.insert key [value] m
  | Some xs => Map.insert key [value, ...xs] m

rootsFromList : (A -> K) -> (A -> Option K) -> List A -> List A -> List A
rootsFromList = idFn parentIdFn items accRev => items ?
  | [] => reverse accRev
  | [x, ...rest] =>
    (parentIdFn x) ?
      | None => rootsFromList idFn parentIdFn rest [x, ...accRev]
      | Some _ => rootsFromList idFn parentIdFn rest accRev

buildTree : (A -> K) -> K -> A -> Map K (List A) -> Tree A
buildTree = idFn id item childrenMap => {
  // preserve original list order by reversing once (since we pushed-front)
  children = reverse (Map.getOrElse id [] childrenMap)
  Node item (buildForest idFn children childrenMap [])
}

buildForest : (A -> K) -> List A -> Map K (List A) -> List (Tree A) -> List (Tree A)
buildForest = idFn items childrenMap accRev => items ?
  | [] => reverse accRev
  | [x, ...rest] => {
    id = idFn x
    childTree = buildTree idFn id x childrenMap
    buildForest idFn rest childrenMap [childTree, ...accRev]
  }
"#;
