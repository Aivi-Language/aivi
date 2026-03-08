# Graph Domain

<!-- quick-info: {"kind":"module","name":"aivi.graph"} -->
The `Graph` domain models directed nodes and weighted edges so you can describe networks, dependencies, routes, and other “things connected to other things” problems.
It includes helpers for building graphs, querying neighbors, and running common traversals such as breadth-first search, depth-first search, and Dijkstra-style shortest-path queries.
<!-- /quick-info -->
<div class="import-badge">use aivi.graph<span class="domain-badge">domain</span></div>

If you have worked with social networks, dependency graphs, road maps, or workflow graphs, this is the domain you want.

This module models **directed** graphs. In a directed graph, each edge runs one way — from a source node to a target node. `NodeId` is currently `Int`, each `Edge` stores `{ from, to, weight }`, and `fromEdges` is shorthand for building weighted edges with `1.0` everywhere.

## What it is for

Typical uses include:

- finding a route from one node to another
- checking whether a dependency graph has a cycle
- computing incoming or outgoing neighbors
- turning a simple edge list into a normalized graph value

## Overview

<<< ../../snippets/from_md/stdlib/math/graph/overview.aivi{aivi}

`fromEdges`, `fromWeightedEdges`, and `addEdge` all keep `nodes` in sync with edge endpoints. If you construct a `Graph` record by hand instead, call `normalize` when you want the stored `nodes` list to include every endpoint, or use `isValid` to detect the mismatch explicitly.

## Types

<<< ../../snippets/from_md/stdlib/math/graph/features.aivi{aivi}

## Domain Definition

`Graph` overloads `+` through its domain instance. With selective imports, pull that operator into scope explicitly via `use aivi.graph (..., domain Graph)`. At an expression site you can also write `use graph in left + right`. If domain imports are new to you, see [Domains](../../syntax/domains.md).

`Graph` currently overloads only `+`. The operator deduplicates node ids while concatenating edge lists in order, which means repeated edges are preserved until you call `dedupEdges`.

<<< ../../snippets/from_md/stdlib/math/graph/block_01.aivi{aivi}


## Core helpers

| Function | What it does |
| --- | --- |
| **empty**<br><code>Graph</code> | The empty graph: `{ nodes: [], edges: [] }`. |
| **fromEdges** edges<br><code>List (NodeId, NodeId) -> Graph</code> | Builds a graph from directed edges and assigns weight `1.0` to each one. |
| **fromWeightedEdges** edges<br><code>List (NodeId, NodeId, Float) -> Graph</code> | Builds a graph from weighted directed edges. |
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
| **bfs** graph { start, end }<br><code>Graph -> { start: NodeId, end: NodeId } -> List NodeId</code> | Returns a path from `start` to `end` using breadth-first search. The second argument is a record, and the result is `[]` when no path exists. |
| **dfs** graph start<br><code>Graph -> NodeId -> List NodeId</code> | Returns nodes visited by depth-first search starting at `start`. |
| **shortestPathUnweighted** graph start goal<br><code>Graph -> NodeId -> NodeId -> List NodeId</code> | Returns the shortest path by hop count, ignores stored edge weights, and returns `[]` when no path exists. |
| **shortestPath** graph start goal<br><code>Graph -> NodeId -> NodeId -> List NodeId</code> | Returns the weighted shortest path computed by the current Dijkstra implementation. Use non-negative edge weights, and expect `[]` when no path exists. |
| **topoSort** graph<br><code>Graph -> List NodeId</code> | Returns a topological ordering for DAGs (directed acyclic graphs, meaning graphs with no directed cycles), or `[]` when the graph has a cycle. |
| **hasCycle** graph<br><code>Graph -> Bool</code> | Returns `True` when the directed graph contains a cycle. |
| **relax** dists dist edge<br><code>Map NodeId Float -> Float -> Edge -> { dists: Map NodeId Float, updated: Bool }</code> | Low-level helper for Dijkstra-style code: relaxes one edge from the current distance and reports whether it improved the destination distance. |
| **relaxEdges** dists dist edges<br><code>Map NodeId Float -> Float -> List Edge -> { dists: Map NodeId Float, count: Int }</code> | Relaxes a whole edge list and returns both the updated distance map and the number of improved destinations. |

## Verification

Automated stdlib coverage for these helpers lives in `integration-tests/stdlib/aivi/graph/graph.aivi`, covering the low-level `relax` / `relaxEdges` helpers as well as traversal and path queries. `integration-tests/runtime/graph_builtins.aivi` is also a useful broader example file for constructors, neighbor queries, traversal helpers, and cycle checks.

## Usage Examples

The example below builds a small weighted graph incrementally with `addEdge`, then runs `shortestPath` on it. Because `addEdge` keeps `nodes` synchronized with edge endpoints, this style does not need a separate `normalize` call. For incoming or outgoing neighborhood queries, see `neighbors`, `inNeighbors`, `edgesFrom`, and `edgesTo` in the table above.

<<< ../../snippets/from_md/stdlib/math/graph/usage_examples.aivi{aivi}
