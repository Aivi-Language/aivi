pub const MODULE_NAME: &str = "aivi.graph";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.graph
export NodeId, Edge
export empty, fromEdges, fromWeightedEdges
export normalize, isValid, dedupEdges
export addNode, addEdge, removeEdge, removeNode
export neighbors, inNeighbors, edgesFrom, edgesTo, degreeOut, degreeIn
export bfs, dfs, shortestPathUnweighted, shortestPath
export topoSort, hasCycle
export relax, relaxEdges
export Graph

use aivi
use aivi.collections (Set, Queue)

NodeId = Int
Edge = { from: NodeId, to: NodeId, weight: Float }
Graph = { nodes: List NodeId, edges: List Edge }

append : List A -> List A -> List A
append = left right => left match
  | [] => right
  | [x, ...xs] => [x, ...append xs right]

reverse : List A -> List A
reverse = xs => reverseHelp xs []

reverseHelp : List A -> List A -> List A
reverseHelp = xs acc => xs match
  | [] => acc
  | [x, ...rest] => reverseHelp rest [x, ...acc]

length : List A -> Int
length = xs => xs match
  | [] => 0
  | [_x, ...rest] => 1 + length rest

uniqueNodeIds : List NodeId -> List NodeId
uniqueNodeIds = items => uniqueNodeIdsHelp items Set.empty []

uniqueNodeIdsHelp : List NodeId -> Set NodeId -> List NodeId -> List NodeId
uniqueNodeIdsHelp = items seen accRev => items match
  | [] => reverse accRev
  | [x, ...rest] =>
    if Set.has x seen then uniqueNodeIdsHelp rest seen accRev else uniqueNodeIdsHelp rest (Set.insert x seen) [x, ...accRev]

domain Graph over Graph = {
  (+) : Graph -> Graph -> Graph
  (+) = a b => { nodes: uniqueNodeIds (append a.nodes b.nodes), edges: append a.edges b.edges }
}

empty : Graph
empty = { nodes: [], edges: [] }

addNode : Graph -> NodeId -> Graph
addNode = g node =>
  if Set.has node (Set.fromList g.nodes) then g else { nodes: append g.nodes [node], edges: g.edges }

addEdge : Graph -> Edge -> Graph
addEdge = g edge => graph.addEdge g edge

fromEdgesHelp : Graph -> List (NodeId, NodeId) -> Graph
fromEdgesHelp = g pairs => pairs match
  | [] => g
  | [(from, to), ...rest] => fromEdgesHelp (addEdge g { from: from, to: to, weight: 1.0 }) rest

fromEdges : List (NodeId, NodeId) -> Graph
fromEdges = pairs => fromEdgesHelp empty pairs

fromWeightedEdgesHelp : Graph -> List (NodeId, NodeId, Float) -> Graph
fromWeightedEdgesHelp = g triples => triples match
  | [] => g
  | [(from, to, w), ...rest] => fromWeightedEdgesHelp (addEdge g { from: from, to: to, weight: w }) rest

fromWeightedEdges : List (NodeId, NodeId, Float) -> Graph
fromWeightedEdges = triples => fromWeightedEdgesHelp empty triples

edgeEq : Edge -> Edge -> Bool
edgeEq = a b => a.from == b.from && a.to == b.to && a.weight == b.weight

removeEdgeHelp : Edge -> List Edge -> List Edge
removeEdgeHelp = edge edges => edges match
  | [] => []
  | [e, ...rest] => if edgeEq e edge then removeEdgeHelp edge rest else [e, ...removeEdgeHelp edge rest]

removeEdge : Graph -> Edge -> Graph
removeEdge = g edge => { nodes: g.nodes, edges: removeEdgeHelp edge g.edges }

removeNodeHelp : NodeId -> List NodeId -> List NodeId
removeNodeHelp = node nodes => nodes match
  | [] => []
  | [n, ...rest] => if n == node then removeNodeHelp node rest else [n, ...removeNodeHelp node rest]

