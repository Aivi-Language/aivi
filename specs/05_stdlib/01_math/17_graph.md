# Graph Domain

The `Graph` domain is for modelling **Relationships** and **Networks**.

In computer science, a "Graph" isn't a pie chart. It's a map of connections:
*   **Social Networks**: People connected by Friendships.
*   **Maps**: Cities connected by Roads.
*   **The Internet**: Pages connected by Links.

If you need to find the shortest path between two points or see who is friends with whom, you need a Graph. This domain provides the data structures and algorithms (like BFS and Dijkstra) to solve these problems efficiently.

## Overview

<<< ../../snippets/from_md/05_stdlib/01_math/17_graph/block_01.aivi{aivi}


## Features

<<< ../../snippets/from_md/05_stdlib/01_math/17_graph/block_02.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/05_stdlib/01_math/17_graph/block_03.aivi{aivi}

## Helper Functions

| Function | Explanation |
| --- | --- |
| **addEdge** graph edge<br><pre><code>`Graph -> Edge -> Graph`</code></pre> | Returns a new graph with the edge added and nodes updated. |
| **neighbors** graph node<br><pre><code>`Graph -> NodeId -> List NodeId`</code></pre> | Returns the outgoing neighbors of `node`. |
| **shortestPath** graph start goal<br><pre><code>`Graph -> NodeId -> NodeId -> List NodeId`</code></pre> | Returns the node path computed by Dijkstra. |

## Usage Examples

<<< ../../snippets/from_md/05_stdlib/01_math/17_graph/block_04.aivi{aivi}
