pub const MODULE_NAME: &str = "aivi.graph";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.graph
export NodeId, Edge
export addEdge, neighbors, shortestPath
export domain Graph

use aivi
use aivi.collections (Set)

NodeId = Int
Edge = { from: NodeId, to: NodeId, weight: Float }
Graph = { nodes: List NodeId, edges: List Edge }

append : List A -> List A -> List A
append left right = left ?
  | [] => right
  | [x, ...xs] => [x, ...append xs right]

unique : List NodeId -> List NodeId
unique items = Set.toList (Set.fromList items)

domain Graph over Graph = {
  (+) : Graph -> Graph -> Graph
  (+) a b = { nodes: unique (append a.nodes b.nodes), edges: append a.edges b.edges }
}

addEdge : Graph -> Edge -> Graph
addEdge g edge = graph.addEdge g edge

neighbors : Graph -> NodeId -> List NodeId
neighbors g node = graph.neighbors g node

shortestPath : Graph -> NodeId -> NodeId -> List NodeId
shortestPath g start goal = graph.shortestPath g start goal"#;
