use std::collections::HashSet;
use std::sync::Arc;

use super::util::{builtin, expect_int, expect_list, list_value, make_none, make_some};
use crate::runtime::values::KeyValue;
use crate::runtime::{values_equal, RuntimeError, Value};

fn expect_callable(value: Value, ctx: &str) -> Result<Value, RuntimeError> {
    match value {
        Value::Builtin(_) | Value::MultiClause(_) => Ok(value),
        _ => Err(RuntimeError::Message(format!("{ctx} expects a function"))),
    }
}

fn expect_bool(value: Value, ctx: &str) -> Result<bool, RuntimeError> {
    match value {
        Value::Bool(value) => Ok(value),
        other => Err(RuntimeError::Message(format!(
            "{ctx} expects Bool, got {}",
            crate::runtime::format_value(&other)
        ))),
    }
}

fn option_from_value(value: Value, ctx: &str) -> Result<Option<Value>, RuntimeError> {
    match value {
        Value::Constructor { name, args } if name == "None" && args.is_empty() => Ok(None),
        Value::Constructor { name, mut args } if name == "Some" && args.len() == 1 => {
            Ok(Some(args.remove(0)))
        }
        other => Err(RuntimeError::Message(format!(
            "{ctx} expects Option, got {}",
            crate::runtime::format_value(&other)
        ))),
    }
}

