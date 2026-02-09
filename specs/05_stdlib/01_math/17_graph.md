# Graph Domain

Data structures for modelling **Relationships** and **Networks**.

## What is a Graph?

In computer science, a "Graph" isn't a chart. It's a collection of things (**Nodes**) connected by lines (**Edges**).
*   **Social Networks**: People are Nodes, Friendships are Edges.
*   **Maps**: Cities are Nodes, Roads are Edges.
*   **The Web**: Pages are Nodes, Hyperlinks are Edges.

## Why this exists

Implementing Graph algorithms (like finding the shortest path between two cities) is complex and error-prone to rewrite from scratch. This domain provides a standard way to define these networks and optimized algorithms (BFS, Dijkstra) to explore them.

## Overview

```aivi
import aivi.std.math.graph use { Graph, bfs }

// Create a small network
let g = Graph.fromEdges([
  (1, 2),  // Node 1 connects to 2
  (2, 3),  // Node 2 connects to 3
  (1, 3)   // Node 1 connects to 3
])

// Find a path through the network
let path = bfs(g, start: 1, end: 3)
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

```aivi
addEdge : Graph -> Edge -> Graph
addEdge g e = { nodes: unique (g.nodes ++ [e.from, e.to]), edges: g.edges ++ [e] }

neighbors : Graph -> NodeId -> List NodeId
neighbors g n = map (.to) (filter (\e -> e.from == n) g.edges)

shortestPath : Graph -> NodeId -> NodeId -> List NodeId
shortestPath g start goal = dijkstra g start goal
```

## Usage Examples

```aivi
use aivi.std.graph

g0 = { nodes: [], edges: [] }
g1 = addEdge g0 { from: 1, to: 2, weight: 1.0 }
g2 = addEdge g1 { from: 2, to: 3, weight: 2.0 }

path = shortestPath g2 1 3
```