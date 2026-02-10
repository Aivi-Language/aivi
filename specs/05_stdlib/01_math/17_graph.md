# Graph Domain

The `Graph` domain is for modelling **Relationships** and **Networks**.

In computer science, a "Graph" isn't a pie chart. It's a map of connections:
*   **Social Networks**: People connected by Friendships.
*   **Maps**: Cities connected by Roads.
*   **The Internet**: Pages connected by Links.

If you need to find the shortest path between two points or see who is friends with whom, you need a Graph. This domain provides the data structures and algorithms (like BFS and Dijkstra) to solve these problems efficiently.

## Overview

```aivi
use aivi.graph (Graph, bfs)

// Create a small network
g = Graph.fromEdges([
  (1, 2),  // Node 1 connects to 2
  (2, 3),  // Node 2 connects to 3
  (1, 3)   // Node 1 connects to 3
])

// Find a path through the network
path = bfs(g, start: 1, end: 3)
```


## Features

```aivi
NodeId = Int
Edge = { from: NodeId, to: NodeId, weight: Float }
Graph = { nodes: List NodeId, edges: List Edge }
```

## Domain Definition

```aivi
domain Graph over Graph = {
  (+) : Graph -> Graph -> Graph
  (+) a b = { nodes: unique (a.nodes ++ b.nodes), edges: a.edges ++ b.edges }
}
```

## Helper Functions

| Function | Explanation |
| --- | --- |
| **addEdge** graph edge<br><pre><code>`Graph -> Edge -> Graph`</code></pre> | Returns a new graph with the edge added and nodes updated. |
| **neighbors** graph node<br><pre><code>`Graph -> NodeId -> List NodeId`</code></pre> | Returns the outgoing neighbors of `node`. |
| **shortestPath** graph start goal<br><pre><code>`Graph -> NodeId -> NodeId -> List NodeId`</code></pre> | Returns the node path computed by Dijkstra. |

## Usage Examples

```aivi
use aivi.graph

g0 = { nodes: [], edges: [] }
g1 = addEdge g0 { from: 1, to: 2, weight: 1.0 }
g2 = addEdge g1 { from: 2, to: 3, weight: 2.0 }

path = shortestPath g2 1 3
```
