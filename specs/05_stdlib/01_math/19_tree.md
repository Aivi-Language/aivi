# Tree

Module: `aivi.tree`

General-purpose rose tree (multi-way tree) data structure. Each node holds a value of type `A` and a `List` of child trees.

## Types

```aivi
Tree A =
  | Node A (List (Tree A))
```

## Constructors

```aivi
node : A -> List (Tree A) -> Tree A
leaf : A -> Tree A
```

- `node value children`   creates a tree node with the given value and children.
- `leaf value`   creates a leaf node (a node with no children).

## Traversals

```aivi
dfsPreorder  : Tree A -> List A
dfsPostorder : Tree A -> List A
bfs          : Tree A -> List A
```

- `dfsPreorder`   depth-first preorder traversal (node, then children left-to-right).
- `dfsPostorder`   depth-first postorder traversal (children first, then node).
- `bfs`   breadth-first traversal (level-by-level).

## Construction from Flat Lists

```aivi
fromListBy : (A -> K) -> (A -> Option K) -> List A -> Option (Tree A)
```

Builds a rooted tree from a flat list of items with `(id, parentId)` relationships.

- `idFn`   extracts a unique identifier from each item.
- `parentIdFn`   returns `None` for root items, `Some parentId` for non-root items.
- Returns `None` if there are zero or multiple roots; `Some tree` for exactly one root.
