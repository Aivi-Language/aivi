# Graph Domain

<!-- quick-info: {"kind":"module","name":"aivi.graph"} -->
The `Graph` domain is for modelling **Relationships** and **Networks**.

In computer science, a "Graph" isn't a pie chart. It's a map of connections:
*   **Social Networks**: People connected by Friendships.
*   **Maps**: Cities connected by Roads.
*   **The Internet**: Pages connected by Links.

If you need to find the shortest path between two points or see who is friends with whom, you need a Graph. This domain provides the data structures and algorithms (like BFS and Dijkstra) to solve these problems efficiently.

<!-- /quick-info -->
<div class="import-badge">use aivi.graph<span class="domain-badge">domain</span></div>

## Overview

<<< ../../snippets/from_md/stdlib/math/graph/overview.aivi{aivi}


## Features

<<< ../../snippets/from_md/stdlib/math/graph/features.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/math/graph/domain_definition.aivi{aivi}

## Helper Functions

| Function | Explanation |
| --- | --- |
| **empty**<br><pre><code>`Graph`</code></pre> | The empty graph: `{ nodes: [], edges: [] }`. |
| **fromEdges** edges<br><pre><code>`List (NodeId, NodeId) -> Graph`</code></pre> | Builds a graph from unweighted edges (default weight `1.0`). |
| **fromWeightedEdges** edges<br><pre><code>`List (NodeId, NodeId, Float) -> Graph`</code></pre> | Builds a graph from weighted edges. |
| **normalize** graph<br><pre><code>`Graph -> Graph`</code></pre> | Ensures `nodes` includes all edge endpoints and removes duplicate nodes. |
| **isValid** graph<br><pre><code>`Graph -> Bool`</code></pre> | Checks that every edge endpoint exists in `nodes`. |
| **dedupEdges** graph<br><pre><code>`Graph -> Graph`</code></pre> | Removes duplicate edges (same `from`, `to`, and `weight`). |
| **addNode** graph node<br><pre><code>`Graph -> NodeId -> Graph`</code></pre> | Returns a new graph with `node` present in `nodes`. |
| **addEdge** graph edge<br><pre><code>`Graph -> Edge -> Graph`</code></pre> | Returns a new graph with the edge added and nodes updated. |
| **removeEdge** graph edge<br><pre><code>`Graph -> Edge -> Graph`</code></pre> | Returns a new graph with the matching edge(s) removed. |
| **removeNode** graph node<br><pre><code>`Graph -> NodeId -> Graph`</code></pre> | Returns a new graph with `node` removed (and any incident edges removed). |
| **neighbors** graph node<br><pre><code>`Graph -> NodeId -> List NodeId`</code></pre> | Returns the outgoing neighbors of `node`. |
| **inNeighbors** graph node<br><pre><code>`Graph -> NodeId -> List NodeId`</code></pre> | Returns the incoming neighbors of `node`. |
| **edgesFrom** graph node<br><pre><code>`Graph -> NodeId -> List Edge`</code></pre> | Returns the outgoing edges from `node`. |
| **edgesTo** graph node<br><pre><code>`Graph -> NodeId -> List Edge`</code></pre> | Returns the incoming edges to `node`. |
| **degreeOut** graph node<br><pre><code>`Graph -> NodeId -> Int`</code></pre> | Returns the out-degree of `node`. |
| **degreeIn** graph node<br><pre><code>`Graph -> NodeId -> Int`</code></pre> | Returns the in-degree of `node`. |
| **bfs** graph { start, end }<br><pre><code>`Graph -> { start: NodeId, end: NodeId } -> List NodeId`</code></pre> | Returns a node path from `start` to `end` using breadth-first search. |
| **dfs** graph start<br><pre><code>`Graph -> NodeId -> List NodeId`</code></pre> | Returns nodes visited by depth-first search starting at `start`. |
| **shortestPathUnweighted** graph start goal<br><pre><code>`Graph -> NodeId -> NodeId -> List NodeId`</code></pre> | Returns the shortest node path when all edge weights are equal (BFS-based). |
| **shortestPath** graph start goal<br><pre><code>`Graph -> NodeId -> NodeId -> List NodeId`</code></pre> | Returns the node path computed by Dijkstra. |
| **topoSort** graph<br><pre><code>`Graph -> List NodeId`</code></pre> | Returns a topological ordering for DAGs (returns `[]` if there is a cycle). |
| **hasCycle** graph<br><pre><code>`Graph -> Bool`</code></pre> | Returns `True` if the graph has a directed cycle. |

## Usage Examples

<<< ../../snippets/from_md/stdlib/math/graph/usage_examples.aivi{aivi}
