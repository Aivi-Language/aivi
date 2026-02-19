use std::sync::{Arc, Mutex};

use im::HashMap as ImHashMap;

use super::util::{builtin, list_value, make_none, make_some};
use crate::runtime::values::KeyValue;
use crate::runtime::{EffectValue, RuntimeError, Value};

fn expect_mutable_map(
    value: Value,
    ctx: &str,
) -> Result<Arc<Mutex<ImHashMap<KeyValue, Value>>>, RuntimeError> {
    match value {
        Value::MutableMap(m) => Ok(m),
        other => Err(RuntimeError::Message(format!(
            "{ctx} expects MutableMap, got {}",
            super::super::format_value(&other)
        ))),
    }
}

fn key_from_value(value: &Value, ctx: &str) -> Result<KeyValue, RuntimeError> {
    KeyValue::try_from_value(value).ok_or_else(|| {
        RuntimeError::Message(format!(
            "{ctx}: value is not a valid map key: {}",
            super::super::format_value(value)
        ))
    })
}

pub(super) fn build_mutable_map_record() -> Value {
    let mut fields = std::collections::HashMap::new();

    // MutableMap.create : Map k v -> Effect e (MutableMap k v)
    fields.insert(
        "create".to_string(),
        builtin("mutableMap.create", 1, |mut args, _| {
            let initial = match args.pop().unwrap() {
                Value::Map(m) => (*m).clone(),
                other => {
                    return Err(RuntimeError::Message(format!(
                        "mutableMap.create expects Map, got {}",
                        super::super::format_value(&other)
                    )));
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    Ok(Value::MutableMap(Arc::new(Mutex::new(initial.clone()))))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    // MutableMap.empty : Effect e (MutableMap k v)
    fields.insert(
        "empty".to_string(),
        builtin("mutableMap.empty", 1, |_, _| {
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    Ok(Value::MutableMap(Arc::new(Mutex::new(ImHashMap::new()))))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    // MutableMap.get : k -> MutableMap k v -> Effect e (Option v)
    fields.insert(
        "get".to_string(),
        builtin("mutableMap.get", 2, |mut args, _| {
            let mmap = expect_mutable_map(args.pop().unwrap(), "mutableMap.get")?;
            let key = key_from_value(&args.pop().unwrap(), "mutableMap.get")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let map = mmap.lock().expect("mutable map lock");
                    Ok(match map.get(&key) {
                        Some(value) => make_some(value.clone()),
                        None => make_none(),
                    })
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    // MutableMap.getOrElse : k -> v -> MutableMap k v -> Effect e v
    fields.insert(
        "getOrElse".to_string(),
        builtin("mutableMap.getOrElse", 3, |mut args, _| {
            let mmap = expect_mutable_map(args.pop().unwrap(), "mutableMap.getOrElse")?;
            let default = args.pop().unwrap();
            let key = key_from_value(&args.pop().unwrap(), "mutableMap.getOrElse")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let map = mmap.lock().expect("mutable map lock");
                    Ok(map.get(&key).cloned().unwrap_or_else(|| default.clone()))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    // MutableMap.insert : k -> v -> MutableMap k v -> Effect e Unit
    fields.insert(
        "insert".to_string(),
        builtin("mutableMap.insert", 3, |mut args, _| {
            let mmap = expect_mutable_map(args.pop().unwrap(), "mutableMap.insert")?;
            let value = args.pop().unwrap();
            let key = key_from_value(&args.pop().unwrap(), "mutableMap.insert")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut map = mmap.lock().expect("mutable map lock");
                    map.insert(key.clone(), value.clone());
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    // MutableMap.remove : k -> MutableMap k v -> Effect e Unit
    fields.insert(
        "remove".to_string(),
        builtin("mutableMap.remove", 2, |mut args, _| {
            let mmap = expect_mutable_map(args.pop().unwrap(), "mutableMap.remove")?;
            let key = key_from_value(&args.pop().unwrap(), "mutableMap.remove")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut map = mmap.lock().expect("mutable map lock");
                    map.remove(&key);
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    // MutableMap.has : k -> MutableMap k v -> Effect e Bool
    fields.insert(
        "has".to_string(),
        builtin("mutableMap.has", 2, |mut args, _| {
            let mmap = expect_mutable_map(args.pop().unwrap(), "mutableMap.has")?;
            let key = key_from_value(&args.pop().unwrap(), "mutableMap.has")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let map = mmap.lock().expect("mutable map lock");
                    Ok(Value::Bool(map.contains_key(&key)))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    // MutableMap.size : MutableMap k v -> Effect e Int
    fields.insert(
        "size".to_string(),
        builtin("mutableMap.size", 1, |mut args, _| {
            let mmap = expect_mutable_map(args.pop().unwrap(), "mutableMap.size")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let map = mmap.lock().expect("mutable map lock");
                    Ok(Value::Int(map.len() as i64))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    // MutableMap.freeze : MutableMap k v -> Effect e (Map k v)
    fields.insert(
        "freeze".to_string(),
        builtin("mutableMap.freeze", 1, |mut args, _| {
            let mmap = expect_mutable_map(args.pop().unwrap(), "mutableMap.freeze")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let map = mmap.lock().expect("mutable map lock");
                    Ok(Value::Map(Arc::new(map.clone())))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    // MutableMap.keys : MutableMap k v -> Effect e (List k)
    fields.insert(
        "keys".to_string(),
        builtin("mutableMap.keys", 1, |mut args, _| {
            let mmap = expect_mutable_map(args.pop().unwrap(), "mutableMap.keys")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let map = mmap.lock().expect("mutable map lock");
                    let items = map.iter().map(|(key, _)| key.to_value()).collect();
                    Ok(list_value(items))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    // MutableMap.values : MutableMap k v -> Effect e (List v)
    fields.insert(
        "values".to_string(),
        builtin("mutableMap.values", 1, |mut args, _| {
            let mmap = expect_mutable_map(args.pop().unwrap(), "mutableMap.values")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let map = mmap.lock().expect("mutable map lock");
                    let items = map.iter().map(|(_, value)| value.clone()).collect();
                    Ok(list_value(items))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    // MutableMap.modify : (Map k v -> Map k v) -> MutableMap k v -> Effect e Unit
    fields.insert(
        "modify".to_string(),
        builtin("mutableMap.modify", 2, |mut args, _| {
            let mmap = expect_mutable_map(args.pop().unwrap(), "mutableMap.modify")?;
            let func = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let snapshot = {
                        let map = mmap.lock().expect("mutable map lock");
                        Value::Map(Arc::new(map.clone()))
                    };
                    let result = runtime.apply(func.clone(), snapshot)?;
                    match result {
                        Value::Map(new_map) => {
                            let mut map = mmap.lock().expect("mutable map lock");
                            *map = (*new_map).clone();
                            Ok(Value::Unit)
                        }
                        other => Err(RuntimeError::Message(format!(
                            "mutableMap.modify: function must return Map, got {}",
                            super::super::format_value(&other)
                        ))),
                    }
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    Value::Record(Arc::new(fields))
}
