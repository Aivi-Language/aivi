use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value as JsonValue;

use super::values::Value;
use super::RuntimeError;

/// Convert a runtime Value to a human-readable JSON representation.
pub(crate) fn value_to_snapshot_json(value: &Value) -> Result<JsonValue, RuntimeError> {
    Ok(match value {
        Value::Unit => JsonValue::Null,
        Value::Bool(v) => JsonValue::Bool(*v),
        Value::Int(v) => serde_json::json!(*v),
        Value::Float(v) => serde_json::json!(*v),
        Value::Text(v) => JsonValue::String(v.clone()),
        Value::DateTime(v) => JsonValue::String(v.clone()),
        Value::BigInt(v) => JsonValue::String(v.to_string()),
        Value::Rational(v) => JsonValue::String(v.to_string()),
        Value::Decimal(v) => JsonValue::String(v.to_string()),
        Value::Bytes(bytes) => {
            let arr: Vec<JsonValue> = bytes.iter().copied().map(JsonValue::from).collect();
            JsonValue::Array(arr)
        }
        Value::List(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items.iter() {
                out.push(value_to_snapshot_json(item)?);
            }
            JsonValue::Array(out)
        }
        Value::Tuple(items) => {
            let mut map = serde_json::Map::new();
            let mut arr = Vec::with_capacity(items.len());
            for item in items.iter() {
                arr.push(value_to_snapshot_json(item)?);
            }
            map.insert("$tuple".to_string(), JsonValue::Array(arr));
            JsonValue::Object(map)
        }
        Value::Record(fields) => {
            let mut map = serde_json::Map::new();
            for (k, v) in fields.iter() {
                map.insert(k.clone(), value_to_snapshot_json(v)?);
            }
            JsonValue::Object(map)
        }
        Value::Constructor { name, args } => {
            let mut map = serde_json::Map::new();
            map.insert("$ctor".to_string(), JsonValue::String(name.clone()));
            if !args.is_empty() {
                let mut arr = Vec::with_capacity(args.len());
                for arg in args.iter() {
                    arr.push(value_to_snapshot_json(arg)?);
                }
                map.insert("$args".to_string(), JsonValue::Array(arr));
            }
            JsonValue::Object(map)
        }
        Value::Map(entries) => {
            let mut arr = Vec::with_capacity(entries.len());
            for (k, v) in entries.iter() {
                arr.push(serde_json::json!({
                    "key": value_to_snapshot_json(&k.to_value())?,
                    "value": value_to_snapshot_json(v)?
                }));
            }
            serde_json::json!({ "$map": arr })
        }
        Value::Set(entries) => {
            let mut arr = Vec::with_capacity(entries.len());
            for item in entries.iter() {
                arr.push(value_to_snapshot_json(&item.to_value())?);
            }
            serde_json::json!({ "$set": arr })
        }
        other => {
            return Err(RuntimeError::Message(format!(
                "assertSnapshot: cannot serialize {}",
                super::format_value(other)
            )));
        }
    })
}

/// Convert a JSON value back into a runtime Value for mock snapshot replay.
pub(crate) fn snapshot_json_to_value(json: &JsonValue) -> Result<Value, RuntimeError> {
    Ok(match json {
        JsonValue::Null => Value::Unit,
        JsonValue::Bool(v) => Value::Bool(*v),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                return Err(RuntimeError::Message(format!(
                    "snapshot: unsupported number: {n}"
                )));
            }
        }
        JsonValue::String(s) => Value::Text(s.clone()),
        JsonValue::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(snapshot_json_to_value(item)?);
            }
            Value::List(out.into())
        }
        JsonValue::Object(map) => {
            if let Some(ctor_name) = map.get("$ctor").and_then(|v| v.as_str()) {
                let args = if let Some(JsonValue::Array(arr)) = map.get("$args") {
                    let mut out = Vec::with_capacity(arr.len());
                    for item in arr {
                        out.push(snapshot_json_to_value(item)?);
                    }
                    out
                } else {
                    Vec::new()
                };
                Value::Constructor {
                    name: ctor_name.to_string(),
                    args,
                }
            } else if let Some(JsonValue::Array(arr)) = map.get("$tuple") {
                let mut out = Vec::with_capacity(arr.len());
                for item in arr {
                    out.push(snapshot_json_to_value(item)?);
                }
                Value::Tuple(out)
            } else {
                let mut fields = HashMap::with_capacity(map.len());
                for (k, v) in map {
                    fields.insert(k.clone(), snapshot_json_to_value(v)?);
                }
                Value::Record(Arc::new(fields))
            }
        }
    })
}

/// Compute the snapshot directory path for a given test.
/// Layout: `<root>/__snapshots__/<module.path>/<test_name>/`
pub(crate) fn snapshot_dir(root: &Path, test_name: &str) -> PathBuf {
    let parts: Vec<&str> = test_name.rsplitn(2, '.').collect();
    let (module, test) = if parts.len() == 2 {
        (parts[1], parts[0])
    } else {
        ("_", test_name)
    };
    root.join("__snapshots__").join(module).join(test)
}

/// Compute the full path to a specific snapshot file.
pub(crate) fn snapshot_file(root: &Path, test_name: &str, snap_name: &str) -> PathBuf {
    snapshot_dir(root, test_name).join(format!("{snap_name}.snap"))
}
