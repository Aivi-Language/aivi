# Tree

<!-- quick-info: {"kind":"module","name":"aivi.tree"} -->
The `Tree` module provides immutable rose trees: each node stores one value and any number of child nodes.
<!-- /quick-info -->

<div class="import-badge">use aivi.tree</div>

Rose trees are a good fit for menus, document outlines, comment threads, file hierarchies, and any nested data where each item can have zero or more children.

## Types

<<< ../../snippets/from_md/stdlib/math/tree/types.aivi{aivi}

## Constructors

<<< ../../snippets/from_md/stdlib/math/tree/constructors.aivi{aivi}

- `node value children` creates a tree node with a value and a list of child trees.
- `leaf value` creates a node with no children.

## Core API

| Function | What it does |
| --- | --- |
| **node** value children<br><code>A -> List (Tree A) -> Tree A</code> | Constructs a node with `value` and `children`. |
| **leaf** value<br><code>A -> Tree A</code> | Constructs a node with no children. |
| **value** tree<br><code>Tree A -> A</code> | Returns the current node value. |
| **children** tree<br><code>Tree A -> List (Tree A)</code> | Returns the node's direct children. |
| **map** f tree<br><code>(A -> B) -> Tree A -> Tree B</code> | Transforms every node value while preserving the shape. |
| **fold** f seed tree<br><code>(B -> A -> B) -> B -> Tree A -> B</code> | Reduces a tree to one accumulated result. |
| **size** tree<br><code>Tree A -> Int</code> | Counts all nodes in the tree. |
| **height** tree<br><code>Tree A -> Int</code> | Returns the maximum depth. |

## Traversals

<<< ../../snippets/from_md/stdlib/math/tree/traversals.aivi{aivi}

- `dfsPreorder` visits a node before its children.
- `dfsPostorder` visits children before the node itself.
- `bfs` visits the tree level by level.

## Building a tree from flat data

<<< ../../snippets/from_md/stdlib/math/tree/construction_from_flat_lists.aivi{aivi}

This helper is useful when your input comes from a database or API as `(id, parentId)` pairs instead of nested values.

- `idFn` extracts a unique identifier from each item.
- `parentIdFn` returns `None` for the root item and `Some parentId` otherwise.
- The result is `None` when the input does not describe exactly one root.