removeNodeEdgesHelp : NodeId -> List Edge -> List Edge
removeNodeEdgesHelp = node edges => edges match
  | [] => []
  | [e, ...rest] =>
    if e.from == node || e.to == node then removeNodeEdgesHelp node rest else [e, ...removeNodeEdgesHelp node rest]

removeNode : Graph -> NodeId -> Graph
removeNode = g node => { nodes: removeNodeHelp node g.nodes, edges: removeNodeEdgesHelp node g.edges }

edgeEndpoints : List Edge -> List NodeId
edgeEndpoints = edges => edges match
  | [] => []
  | [e, ...rest] => [e.from, e.to, ...edgeEndpoints rest]

normalize : Graph -> Graph
normalize = g => {
  endpoints = edgeEndpoints g.edges
  { nodes: uniqueNodeIds (append g.nodes endpoints), edges: g.edges }
}

isValidEdges : Set NodeId -> List Edge -> Bool
isValidEdges = nodes edges => edges match
  | [] => True
  | [e, ...rest] => if Set.has e.from nodes && Set.has e.to nodes then isValidEdges nodes rest else False

isValid : Graph -> Bool
isValid = g => isValidEdges (Set.fromList g.nodes) g.edges

edgeIn : Edge -> List Edge -> Bool
edgeIn = edge edges => edges match
  | [] => False
  | [e, ...rest] => if edgeEq e edge then True else edgeIn edge rest

dedupEdgesHelp : List Edge -> List Edge -> List Edge
dedupEdgesHelp = seenRev edges => edges match
  | [] => reverse seenRev
  | [e, ...rest] => if edgeIn e seenRev then dedupEdgesHelp seenRev rest else dedupEdgesHelp [e, ...seenRev] rest

dedupEdges : Graph -> Graph
dedupEdges = g => { nodes: g.nodes, edges: dedupEdgesHelp [] g.edges }

neighbors : Graph -> NodeId -> List NodeId
neighbors = g node => graph.neighbors g node

inNeighborsHelp : NodeId -> List Edge -> List NodeId
inNeighborsHelp = node edges => edges match
  | [] => []
  | [e, ...rest] => if e.to == node then [e.from, ...inNeighborsHelp node rest] else inNeighborsHelp node rest

inNeighbors : Graph -> NodeId -> List NodeId
inNeighbors = g node => inNeighborsHelp node g.edges

edgesFromHelp : NodeId -> List Edge -> List Edge
edgesFromHelp = node edges => edges match
  | [] => []
  | [e, ...rest] => if e.from == node then [e, ...edgesFromHelp node rest] else edgesFromHelp node rest

edgesFrom : Graph -> NodeId -> List Edge
edgesFrom = g node => edgesFromHelp node g.edges

edgesToHelp : NodeId -> List Edge -> List Edge
edgesToHelp = node edges => edges match
  | [] => []
  | [e, ...rest] => if e.to == node then [e, ...edgesToHelp node rest] else edgesToHelp node rest

edgesTo : Graph -> NodeId -> List Edge
edgesTo = g node => edgesToHelp node g.edges

degreeOut : Graph -> NodeId -> Int
degreeOut = g node => length (edgesFrom g node)

degreeIn : Graph -> NodeId -> Int
degreeIn = g node => length (edgesTo g node)

GraphIndex = { nodes: List NodeId, out: Map NodeId (List Edge), incoming: Map NodeId (List Edge) }

mapPushFront : NodeId -> Edge -> Map NodeId (List Edge) -> Map NodeId (List Edge)
mapPushFront = key edge m =>
  (Map.get key m) match
    | None => Map.insert key [edge] m
    | Some xs => Map.insert key [edge, ...xs] m

