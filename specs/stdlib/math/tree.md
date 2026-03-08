# Tree

<!-- quick-info: {"kind":"module","name":"aivi.tree"} -->
The `Tree` module provides immutable rose trees: each node stores one value and any number of child nodes.
<!-- /quick-info -->

<div class="import-badge">use aivi.tree</div>

Rose trees are a good fit for menus, document outlines, comment threads, file hierarchies, and any nested data where each item can have zero or more children.

## Types

<<< ../../snippets/from_md/stdlib/math/tree/types.aivi{aivi}

`Tree` is a non-empty rose tree. You can pattern-match on `Node value children` directly when constructors and traversal helpers are not enough.

## Constructors

<<< ../../snippets/from_md/stdlib/math/tree/constructors.aivi{aivi}

- `node value children` creates a tree node with a value and a list of child trees.
- `leaf value` creates a node with no children. It is shorthand for `node value []`.

<<< ../../snippets/from_md/stdlib/math/tree/block_01.aivi{aivi}


## Core API

| Function | What it does |
| --- | --- |
| **node** value children<br><code>A -> List (Tree A) -> Tree A</code> | Constructs a node with `value` and `children`. |
| **leaf** value<br><code>A -> Tree A</code> | Constructs a node with no children. |
| **dfsPreorder** tree<br><code>Tree A -> List A</code> | Visits the current node first, then each subtree from left to right. |
| **dfsPostorder** tree<br><code>Tree A -> List A</code> | Visits each subtree first and the current node last. |
| **bfs** tree<br><code>Tree A -> List A</code> | Visits the tree level by level from left to right. |
| **fromListBy** idFn parentIdFn items<br><code>(A -> K) -> (A -> Option K) -> List A -> Option (Tree A)</code> | Builds one rooted tree from flat `(id, parentId)` style data when the input has exactly one root. |

## Traversals

<<< ../../snippets/from_md/stdlib/math/tree/traversals.aivi{aivi}

- `dfsPreorder` visits a node before its children.
- `dfsPostorder` visits children before the node itself.
- `bfs` visits the tree level by level.

<<< ../../snippets/from_md/stdlib/math/tree/block_02.aivi{aivi}


## Building a tree from flat data

<<< ../../snippets/from_md/stdlib/math/tree/construction_from_flat_lists.aivi{aivi}

This helper is useful when your input comes from a database or API as `(id, parentId)` pairs instead of nested values.

- `idFn` extracts a unique identifier from each item.
- `parentIdFn` returns `None` for the root item and `Some parentId` otherwise.
- The result is `None` when the input does not describe exactly one root.
- Provide unique ids and parent ids that refer to items in the same list.

<<< ../../snippets/from_md/stdlib/math/tree/block_03.aivi{aivi}


## Verification

Current behavior is exercised in `integration-tests/stdlib/aivi/tree/Tree.aivi`, covering the public constructors, all three traversal helpers, and the success and failure cases for `fromListBy`.
