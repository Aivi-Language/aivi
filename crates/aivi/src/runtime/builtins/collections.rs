use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

use im::{HashMap as ImHashMap, HashSet as ImHashSet, Vector as ImVector};

use super::util::{builtin, expect_list, list_value, make_none, make_some};
use crate::runtime::values::KeyValue;
use crate::runtime::{RuntimeError, Value};

pub(super) fn build_collections_record() -> Value {
    let mut fields = std::collections::HashMap::new();
    fields.insert("map".to_string(), build_map_record());
    fields.insert("set".to_string(), build_set_record());
    fields.insert("queue".to_string(), build_queue_record());
    fields.insert("deque".to_string(), build_deque_record());
    fields.insert("heap".to_string(), build_heap_record());
    Value::Record(Arc::new(fields))
}

pub(super) fn build_map_record() -> Value {
    let mut fields = std::collections::HashMap::new();
    fields.insert("empty".to_string(), Value::Map(Arc::new(ImHashMap::new())));
    fields.insert(
        "size".to_string(),
        builtin("map.size", 1, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.size")?;
            Ok(Value::Int(map.len() as i64))
        }),
    );
    fields.insert(
        "has".to_string(),
        builtin("map.has", 2, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.has")?;
            let key = key_from_value(&args.pop().unwrap(), "map.has")?;
            Ok(Value::Bool(map.contains_key(&key)))
        }),
    );
    fields.insert(
        "get".to_string(),
        builtin("map.get", 2, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.get")?;
            let key = key_from_value(&args.pop().unwrap(), "map.get")?;
            Ok(match map.get(&key) {
                Some(value) => make_some(value.clone()),
                None => make_none(),
            })
        }),
    );
    fields.insert(
        "insert".to_string(),
        builtin("map.insert", 3, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.insert")?;
            let value = args.pop().unwrap();
            let key = key_from_value(&args.pop().unwrap(), "map.insert")?;
            let mut out = (*map).clone();
            out.insert(key, value);
            Ok(Value::Map(Arc::new(out)))
        }),
    );
    fields.insert(
        "update".to_string(),
        builtin("map.update", 3, |mut args, runtime| {
            let map = expect_map(args.pop().unwrap(), "map.update")?;
            let func = args.pop().unwrap();
            let key = key_from_value(&args.pop().unwrap(), "map.update")?;
            if let Some(current) = map.get(&key) {
                let updated = runtime.apply(func, current.clone())?;
                let mut out = (*map).clone();
                out.insert(key, updated);
                Ok(Value::Map(Arc::new(out)))
            } else {
                Ok(Value::Map(map))
            }
        }),
    );
    fields.insert(
        "remove".to_string(),
        builtin("map.remove", 2, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.remove")?;
            let key = key_from_value(&args.pop().unwrap(), "map.remove")?;
            let mut out = (*map).clone();
            out.remove(&key);
            Ok(Value::Map(Arc::new(out)))
        }),
    );
    fields.insert(
        "map".to_string(),
        builtin("map.map", 2, |mut args, runtime| {
            let map = expect_map(args.pop().unwrap(), "map.map")?;
            let func = args.pop().unwrap();
            let mut out = ImHashMap::new();
            for (key, value) in map.iter() {
                let mapped = runtime.apply(func.clone(), value.clone())?;
                out.insert(key.clone(), mapped);
            }
            Ok(Value::Map(Arc::new(out)))
        }),
    );
    fields.insert(
        "mapWithKey".to_string(),
        builtin("map.mapWithKey", 2, |mut args, runtime| {
            let map = expect_map(args.pop().unwrap(), "map.mapWithKey")?;
            let func = args.pop().unwrap();
            let mut out = ImHashMap::new();
            for (key, value) in map.iter() {
                let partial = runtime.apply(func.clone(), key.to_value())?;
                let mapped = runtime.apply(partial, value.clone())?;
                out.insert(key.clone(), mapped);
            }
            Ok(Value::Map(Arc::new(out)))
        }),
    );
    fields.insert(
        "keys".to_string(),
        builtin("map.keys", 1, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.keys")?;
            let items = map.iter().map(|(key, _)| key.to_value()).collect();
            Ok(list_value(items))
        }),
    );
    fields.insert(
        "values".to_string(),
        builtin("map.values", 1, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.values")?;
            let items = map.iter().map(|(_, value)| value.clone()).collect();
            Ok(list_value(items))
        }),
    );
    fields.insert(
        "entries".to_string(),
        builtin("map.entries", 1, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.entries")?;
            let items = map
                .iter()
                .map(|(key, value)| Value::Tuple(vec![key.to_value(), value.clone()]))
                .collect();
            Ok(list_value(items))
        }),
    );
    fields.insert(
        "fromList".to_string(),
        builtin("map.fromList", 1, |mut args, _| {
            let items = expect_list(args.pop().unwrap(), "map.fromList")?;
            let mut out = ImHashMap::new();
            for item in items.iter() {
                match item {
                    Value::Tuple(entries) if entries.len() == 2 => {
                        let key = key_from_value(&entries[0], "map.fromList")?;
                        out.insert(key, entries[1].clone());
                    }
                    _ => {
                        return Err(RuntimeError::Message(
                            "map.fromList expects List (k, v)".to_string(),
                        ))
                    }
                }
            }
            Ok(Value::Map(Arc::new(out)))
        }),
    );
    fields.insert(
        "toList".to_string(),
        builtin("map.toList", 1, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.toList")?;
            let items = map
                .iter()
                .map(|(key, value)| Value::Tuple(vec![key.to_value(), value.clone()]))
                .collect();
            Ok(list_value(items))
        }),
    );
    fields.insert(
        "union".to_string(),
        builtin("map.union", 2, |mut args, _| {
            let right = expect_map(args.pop().unwrap(), "map.union")?;
            let left = expect_map(args.pop().unwrap(), "map.union")?;
            let mut out = (*left).clone();
            for (key, value) in right.iter() {
                out.insert(key.clone(), value.clone());
            }
            Ok(Value::Map(Arc::new(out)))
        }),
    );
    fields.insert(
        "getOrElse".to_string(),
        builtin("map.getOrElse", 3, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.getOrElse")?;
            let default = args.pop().unwrap();
            let key = key_from_value(&args.pop().unwrap(), "map.getOrElse")?;
            Ok(map.get(&key).cloned().unwrap_or(default))
        }),
    );
    fields.insert(
        "alter".to_string(),
        builtin("map.alter", 3, |mut args, runtime| {
            let map = expect_map(args.pop().unwrap(), "map.alter")?;
            let func = args.pop().unwrap();
            let key = key_from_value(&args.pop().unwrap(), "map.alter")?;
            let current = match map.get(&key) {
                Some(value) => make_some(value.clone()),
                None => make_none(),
            };
            let updated = runtime.apply(func, current)?;
            match updated {
                Value::Constructor { name, args } if name == "None" && args.is_empty() => {
                    let mut out = (*map).clone();
                    out.remove(&key);
                    Ok(Value::Map(Arc::new(out)))
                }
                Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
                    let mut out = (*map).clone();
                    out.insert(key, args[0].clone());
                    Ok(Value::Map(Arc::new(out)))
                }
                other => Err(RuntimeError::Message(format!(
                    "map.alter expects f : Option v -> Option v, got {}",
                    crate::runtime::format_value(&other)
                ))),
            }
        }),
    );
    fields.insert(
        "mergeWith".to_string(),
        builtin("map.mergeWith", 3, |mut args, runtime| {
            let right = expect_map(args.pop().unwrap(), "map.mergeWith")?;
            let left = expect_map(args.pop().unwrap(), "map.mergeWith")?;
            let combine = args.pop().unwrap();
            let mut out = (*left).clone();
            for (key, right_val) in right.iter() {
                match left.get(key) {
                    Some(left_val) => {
                        let partial = runtime.apply(combine.clone(), key.to_value())?;
                        let partial = runtime.apply(partial, left_val.clone())?;
                        let merged = runtime.apply(partial, right_val.clone())?;
                        out.insert(key.clone(), merged);
                    }
                    None => {
                        out.insert(key.clone(), right_val.clone());
                    }
                }
            }
            Ok(Value::Map(Arc::new(out)))
        }),
    );
    fields.insert(
        "filterWithKey".to_string(),
        builtin("map.filterWithKey", 2, |mut args, runtime| {
            let map = expect_map(args.pop().unwrap(), "map.filterWithKey")?;
            let pred = args.pop().unwrap();
            let mut out = ImHashMap::new();
            for (key, value) in map.iter() {
                let partial = runtime.apply(pred.clone(), key.to_value())?;
                let ok = runtime.apply(partial, value.clone())?;
                match ok {
                    Value::Bool(true) => {
                        out.insert(key.clone(), value.clone());
                    }
                    Value::Bool(false) => {}
                    other => {
                        return Err(RuntimeError::Message(format!(
                            "map.filterWithKey expects Bool predicate, got {}",
                            crate::runtime::format_value(&other)
                        )))
                    }
                }
            }
            Ok(Value::Map(Arc::new(out)))
        }),
    );
    fields.insert(
        "foldWithKey".to_string(),
        builtin("map.foldWithKey", 3, |mut args, runtime| {
            let map = expect_map(args.pop().unwrap(), "map.foldWithKey")?;
            let init = args.pop().unwrap();
            let func = args.pop().unwrap();
            let mut acc = init;
            for (key, value) in map.iter() {
                let partial = runtime.apply(func.clone(), acc)?;
                let partial = runtime.apply(partial, key.to_value())?;
                acc = runtime.apply(partial, value.clone())?;
            }
            Ok(acc)
        }),
    );
    Value::Record(Arc::new(fields))
}