indexEdges : GraphIndex -> List Edge -> GraphIndex
indexEdges = idx edges => edges match
  | [] => idx
  | [e, ...rest] =>
    indexEdges { nodes: idx.nodes, out: mapPushFront e.from e idx.out, incoming: mapPushFront e.to e idx.incoming } rest

index : Graph -> GraphIndex
index = g => {
  g2 = normalize g
  idxRev = indexEdges { nodes: g2.nodes, out: Map.empty, incoming: Map.empty } g2.edges
  { nodes: g2.nodes, out: Map.map reverse idxRev.out, incoming: Map.map reverse idxRev.incoming }
}

edgesFromI : GraphIndex -> NodeId -> List Edge
edgesFromI = idx node => Map.getOrElse node [] idx.out

edgeTos : List Edge -> List NodeId
edgeTos = edges => edges match
  | [] => []
  | [e, ...rest] => [e.to, ...edgeTos rest]

neighborsI : GraphIndex -> NodeId -> List NodeId
neighborsI = idx node => edgeTos (edgesFromI idx node)

reconstructHelp : NodeId -> NodeId -> Map NodeId NodeId -> List NodeId -> List NodeId
reconstructHelp = start current parents accRev =>
  if current == start then [start, ...accRev] else ((Map.get current parents) ? | None => [] | Some p => reconstructHelp start p parents [current, ...accRev])

pathFromParents : NodeId -> NodeId -> Map NodeId NodeId -> List NodeId
pathFromParents = start goal parents => ((Map.get goal parents) ? | None => [] | Some _ => reconstructHelp start goal parents [])

bfsVisitNeighbors : GraphIndex -> NodeId -> NodeId -> List NodeId -> Queue NodeId -> Set NodeId -> Map NodeId NodeId -> Option (Map NodeId NodeId)
bfsVisitNeighbors = idx goal parent ns q visited parents => ns match
  | [] => bfsLoop idx goal q visited parents
  | [n, ...rest] =>
    if Set.has n visited then bfsVisitNeighbors idx goal parent rest q visited parents else bfsVisitNeighbors idx goal parent rest (Queue.enqueue n q) (Set.insert n visited) (Map.insert n parent parents)

bfsLoop : GraphIndex -> NodeId -> Queue NodeId -> Set NodeId -> Map NodeId NodeId -> Option (Map NodeId NodeId)
bfsLoop = idx goal q visited parents =>
  (Queue.dequeue q) match
    | None => None
    | Some (node, q2) =>
      if node == goal then Some parents else bfsVisitNeighbors idx goal node (neighborsI idx node) q2 visited parents

bfsPathIndexed : GraphIndex -> NodeId -> NodeId -> List NodeId
bfsPathIndexed = idx start goal => {
  q0 = Queue.enqueue start Queue.empty
  visited0 = Set.insert start Set.empty
  parents0 = Map.empty
  res = bfsLoop idx goal q0 visited0 parents0
  res match
    | None => []
    | Some parents => pathFromParents start goal parents
}

bfsPath : Graph -> NodeId -> NodeId -> List NodeId
bfsPath = g start goal => if start == goal then [start] else bfsPathIndexed (index g) start goal

bfs : Graph -> { start: NodeId, end: NodeId } -> List NodeId
bfs = g args => bfsPath g args.start args.end

shortestPathUnweighted : Graph -> NodeId -> NodeId -> List NodeId
shortestPathUnweighted = g start goal => bfsPath g start goal

dfsLoop : GraphIndex -> Set NodeId -> List NodeId -> List NodeId -> List NodeId
dfsLoop = idx visited stack outRev => stack match
  | [] => reverse outRev
  | [node, ...rest] =>
    if Set.has node visited then dfsLoop idx visited rest outRev else {
      visited2 = Set.insert node visited
      next = append (reverse (neighborsI idx node)) rest
      dfsLoop idx visited2 next [node, ...outRev]
    }

