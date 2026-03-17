use std::collections::HashMap;
use std::sync::Arc;

use aivi_database::{DatabaseState, Driver};
use serde_json::Value as JsonValue;

use super::util::{
    builtin, builtin_constructor, expect_list, expect_record, expect_text, list_value,
};
use crate::runtime::{EffectValue, Runtime, RuntimeError, Value};

fn table_parts(value: Value, ctx: &str) -> Result<(String, Value, Arc<Vec<Value>>), RuntimeError> {
    let fields = expect_record(value, ctx)?;
    let name = fields
        .get("name")
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Table.name")))?;
    let name = expect_text(name.clone(), ctx)?;
    let columns = fields
        .get("columns")
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Table.columns")))?;
    let rows = fields
        .get("rows")
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Table.rows")))?;
    let rows = expect_list(rows.clone(), ctx)?;
    Ok((name, columns.clone(), rows))
}

fn make_table(name: String, columns: Value, rows: Vec<Value>) -> Value {
    let mut fields = HashMap::new();
    fields.insert("name".to_string(), Value::Text(name));
    fields.insert("columns".to_string(), columns);
    fields.insert("rows".to_string(), list_value(rows));
    Value::Record(Arc::new(fields))
}

fn encode_value(value: &Value) -> Result<JsonValue, RuntimeError> {
    Ok(match value {
        Value::Unit => serde_json::json!({ "t": "Unit" }),
        Value::Bool(v) => serde_json::json!({ "t": "Bool", "v": v }),
        Value::Int(v) => serde_json::json!({ "t": "Int", "v": v }),
        Value::Float(v) => serde_json::json!({ "t": "Float", "v": v }),
        Value::Text(v) => serde_json::json!({ "t": "Text", "v": v }),
        Value::DateTime(v) => serde_json::json!({ "t": "DateTime", "v": v }),
        Value::BigInt(v) => serde_json::json!({ "t": "BigInt", "v": v.to_string() }),
        Value::Rational(v) => serde_json::json!({ "t": "Rational", "v": v.to_string() }),
        Value::Decimal(v) => serde_json::json!({ "t": "Decimal", "v": v.to_string() }),
        Value::Bytes(bytes) => {
            let arr: Vec<JsonValue> = bytes.iter().copied().map(JsonValue::from).collect();
            serde_json::json!({ "t": "Bytes", "v": arr })
        }
        Value::List(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items.iter() {
                out.push(encode_value(item)?);
            }
            serde_json::json!({ "t": "List", "v": out })
        }
        Value::Tuple(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items.iter() {
                out.push(encode_value(item)?);
            }
            serde_json::json!({ "t": "Tuple", "v": out })
        }
        Value::Record(fields) => {
            let mut map = serde_json::Map::new();
            for (k, v) in fields.iter() {
                map.insert(k.clone(), encode_value(v)?);
            }
            serde_json::json!({ "t": "Record", "v": JsonValue::Object(map) })
        }
        Value::Constructor { name, args } => {
            let mut out = Vec::with_capacity(args.len());
            for arg in args.iter() {
                out.push(encode_value(arg)?);
            }
            serde_json::json!({ "t": "Constructor", "name": name, "args": out })
        }
        other => {
            return Err(RuntimeError::Message(format!(
                "database: cannot persist value {}",
                crate::runtime::format_value(other)
            )))
        }
    })
}