pub(super) fn build_set_record() -> Value {
    let mut fields = std::collections::HashMap::new();
    fields.insert("empty".to_string(), Value::Set(Arc::new(ImHashSet::new())));
    fields.insert(
        "size".to_string(),
        builtin("set.size", 1, |mut args, _| {
            let set = expect_set(args.pop().unwrap(), "set.size")?;
            Ok(Value::Int(set.len() as i64))
        }),
    );
    fields.insert(
        "has".to_string(),
        builtin("set.has", 2, |mut args, _| {
            let set = expect_set(args.pop().unwrap(), "set.has")?;
            let key = key_from_value(&args.pop().unwrap(), "set.has")?;
            Ok(Value::Bool(set.contains(&key)))
        }),
    );
    fields.insert(
        "contains".to_string(),
        builtin("set.contains", 2, |mut args, _| {
            let set = expect_set(args.pop().unwrap(), "set.contains")?;
            let key = key_from_value(&args.pop().unwrap(), "set.contains")?;
            Ok(Value::Bool(set.contains(&key)))
        }),
    );
    fields.insert(
        "insert".to_string(),
        builtin("set.insert", 2, |mut args, _| {
            let set = expect_set(args.pop().unwrap(), "set.insert")?;
            let key = key_from_value(&args.pop().unwrap(), "set.insert")?;
            let mut out = (*set).clone();
            out.insert(key);
            Ok(Value::Set(Arc::new(out)))
        }),
    );
    fields.insert(
        "remove".to_string(),
        builtin("set.remove", 2, |mut args, _| {
            let set = expect_set(args.pop().unwrap(), "set.remove")?;
            let key = key_from_value(&args.pop().unwrap(), "set.remove")?;
            let mut out = (*set).clone();
            out.remove(&key);
            Ok(Value::Set(Arc::new(out)))
        }),
    );
    fields.insert(
        "union".to_string(),
        builtin("set.union", 2, |mut args, _| {
            let right = expect_set(args.pop().unwrap(), "set.union")?;
            let left = expect_set(args.pop().unwrap(), "set.union")?;
            let out = (*left).clone().union((*right).clone());
            Ok(Value::Set(Arc::new(out)))
        }),
    );
    fields.insert(
        "intersection".to_string(),
        builtin("set.intersection", 2, |mut args, _| {
            let right = expect_set(args.pop().unwrap(), "set.intersection")?;
            let left = expect_set(args.pop().unwrap(), "set.intersection")?;
            let out = (*left).clone().intersection((*right).clone());
            Ok(Value::Set(Arc::new(out)))
        }),
    );
    fields.insert(
        "difference".to_string(),
        builtin("set.difference", 2, |mut args, _| {
            let right = expect_set(args.pop().unwrap(), "set.difference")?;
            let left = expect_set(args.pop().unwrap(), "set.difference")?;
            let out = (*left).clone().relative_complement((*right).clone());
            Ok(Value::Set(Arc::new(out)))
        }),
    );
    fields.insert(
        "fromList".to_string(),
        builtin("set.fromList", 1, |mut args, _| {
            let items = expect_list(args.pop().unwrap(), "set.fromList")?;
            let mut out = ImHashSet::new();
            for item in items.iter() {
                let key = key_from_value(item, "set.fromList")?;
                out.insert(key);
            }
            Ok(Value::Set(Arc::new(out)))
        }),
    );
    fields.insert(
        "toList".to_string(),
        builtin("set.toList", 1, |mut args, _| {
            let set = expect_set(args.pop().unwrap(), "set.toList")?;
            let items = set.iter().map(|key| key.to_value()).collect();
            Ok(list_value(items))
        }),
    );
    Value::Record(Arc::new(fields))
}

