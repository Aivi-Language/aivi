# Graph Domain

<!-- quick-info: {"kind":"module","name":"aivi.graph"} -->
The `Graph` domain models nodes and edges so you can describe networks, dependencies, routes, and other “things connected to other things” problems.
It includes helpers for building graphs, querying neighbors, and running common traversals such as breadth-first search and Dijkstra shortest path.
<!-- /quick-info -->
<div class="import-badge">use aivi.graph<span class="domain-badge">domain</span></div>

If you have worked with social networks, dependency trees, road maps, or workflow graphs, this is the domain you want.

## What it is for

Typical uses include:

- finding a route from one node to another
- checking whether a dependency graph has a cycle
- computing incoming or outgoing neighbors
- turning a simple edge list into a normalized graph value

## Overview

<<< ../../snippets/from_md/stdlib/math/graph/overview.aivi{aivi}

## Features

<<< ../../snippets/from_md/stdlib/math/graph/features.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/math/graph/domain_definition.aivi{aivi}

## Core helpers

| Function | What it does |
| --- | --- |
| **empty**<br><code>Graph</code> | The empty graph: `{ nodes: [], edges: [] }`. |
| **fromEdges** edges<br><code>List (NodeId, NodeId) -> Graph</code> | Builds a graph from unweighted edges and assigns weight `1.0` to each edge. |
| **fromWeightedEdges** edges<br><code>List (NodeId, NodeId, Float) -> Graph</code> | Builds a graph from weighted edges. |
| **normalize** graph<br><code>Graph -> Graph</code> | Ensures `nodes` includes every edge endpoint and removes duplicate nodes. |
| **isValid** graph<br><code>Graph -> Bool</code> | Checks that every edge endpoint exists in `nodes`. |
| **dedupEdges** graph<br><code>Graph -> Graph</code> | Removes duplicate edges with the same `from`, `to`, and `weight`. |
| **addNode** graph node<br><code>Graph -> NodeId -> Graph</code> | Returns a new graph with `node` present in `nodes`. |
| **addEdge** graph edge<br><code>Graph -> Edge -> Graph</code> | Returns a new graph with the edge added and endpoints present in `nodes`. |
| **removeEdge** graph edge<br><code>Graph -> Edge -> Graph</code> | Returns a new graph with matching edge values removed. |
| **removeNode** graph node<br><code>Graph -> NodeId -> Graph</code> | Returns a new graph with `node` and its incident edges removed. |
| **neighbors** graph node<br><code>Graph -> NodeId -> List NodeId</code> | Returns outgoing neighbors of `node`. |
| **inNeighbors** graph node<br><code>Graph -> NodeId -> List NodeId</code> | Returns incoming neighbors of `node`. |
| **edgesFrom** graph node<br><code>Graph -> NodeId -> List Edge</code> | Returns outgoing edges from `node`. |
| **edgesTo** graph node<br><code>Graph -> NodeId -> List Edge</code> | Returns incoming edges to `node`. |
| **degreeOut** graph node<br><code>Graph -> NodeId -> Int</code> | Returns the out-degree of `node`. |
| **degreeIn** graph node<br><code>Graph -> NodeId -> Int</code> | Returns the in-degree of `node`. |
| **bfs** graph { start, end }<br><code>Graph -> { start: NodeId, end: NodeId } -> List NodeId</code> | Returns a path from `start` to `end` using breadth-first search. |
| **dfs** graph start<br><code>Graph -> NodeId -> List NodeId</code> | Returns nodes visited by depth-first search starting at `start`. |
| **shortestPathUnweighted** graph start goal<br><code>Graph -> NodeId -> NodeId -> List NodeId</code> | Returns the shortest path when all edges have equal cost. |
| **shortestPath** graph start goal<br><code>Graph -> NodeId -> NodeId -> List NodeId</code> | Returns the path computed by Dijkstra's algorithm. |
| **topoSort** graph<br><code>Graph -> List NodeId</code> | Returns a topological ordering for DAGs (directed acyclic graphs, meaning graphs with no directed cycles), or `[]` when the graph has a cycle. |
| **hasCycle** graph<br><code>Graph -> Bool</code> | Returns `True` when the directed graph contains a cycle. |

## Usage Examples

The examples focus on two common jobs: building a graph from simple data and then querying paths or neighborhood information from it.

<<< ../../snippets/from_md/stdlib/math/graph/usage_examples.aivi{aivi}
