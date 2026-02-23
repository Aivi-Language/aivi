# Tree

<!-- quick-info: {"kind":"module","name":"aivi.tree"} -->
The `Tree` module provides immutable rose trees (multi-way trees) and traversal helpers.
<!-- /quick-info -->

<div class="import-badge">use aivi.tree</div>

General-purpose rose tree (multi-way tree) data structure. Each node holds a value of type `A` and a `List` of child trees.

## Types

<<< ../../snippets/from_md/stdlib/math/tree/types.aivi{aivi}


## Constructors

<<< ../../snippets/from_md/stdlib/math/tree/constructors.aivi{aivi}


- `node value children`   creates a tree node with the given value and children.
- `leaf value`   creates a leaf node (a node with no children).

## Core API

| Function | Explanation |
| --- | --- |
| **node** value children<br><pre><code>`A -> List (Tree A) -> Tree A`</code></pre> | Constructs a node with `value` and `children`. |
| **leaf** value<br><pre><code>`A -> Tree A`</code></pre> | Constructs a node with no children. |
| **value** tree<br><pre><code>`Tree A -> A`</code></pre> | Returns the current node value. |
| **children** tree<br><pre><code>`Tree A -> List (Tree A)`</code></pre> | Returns direct children. |
| **map** f tree<br><pre><code>`(A -> B) -> Tree A -> Tree B`</code></pre> | Transforms every node value. |
| **fold** f seed tree<br><pre><code>`(B -> A -> B) -> B -> Tree A -> B`</code></pre> | Reduces a tree to one value. |
| **size** tree<br><pre><code>`Tree A -> Int`</code></pre> | Counts all nodes. |
| **height** tree<br><pre><code>`Tree A -> Int`</code></pre> | Returns maximum depth. |

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