fn decode_value(value: &JsonValue) -> Result<Value, RuntimeError> {
    let obj = value.as_object().ok_or_else(|| {
        RuntimeError::Message("database: invalid persisted value (expected object)".to_string())
    })?;
    let tag = obj.get("t").and_then(|v| v.as_str()).ok_or_else(|| {
        RuntimeError::Message("database: missing persisted value tag".to_string())
    })?;
    match tag {
        "Unit" => Ok(Value::Unit),
        "Bool" => Ok(Value::Bool(
            obj.get("v")
                .and_then(|v| v.as_bool())
                .ok_or_else(|| RuntimeError::Message("database: invalid Bool".to_string()))?,
        )),
        "Int" => Ok(Value::Int(
            obj.get("v")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| RuntimeError::Message("database: invalid Int".to_string()))?,
        )),
        "Float" => Ok(Value::Float(
            obj.get("v")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| RuntimeError::Message("database: invalid Float".to_string()))?,
        )),
        "Text" => Ok(Value::Text(
            obj.get("v")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RuntimeError::Message("database: invalid Text".to_string()))?
                .to_string(),
        )),
        "DateTime" => Ok(Value::DateTime(
            obj.get("v")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RuntimeError::Message("database: invalid DateTime".to_string()))?
                .to_string(),
        )),
        "BigInt" => {
            let s = obj
                .get("v")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RuntimeError::Message("database: invalid BigInt".to_string()))?;
            let parsed = s
                .parse::<num_bigint::BigInt>()
                .map_err(|_| RuntimeError::Message("database: invalid BigInt".to_string()))?;
            Ok(Value::BigInt(Arc::new(parsed)))
        }
        "Rational" => {
            let s = obj
                .get("v")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RuntimeError::Message("database: invalid Rational".to_string()))?;
            let parsed = s
                .parse::<num_rational::BigRational>()
                .map_err(|_| RuntimeError::Message("database: invalid Rational".to_string()))?;
            Ok(Value::Rational(Arc::new(parsed)))
        }
        "Decimal" => {
            let s = obj
                .get("v")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RuntimeError::Message("database: invalid Decimal".to_string()))?;
            let parsed = s
                .parse::<rust_decimal::Decimal>()
                .map_err(|_| RuntimeError::Message("database: invalid Decimal".to_string()))?;
            Ok(Value::Decimal(parsed))
        }
        "Bytes" => {
            let arr = obj
                .get("v")
                .and_then(|v| v.as_array())
                .ok_or_else(|| RuntimeError::Message("database: invalid Bytes".to_string()))?;
            let mut out = Vec::with_capacity(arr.len());
            for item in arr.iter() {
                let b = item
                    .as_u64()
                    .and_then(|b| u8::try_from(b).ok())
                    .ok_or_else(|| RuntimeError::Message("database: invalid Bytes".to_string()))?;
                out.push(b);
            }
            Ok(Value::Bytes(Arc::new(out)))
        }
        "List" => {
            let arr = obj
                .get("v")
                .and_then(|v| v.as_array())
                .ok_or_else(|| RuntimeError::Message("database: invalid List".to_string()))?;
            let mut out = Vec::with_capacity(arr.len());
            for item in arr.iter() {
                out.push(decode_value(item)?);
            }
            Ok(list_value(out))
        }
        "Tuple" => {
            let arr = obj
                .get("v")
                .and_then(|v| v.as_array())
                .ok_or_else(|| RuntimeError::Message("database: invalid Tuple".to_string()))?;
            let mut out = Vec::with_capacity(arr.len());
            for item in arr.iter() {
                out.push(decode_value(item)?);
            }
            Ok(Value::Tuple(out))
        }
        "Record" => {
            let map = obj
                .get("v")
                .and_then(|v| v.as_object())
                .ok_or_else(|| RuntimeError::Message("database: invalid Record".to_string()))?;
            let mut out = HashMap::new();
            for (k, v) in map.iter() {
                out.insert(k.clone(), decode_value(v)?);
            }
            Ok(Value::Record(Arc::new(out)))
        }
        "Constructor" => {
            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RuntimeError::Message("database: invalid Constructor".to_string()))?
                .to_string();
            let args_arr = obj.get("args").and_then(|v| v.as_array()).ok_or_else(|| {
                RuntimeError::Message("database: invalid Constructor".to_string())
            })?;
            let mut args = Vec::with_capacity(args_arr.len());
            for item in args_arr.iter() {
                args.push(decode_value(item)?);
            }
            Ok(Value::Constructor { name, args })
        }
        _ => Err(RuntimeError::Message(format!(
            "database: unknown persisted value tag {tag}"
        ))),
    }
}

pub(super) fn encode_json(value: &Value) -> Result<String, RuntimeError> {
    let json = encode_value(value)?;
    serde_json::to_string(&json)
        .map_err(|e| RuntimeError::Message(format!("database: json encode error: {e}")))
}

pub(super) fn decode_json(text: &str) -> Result<Value, RuntimeError> {
    let json: JsonValue = serde_json::from_str(text)
        .map_err(|e| RuntimeError::Message(format!("database: json decode error: {e}")))?;
    decode_value(&json)
}
