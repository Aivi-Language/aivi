use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;

use ordered_float::OrderedFloat;

use super::util::{builtin, expect_float, expect_int, expect_list, expect_record, list_ints};
use crate::runtime::{RuntimeError, Value};

pub(super) fn build_graph_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "addEdge".to_string(),
        builtin("graph.addEdge", 2, |mut args, _| {
            let edge_value = args.pop().unwrap();
            let graph_value = args.pop().unwrap();
            let (from, to, weight) = edge_from_value(edge_value, "graph.addEdge")?;
            let (mut nodes, mut edges) = graph_from_value(graph_value, "graph.addEdge")?;
            if !nodes.contains(&from) {
                nodes.push(from);
            }
            if !nodes.contains(&to) {
                nodes.push(to);
            }
            edges.push((from, to, weight));
            Ok(graph_to_value(nodes, edges))
        }),
    );
    fields.insert(
        "neighbors".to_string(),
        builtin("graph.neighbors", 2, |mut args, _| {
            let node = expect_int(args.pop().unwrap(), "graph.neighbors")?;
            let (_, edges) = graph_from_value(args.pop().unwrap(), "graph.neighbors")?;
            let neighbors: Vec<Value> = edges
                .iter()
                .filter_map(|(from, to, _)| {
                    if *from == node {
                        Some(Value::Int(*to))
                    } else {
                        None
                    }
                })
                .collect();
            Ok(Value::List(Arc::new(neighbors)))
        }),
    );
    fields.insert(
        "shortestPath".to_string(),
        builtin("graph.shortestPath", 3, |mut args, _| {
            let goal = expect_int(args.pop().unwrap(), "graph.shortestPath")?;
            let start = expect_int(args.pop().unwrap(), "graph.shortestPath")?;
            let (nodes, edges) = graph_from_value(args.pop().unwrap(), "graph.shortestPath")?;
            if start == goal {
                return Ok(Value::List(Arc::new(vec![Value::Int(start)])));
            }
            let mut adjacency: HashMap<i64, Vec<(i64, f64)>> = HashMap::new();
            for (from, to, weight) in edges {
                adjacency
                    .entry(from)
                    .or_default()
                    .push((to, weight));
            }
            for node in nodes {
                adjacency.entry(node).or_default();
            }
            let mut dist: HashMap<i64, f64> = HashMap::new();
            let mut prev: HashMap<i64, i64> = HashMap::new();
            let mut heap = BinaryHeap::new();
            dist.insert(start, 0.0);
            heap.push((Reverse(OrderedFloat(0.0)), start));
            while let Some((Reverse(OrderedFloat(cost)), node)) = heap.pop() {
                if cost > *dist.get(&node).unwrap_or(&f64::INFINITY) {
                    continue;
                }
                if node == goal {
                    break;
                }
                if let Some(neighbors) = adjacency.get(&node) {
                    for (next, weight) in neighbors {
                        let next_cost = cost + *weight;
                        let current = dist.get(next).copied().unwrap_or(f64::INFINITY);
                        if next_cost < current {
                            dist.insert(*next, next_cost);
                            prev.insert(*next, node);
                            heap.push((Reverse(OrderedFloat(next_cost)), *next));
                        }
                    }
                }
            }
            if !prev.contains_key(&goal) {
                return Ok(Value::List(Arc::new(Vec::new())));
            }
            let mut path = Vec::new();
            let mut current = goal;
            path.push(Value::Int(current));
            while current != start {
                match prev.get(&current) {
                    Some(node) => {
                        current = *node;
                        path.push(Value::Int(current));
                    }
                    None => return Ok(Value::List(Arc::new(Vec::new()))),
                }
            }
            path.reverse();
            Ok(Value::List(Arc::new(path)))
        }),
    );
    Value::Record(Arc::new(fields))
}
fn edge_from_value(value: Value, ctx: &str) -> Result<(i64, i64, f64), RuntimeError> {
    let record = expect_record(value, ctx)?;
    let from = match record.get("from") {
        Some(value) => expect_int(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Edge.from"
            )))
        }
    };
    let to = match record.get("to") {
        Some(value) => expect_int(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Edge.to"
            )))
        }
    };
    let weight = match record.get("weight") {
        Some(value) => expect_float(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Edge.weight"
            )))
        }
    };
    Ok((from, to, weight))
}
fn graph_from_value(
    value: Value,
    ctx: &str,
) -> Result<(Vec<i64>, Vec<(i64, i64, f64)>), RuntimeError> {
    let record = expect_record(value, ctx)?;
    let nodes_list = match record.get("nodes") {
        Some(value) => expect_list(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Graph.nodes"
            )))
        }
    };
    let edges_list = match record.get("edges") {
        Some(value) => expect_list(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Graph.edges"
            )))
        }
    };
    let nodes = list_ints(&nodes_list, ctx)?;
    let mut edges = Vec::with_capacity(edges_list.len());
    for edge in edges_list.iter() {
        edges.push(edge_from_value(edge.clone(), ctx)?);
    }
    Ok((nodes, edges))
}
fn graph_to_value(nodes: Vec<i64>, edges: Vec<(i64, i64, f64)>) -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "nodes".to_string(),
        Value::List(Arc::new(nodes.into_iter().map(Value::Int).collect())),
    );
    let list = edges
        .into_iter()
        .map(|(from, to, weight)| {
            let mut edge = HashMap::new();
            edge.insert("from".to_string(), Value::Int(from));
            edge.insert("to".to_string(), Value::Int(to));
            edge.insert("weight".to_string(), Value::Float(weight));
            Value::Record(Arc::new(edge))
        })
        .collect();
    fields.insert("edges".to_string(), Value::List(Arc::new(list)));
    Value::Record(Arc::new(fields))
}
