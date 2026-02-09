# Standard Library: Graph Domain

## Module

```aivi
module aivi.std.graph = {
  export domain Graph
  export NodeId, Edge, Graph
  export addEdge, neighbors, shortestPath
}
```

## Types

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