pub(super) fn build_list_record() -> Value {
    let mut fields = std::collections::HashMap::new();

    fields.insert("empty".to_string(), list_value(Vec::new()));

    fields.insert(
        "isEmpty".to_string(),
        builtin("list.isEmpty", 1, |mut args, _| {
            let items = expect_list(args.pop().unwrap(), "List.isEmpty")?;
            Ok(Value::Bool(items.is_empty()))
        }),
    );

    fields.insert(
        "length".to_string(),
        builtin("list.length", 1, |mut args, _| {
            let items = expect_list(args.pop().unwrap(), "List.length")?;
            Ok(Value::Int(items.len() as i64))
        }),
    );

    fields.insert(
        "map".to_string(),
        builtin("list.map", 2, |mut args, runtime| {
            let items = expect_list(args.pop().unwrap(), "List.map")?;
            let func = expect_callable(args.pop().unwrap(), "List.map")?;
            let mut out = Vec::with_capacity(items.len());
            for item in items.iter().cloned() {
                out.push(runtime.apply(func.clone(), item)?);
            }
            Ok(list_value(out))
        }),
    );

    fields.insert(
        "filter".to_string(),
        builtin("list.filter", 2, |mut args, runtime| {
            let items = expect_list(args.pop().unwrap(), "List.filter")?;
            let pred = expect_callable(args.pop().unwrap(), "List.filter")?;
            let mut out = Vec::new();
            for item in items.iter().cloned() {
                let ok = runtime.apply(pred.clone(), item.clone())?;
                if expect_bool(ok, "List.filter")? {
                    out.push(item);
                }
            }
            Ok(list_value(out))
        }),
    );

    fields.insert(
        "flatMap".to_string(),
        builtin("list.flatMap", 2, |mut args, runtime| {
            let items = expect_list(args.pop().unwrap(), "List.flatMap")?;
            let func = expect_callable(args.pop().unwrap(), "List.flatMap")?;
            let mut out = Vec::new();
            for item in items.iter().cloned() {
                let mapped = runtime.apply(func.clone(), item)?;
                let mapped_items = expect_list(mapped, "List.flatMap")?;
                out.extend(mapped_items.iter().cloned());
            }
            Ok(list_value(out))
        }),
    );

    fields.insert(
        "foldl".to_string(),
        builtin("list.foldl", 3, |mut args, runtime| {
            let items = expect_list(args.pop().unwrap(), "List.foldl")?;
            let init = args.pop().unwrap();
            let step = expect_callable(args.pop().unwrap(), "List.foldl")?;
            let mut acc = init;
            for item in items.iter().cloned() {
                let partial = runtime.apply(step.clone(), acc)?;
                acc = runtime.apply(partial, item)?;
            }
            Ok(acc)
        }),
    );

    fields.insert(
        "foldr".to_string(),
        builtin("list.foldr", 3, |mut args, runtime| {
            let items = expect_list(args.pop().unwrap(), "List.foldr")?;
            let init = args.pop().unwrap();
            let step = expect_callable(args.pop().unwrap(), "List.foldr")?;
            let mut acc = init;
            for item in items.iter().rev().cloned() {
                let partial = runtime.apply(step.clone(), item)?;
                acc = runtime.apply(partial, acc)?;
            }
            Ok(acc)
        }),
    );

    fields.insert(
        "scanl".to_string(),
        builtin("list.scanl", 3, |mut args, runtime| {
            let items = expect_list(args.pop().unwrap(), "List.scanl")?;
            let init = args.pop().unwrap();
            let step = expect_callable(args.pop().unwrap(), "List.scanl")?;
            let mut out = Vec::with_capacity(items.len() + 1);
            let mut acc = init;
            out.push(acc.clone());
            for item in items.iter().cloned() {
                let partial = runtime.apply(step.clone(), acc)?;
                acc = runtime.apply(partial, item)?;
                out.push(acc.clone());
            }
            Ok(list_value(out))
        }),
    );

    fields.insert(
        "take".to_string(),
        builtin("list.take", 2, |mut args, _| {
            let items = expect_list(args.pop().unwrap(), "List.take")?;
            let n = expect_int(args.pop().unwrap(), "List.take")?;
            if n <= 0 {
                return Ok(list_value(Vec::new()));
            }
            let n = usize::try_from(n).unwrap_or(0);
            let out = items.iter().take(n).cloned().collect::<Vec<_>>();
            Ok(list_value(out))
        }),
    );

    fields.insert(
        "drop".to_string(),
        builtin("list.drop", 2, |mut args, _| {
            let items = expect_list(args.pop().unwrap(), "List.drop")?;
            let n = expect_int(args.pop().unwrap(), "List.drop")?;
            if n <= 0 {
                return Ok(Value::List(items));
            }
            let n = usize::try_from(n).unwrap_or(usize::MAX);
            let out = items.iter().skip(n).cloned().collect::<Vec<_>>();
            Ok(list_value(out))
        }),
    );

    fields.insert(
        "takeWhile".to_string(),
        builtin("list.takeWhile", 2, |mut args, runtime| {
            let items = expect_list(args.pop().unwrap(), "List.takeWhile")?;
            let pred = expect_callable(args.pop().unwrap(), "List.takeWhile")?;
            let mut out = Vec::new();
            for item in items.iter().cloned() {
                let ok = runtime.apply(pred.clone(), item.clone())?;
                if expect_bool(ok, "List.takeWhile")? {
                    out.push(item);
                } else {
                    break;
                }
            }
            Ok(list_value(out))
        }),
    );

    fields.insert(
        "dropWhile".to_string(),
        builtin("list.dropWhile", 2, |mut args, runtime| {
            let items = expect_list(args.pop().unwrap(), "List.dropWhile")?;
            let pred = expect_callable(args.pop().unwrap(), "List.dropWhile")?;
            let mut idx = 0usize;
            for item in items.iter().cloned() {
                let ok = runtime.apply(pred.clone(), item)?;
                if expect_bool(ok, "List.dropWhile")? {
                    idx += 1;
                } else {
                    break;
                }
            }
            let out = items.iter().skip(idx).cloned().collect::<Vec<_>>();
            Ok(list_value(out))
        }),
    );

    fields.insert(
        "partition".to_string(),
        builtin("list.partition", 2, |mut args, runtime| {
            let items = expect_list(args.pop().unwrap(), "List.partition")?;
            let pred = expect_callable(args.pop().unwrap(), "List.partition")?;
            let mut yes = Vec::new();
            let mut no = Vec::new();
            for item in items.iter().cloned() {
                let ok = runtime.apply(pred.clone(), item.clone())?;
                if expect_bool(ok, "List.partition")? {
                    yes.push(item);
                } else {
                    no.push(item);
                }
            }
            Ok(Value::Tuple(vec![list_value(yes), list_value(no)]))
        }),
    );

    fields.insert(
        "find".to_string(),
        builtin("list.find", 2, |mut args, runtime| {
            let items = expect_list(args.pop().unwrap(), "List.find")?;
            let pred = expect_callable(args.pop().unwrap(), "List.find")?;
            for item in items.iter().cloned() {
                let ok = runtime.apply(pred.clone(), item.clone())?;
                if expect_bool(ok, "List.find")? {
                    return Ok(make_some(item));
                }
            }
            Ok(make_none())
        }),
    );

    fields.insert(
        "findMap".to_string(),
        builtin("list.findMap", 2, |mut args, runtime| {
            let items = expect_list(args.pop().unwrap(), "List.findMap")?;
            let func = expect_callable(args.pop().unwrap(), "List.findMap")?;
            for item in items.iter().cloned() {
                let mapped = runtime.apply(func.clone(), item)?;
                match option_from_value(mapped, "List.findMap")? {
                    Some(value) => return Ok(make_some(value)),
                    None => continue,
                }
            }
            Ok(make_none())
        }),
    );

    fields.insert(
        "at".to_string(),
        builtin("list.at", 2, |mut args, _| {
            let items = expect_list(args.pop().unwrap(), "List.at")?;
            let idx = expect_int(args.pop().unwrap(), "List.at")?;
            if idx < 0 {
                return Ok(make_none());
            }
            let idx = usize::try_from(idx).unwrap_or(usize::MAX);
            match items.get(idx) {
                Some(value) => Ok(make_some(value.clone())),
                None => Ok(make_none()),
            }
        }),
    );

    fields.insert(
        "indexOf".to_string(),
        builtin("list.indexOf", 2, |mut args, _runtime| {
            let items = expect_list(args.pop().unwrap(), "List.indexOf")?;
            let needle = args.pop().unwrap();
            for (idx, item) in items.iter().enumerate() {
                if values_equal(item, &needle) {
                    return Ok(make_some(Value::Int(idx as i64)));
                }
            }
            Ok(make_none())
        }),
    );

    fields.insert(
        "zip".to_string(),
        builtin("list.zip", 2, |mut args, _| {
            let right = expect_list(args.pop().unwrap(), "List.zip")?;
            let left = expect_list(args.pop().unwrap(), "List.zip")?;
            let n = left.len().min(right.len());
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                out.push(Value::Tuple(vec![left[i].clone(), right[i].clone()]));
            }
            Ok(list_value(out))
        }),
    );

    fields.insert(
        "zipWith".to_string(),
        builtin("list.zipWith", 3, |mut args, runtime| {
            let right = expect_list(args.pop().unwrap(), "List.zipWith")?;
            let left = expect_list(args.pop().unwrap(), "List.zipWith")?;
            let func = expect_callable(args.pop().unwrap(), "List.zipWith")?;
            let n = left.len().min(right.len());
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let partial = runtime.apply(func.clone(), left[i].clone())?;
                out.push(runtime.apply(partial, right[i].clone())?);
            }
            Ok(list_value(out))
        }),
    );

    fields.insert(
        "unzip".to_string(),
        builtin("list.unzip", 1, |mut args, _| {
            let pairs = expect_list(args.pop().unwrap(), "List.unzip")?;
            let mut left = Vec::with_capacity(pairs.len());
            let mut right = Vec::with_capacity(pairs.len());
            for pair in pairs.iter() {
                match pair {
                    Value::Tuple(items) if items.len() == 2 => {
                        left.push(items[0].clone());
                        right.push(items[1].clone());
                    }
                    _ => {
                        return Err(RuntimeError::Message(
                            "List.unzip expects List (a, b)".to_string(),
                        ))
                    }
                }
            }
            Ok(Value::Tuple(vec![list_value(left), list_value(right)]))
        }),
    );

    fields.insert(
        "intersperse".to_string(),
        builtin("list.intersperse", 2, |mut args, _| {
            let items = expect_list(args.pop().unwrap(), "List.intersperse")?;
            let sep = args.pop().unwrap();
            if items.len() <= 1 {
                return Ok(Value::List(items));
            }
            let mut out = Vec::with_capacity(items.len() * 2 - 1);
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push(sep.clone());
                }
                out.push(item.clone());
            }
            Ok(list_value(out))
        }),
    );

    fields.insert(
        "chunk".to_string(),
        builtin("list.chunk", 2, |mut args, _| {
            let items = expect_list(args.pop().unwrap(), "List.chunk")?;
            let size = expect_int(args.pop().unwrap(), "List.chunk")?;
            if size <= 0 {
                return Ok(list_value(Vec::new()));
            }
            let size = usize::try_from(size).unwrap_or(0);
            if size == 0 {
                return Ok(list_value(Vec::new()));
            }
            let mut out = Vec::new();
            let mut buf = Vec::with_capacity(size);
            for item in items.iter().cloned() {
                buf.push(item);
                if buf.len() == size {
                    out.push(list_value(std::mem::take(&mut buf)));
                }
            }
            if !buf.is_empty() {
                out.push(list_value(buf));
            }
            Ok(list_value(out))
        }),
    );

    fields.insert(
        "dedup".to_string(),
        builtin("list.dedup", 1, |mut args, _| {
            let items = expect_list(args.pop().unwrap(), "List.dedup")?;
            let mut out: Vec<Value> = Vec::new();
            let mut last: Option<Value> = None;
            for item in items.iter().cloned() {
                if last.as_ref().is_some_and(|prev| values_equal(prev, &item)) {
                    continue;
                }
                last = Some(item.clone());
                out.push(item);
            }
            Ok(list_value(out))
        }),
    );

    fields.insert(
        "uniqueBy".to_string(),
        builtin("list.uniqueBy", 2, |mut args, runtime| {
            let items = expect_list(args.pop().unwrap(), "List.uniqueBy")?;
            let key_fn = expect_callable(args.pop().unwrap(), "List.uniqueBy")?;
            let mut seen: HashSet<KeyValue> = HashSet::new();
            let mut out = Vec::new();
            for item in items.iter().cloned() {
                let key_value = runtime.apply(key_fn.clone(), item.clone())?;
                let key = KeyValue::try_from_value(&key_value).ok_or_else(|| {
                    RuntimeError::Message("List.uniqueBy expects a hashable key".to_string())
                })?;
                if seen.insert(key) {
                    out.push(item);
                }
            }
            Ok(list_value(out))
        }),
    );

    Value::Record(Arc::new(fields))
}