dfs : Graph -> NodeId -> List NodeId
dfs = g start => dfsLoop (index g) Set.empty [start] []

indegreeInit : List NodeId -> Map NodeId Int -> Map NodeId Int
indegreeInit = nodes m => nodes match
  | [] => m
  | [n, ...rest] => indegreeInit rest (Map.insert n 0 m)

indegreeEdges : Map NodeId Int -> List NodeId -> Map NodeId (List Edge) -> Map NodeId Int
indegreeEdges = indeg nodes inMap => nodes match
  | [] => indeg
  | [n, ...rest] => indegreeEdges (Map.insert n (length (Map.getOrElse n [] inMap)) indeg) rest inMap

enqueueZero : List NodeId -> Map NodeId Int -> Queue NodeId -> Queue NodeId
enqueueZero = nodes indeg q => nodes match
  | [] => q
  | [n, ...rest] => if Map.getOrElse n 0 indeg == 0 then enqueueZero rest indeg (Queue.enqueue n q) else enqueueZero rest indeg q

topoRelax : List Edge -> Map NodeId Int -> Queue NodeId -> { indeg: Map NodeId Int, q: Queue NodeId }
topoRelax = edges indeg q => edges match
  | [] => { indeg: indeg, q: q }
  | [e, ...rest] => {
    to = e.to
    cur = Map.getOrElse to 0 indeg
    next = cur - 1
    indeg2 = Map.insert to next indeg
    q2 = if next == 0 then Queue.enqueue to q else q
    topoRelax rest indeg2 q2
  }

topoLoop : GraphIndex -> Map NodeId Int -> Queue NodeId -> List NodeId -> List NodeId
topoLoop = idx indeg q outRev =>
  (Queue.dequeue q) match
    | None => outRev
    | Some (node, q2) => {
      res = topoRelax (edgesFromI idx node) indeg q2
      topoLoop idx res.indeg res.q [node, ...outRev]
    }

topoSortIndexed : GraphIndex -> List NodeId
topoSortIndexed = idx => {
  indeg0 = indegreeInit idx.nodes Map.empty
  indeg = indegreeEdges indeg0 idx.nodes idx.incoming
  q0 = enqueueZero idx.nodes indeg Queue.empty
  outRev = topoLoop idx indeg q0 []
  out = reverse outRev
  if length out == length idx.nodes then out else []
}

topoSort : Graph -> List NodeId
topoSort = g => topoSortIndexed (index g)

hasCycle : Graph -> Bool
hasCycle = g => length (topoSort g) != length ((index g).nodes)

shortestPath : Graph -> NodeId -> NodeId -> List NodeId
shortestPath = g start goal => graph.shortestPath g start goal

// Relax a single edge against immutable distances. Returns updated map + update flag.
relax : Map NodeId Float -> Float -> Edge -> { dists: Map NodeId Float, updated: Bool }
relax = dists currentDist edge => {
  newDist = currentDist + edge.weight
  maybeDist = Map.get edge.to dists
  shouldUpdate = maybeDist ? | None => True | Some oldDist => newDist < oldDist
  if shouldUpdate
    then {
      dists: Map.insert edge.to newDist dists
      updated: True
    }
    else { dists: dists, updated: False }
}

// Relax all edges in a list, returning updated distances + number of updates.
relaxEdges : Map NodeId Float -> Float -> List Edge -> { dists: Map NodeId Float, count: Int }
relaxEdges = dists currentDist edges => relaxEdgesHelp dists currentDist edges 0

relaxEdgesHelp : Map NodeId Float -> Float -> List Edge -> Int -> { dists: Map NodeId Float, count: Int }
relaxEdgesHelp = dists currentDist edges count => edges match
  | [] => { dists: dists, count: count }
  | [e, ...rest] => {
    step = relax dists currentDist e
    next = if step.updated then count + 1 else count
    relaxEdgesHelp step.dists currentDist rest next
  }
"#;