pub(super) fn build_queue_record() -> Value {
    let mut fields = std::collections::HashMap::new();
    fields.insert("empty".to_string(), Value::Queue(Arc::new(ImVector::new())));
    fields.insert(
        "enqueue".to_string(),
        builtin("queue.enqueue", 2, |mut args, _| {
            let queue = expect_queue(args.pop().unwrap(), "queue.enqueue")?;
            let value = args.pop().unwrap();
            let mut out = (*queue).clone();
            out.push_back(value);
            Ok(Value::Queue(Arc::new(out)))
        }),
    );
    fields.insert(
        "dequeue".to_string(),
        builtin("queue.dequeue", 1, |mut args, _| {
            let queue = expect_queue(args.pop().unwrap(), "queue.dequeue")?;
            let mut out = (*queue).clone();
            match out.pop_front() {
                Some(value) => Ok(make_some(Value::Tuple(vec![
                    value,
                    Value::Queue(Arc::new(out)),
                ]))),
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "peek".to_string(),
        builtin("queue.peek", 1, |mut args, _| {
            let queue = expect_queue(args.pop().unwrap(), "queue.peek")?;
            match queue.front() {
                Some(value) => Ok(make_some(value.clone())),
                None => Ok(make_none()),
            }
        }),
    );
    Value::Record(Arc::new(fields))
}

pub(super) fn build_deque_record() -> Value {
    let mut fields = std::collections::HashMap::new();
    fields.insert("empty".to_string(), Value::Deque(Arc::new(ImVector::new())));
    fields.insert(
        "pushFront".to_string(),
        builtin("deque.pushFront", 2, |mut args, _| {
            let deque = expect_deque(args.pop().unwrap(), "deque.pushFront")?;
            let value = args.pop().unwrap();
            let mut out = (*deque).clone();
            out.push_front(value);
            Ok(Value::Deque(Arc::new(out)))
        }),
    );
    fields.insert(
        "pushBack".to_string(),
        builtin("deque.pushBack", 2, |mut args, _| {
            let deque = expect_deque(args.pop().unwrap(), "deque.pushBack")?;
            let value = args.pop().unwrap();
            let mut out = (*deque).clone();
            out.push_back(value);
            Ok(Value::Deque(Arc::new(out)))
        }),
    );
    fields.insert(
        "popFront".to_string(),
        builtin("deque.popFront", 1, |mut args, _| {
            let deque = expect_deque(args.pop().unwrap(), "deque.popFront")?;
            let mut out = (*deque).clone();
            match out.pop_front() {
                Some(value) => Ok(make_some(Value::Tuple(vec![
                    value,
                    Value::Deque(Arc::new(out)),
                ]))),
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "popBack".to_string(),
        builtin("deque.popBack", 1, |mut args, _| {
            let deque = expect_deque(args.pop().unwrap(), "deque.popBack")?;
            let mut out = (*deque).clone();
            match out.pop_back() {
                Some(value) => Ok(make_some(Value::Tuple(vec![
                    value,
                    Value::Deque(Arc::new(out)),
                ]))),
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "peekFront".to_string(),
        builtin("deque.peekFront", 1, |mut args, _| {
            let deque = expect_deque(args.pop().unwrap(), "deque.peekFront")?;
            match deque.front() {
                Some(value) => Ok(make_some(value.clone())),
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "peekBack".to_string(),
        builtin("deque.peekBack", 1, |mut args, _| {
            let deque = expect_deque(args.pop().unwrap(), "deque.peekBack")?;
            match deque.back() {
                Some(value) => Ok(make_some(value.clone())),
                None => Ok(make_none()),
            }
        }),
    );
    Value::Record(Arc::new(fields))
}

pub(super) fn build_heap_record() -> Value {
    let mut fields = std::collections::HashMap::new();
    fields.insert(
        "empty".to_string(),
        Value::Heap(Arc::new(BinaryHeap::new())),
    );
    fields.insert(
        "push".to_string(),
        builtin("heap.push", 2, |mut args, _| {
            let heap = expect_heap(args.pop().unwrap(), "heap.push")?;
            let value = args.pop().unwrap();
            let key = key_from_value(&value, "heap.push")?;
            let mut out = (*heap).clone();
            out.push(Reverse(key));
            Ok(Value::Heap(Arc::new(out)))
        }),
    );
    fields.insert(
        "popMin".to_string(),
        builtin("heap.popMin", 1, |mut args, _| {
            let heap = expect_heap(args.pop().unwrap(), "heap.popMin")?;
            let mut out = (*heap).clone();
            match out.pop() {
                Some(Reverse(value)) => Ok(make_some(Value::Tuple(vec![
                    value.to_value(),
                    Value::Heap(Arc::new(out)),
                ]))),
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "peekMin".to_string(),
        builtin("heap.peekMin", 1, |mut args, _| {
            let heap = expect_heap(args.pop().unwrap(), "heap.peekMin")?;
            match heap.peek() {
                Some(Reverse(value)) => Ok(make_some(value.to_value())),
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "size".to_string(),
        builtin("heap.size", 1, |mut args, _| {
            let heap = expect_heap(args.pop().unwrap(), "heap.size")?;
            Ok(Value::Int(heap.len() as i64))
        }),
    );
    fields.insert(
        "fromList".to_string(),
        builtin("heap.fromList", 1, |mut args, _| {
            let list = expect_list(args.pop().unwrap(), "heap.fromList")?;
            let mut heap = BinaryHeap::new();
            for item in list.iter() {
                let key = key_from_value(item, "heap.fromList")?;
                heap.push(Reverse(key));
            }
            Ok(Value::Heap(Arc::new(heap)))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn key_from_value(value: &Value, ctx: &str) -> Result<KeyValue, RuntimeError> {
    KeyValue::try_from_value(value)
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects a hashable key")))
}

fn expect_map(value: Value, ctx: &str) -> Result<Arc<ImHashMap<KeyValue, Value>>, RuntimeError> {
    match value {
        Value::Map(entries) => Ok(entries),
        _ => Err(RuntimeError::Message(format!(
            "{ctx} expects Map, got {value:?}",
        ))),
    }
}

fn expect_set(value: Value, ctx: &str) -> Result<Arc<ImHashSet<KeyValue>>, RuntimeError> {
    match value {
        Value::Set(entries) => Ok(entries),
        _ => Err(RuntimeError::Message(format!("{ctx} expects Set, got {value:?}"))),
    }
}

fn expect_queue(value: Value, ctx: &str) -> Result<Arc<ImVector<Value>>, RuntimeError> {
    match value {
        Value::Queue(items) => Ok(items),
        _ => Err(RuntimeError::Message(format!("{ctx} expects Queue"))),
    }
}

fn expect_deque(value: Value, ctx: &str) -> Result<Arc<ImVector<Value>>, RuntimeError> {
    match value {
        Value::Deque(items) => Ok(items),
        _ => Err(RuntimeError::Message(format!("{ctx} expects Deque"))),
    }
}

fn expect_heap(
    value: Value,
    ctx: &str,
) -> Result<Arc<BinaryHeap<Reverse<KeyValue>>>, RuntimeError> {
    match value {
        Value::Heap(items) => Ok(items),
        _ => Err(RuntimeError::Message(format!("{ctx} expects Heap"))),
    }
}
