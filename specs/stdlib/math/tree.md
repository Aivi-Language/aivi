# Tree

<div class="import-badge">use aivi.tree</div>

General-purpose rose tree (multi-way tree) data structure. Each node holds a value of type `A` and a `List` of child trees.

## Types

<<< ../../snippets/from_md/stdlib/math/tree/types.aivi{aivi}


## Constructors

<<< ../../snippets/from_md/stdlib/math/tree/constructors.aivi{aivi}


- `node value children`   creates a tree node with the given value and children.
- `leaf value`   creates a leaf node (a node with no children).

## Traversals

<<< ../../snippets/from_md/stdlib/math/tree/traversals.aivi{aivi}


- `dfsPreorder`   depth-first preorder traversal (node, then children left-to-right).
- `dfsPostorder`   depth-first postorder traversal (children first, then node).
- `bfs`   breadth-first traversal (level-by-level).

## Construction from Flat Lists

<<< ../../snippets/from_md/stdlib/math/tree/construction_from_flat_lists.aivi{aivi}


Builds a rooted tree from a flat list of items with `(id, parentId)` relationships.

- `idFn`   extracts a unique identifier from each item.
- `parentIdFn`   returns `None` for root items, `Some parentId` for non-root items.
- Returns `None` if there are zero or multiple roots; `Some tree` for exactly one root.
