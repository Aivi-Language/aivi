use std::collections::HashMap;
use std::sync::Arc;

use super::util::{builtin, list_value};
use crate::runtime::{RuntimeError, Value};

/// Converts any AIVI runtime `Value` into the AIVI `JsonValue` ADT.
///
/// Mapping:
///   Text       → JsonString
///   Int        → JsonInt
///   Float      → JsonFloat
///   Bool       → JsonBool
///   List       → JsonArray
///   Record     → JsonObject (field order from BTreeMap, i.e. sorted by key)
///   Some v     → toJson v
///   None       → JsonNull
///   Unit       → JsonNull
///   Tuple      → JsonArray
///   other constructors → JsonString "<ConstructorName>"
pub(super) fn value_to_json_value(value: Value) -> Result<Value, RuntimeError> {
    match value {
        Value::Text(s) => Ok(con1("JsonString", Value::Text(s))),
        Value::Int(n) => Ok(con1("JsonInt", Value::Int(n))),
        Value::Float(f) => Ok(con1("JsonFloat", Value::Float(f))),
        Value::Bool(b) => Ok(con1("JsonBool", Value::Bool(b))),
        Value::Unit => Ok(con0("JsonNull")),
        Value::List(items) => {
            let encoded: Result<Vec<Value>, RuntimeError> =
                items.iter().cloned().map(value_to_json_value).collect();
            Ok(con1("JsonArray", list_value(encoded?)))
        }
        Value::Record(fields) => {
            let mut pairs: Vec<Value> = fields
                .iter()
                .map(|(k, v)| {
                    value_to_json_value(v.clone())
                        .map(|jv| Value::Tuple(vec![Value::Text(k.clone()), jv]))
                })
                .collect::<Result<_, _>>()?;
            // Sort by key for deterministic output (fields come from BTreeMap already sorted).
            pairs.sort_by(|a, b| {
                let ka = if let Value::Tuple(ref t) = a {
                    t[0].clone()
                } else {
                    Value::Text(String::new())
                };
                let kb = if let Value::Tuple(ref t) = b {
                    t[0].clone()
                } else {
                    Value::Text(String::new())
                };
                match (&ka, &kb) {
                    (Value::Text(sa), Value::Text(sb)) => sa.cmp(sb),
                    _ => std::cmp::Ordering::Equal,
                }
            });
            Ok(con1("JsonObject", list_value(pairs)))
        }
        Value::Tuple(items) => {
            let encoded: Result<Vec<Value>, RuntimeError> =
                items.into_iter().map(value_to_json_value).collect();
            Ok(con1("JsonArray", list_value(encoded?)))
        }
        Value::Constructor { ref name, ref args } if name == "None" && args.is_empty() => {
            Ok(con0("JsonNull"))
        }
        Value::Constructor { ref name, ref args } if name == "Some" && args.len() == 1 => {
            value_to_json_value(args[0].clone())
        }
        Value::Constructor { name, .. } => Ok(con1("JsonString", Value::Text(name))),
        other => Err(RuntimeError::Message(format!(
            "toJson: cannot encode value of kind {}",
            value_kind_name(&other)
        ))),
    }
}

fn value_kind_name(v: &Value) -> &'static str {
    match v {
        Value::Text(_) => "Text",
        Value::Int(_) => "Int",
        Value::Float(_) => "Float",
        Value::Bool(_) => "Bool",
        Value::Unit => "Unit",
        Value::List(_) => "List",
        Value::Record(_) => "Record",
        Value::Tuple(_) => "Tuple",
        Value::Constructor { .. } => "Constructor",
        _ => "unknown",
    }
}

fn con0(name: &str) -> Value {
    Value::Constructor {
        name: name.to_string(),
        args: vec![],
    }
}

fn con1(name: &str, arg: Value) -> Value {
    Value::Constructor {
        name: name.to_string(),
        args: vec![arg],
    }
}

/// Converts an AIVI `JsonValue` ADT value back to a JSON text string.
/// Used by the HTTP runtime to serialize a `Json` body variant.
pub(super) fn json_value_to_text(value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Constructor { name, args } if name == "JsonNull" && args.is_empty() => {
            Ok("null".to_string())
        }
        Value::Constructor { name, args } if name == "JsonBool" && args.len() == 1 => {
            match &args[0] {
                Value::Bool(true) => Ok("true".to_string()),
                Value::Bool(false) => Ok("false".to_string()),
                _ => Err(RuntimeError::Message(
                    "json: JsonBool expects Bool".to_string(),
                )),
            }
        }
        Value::Constructor { name, args } if name == "JsonInt" && args.len() == 1 => {
            match &args[0] {
                Value::Int(n) => Ok(n.to_string()),
                _ => Err(RuntimeError::Message(
                    "json: JsonInt expects Int".to_string(),
                )),
            }
        }
        Value::Constructor { name, args } if name == "JsonFloat" && args.len() == 1 => {
            match &args[0] {
                Value::Float(f) => Ok(format_float(*f)),
                _ => Err(RuntimeError::Message(
                    "json: JsonFloat expects Float".to_string(),
                )),
            }
        }
        Value::Constructor { name, args } if name == "JsonString" && args.len() == 1 => {
            match &args[0] {
                Value::Text(s) => Ok(format!("\"{}\"", json_escape(s))),
                _ => Err(RuntimeError::Message(
                    "json: JsonString expects Text".to_string(),
                )),
            }
        }
        Value::Constructor { name, args } if name == "JsonArray" && args.len() == 1 => {
            let items = match &args[0] {
                Value::List(list) => list.clone(),
                _ => {
                    return Err(RuntimeError::Message(
                        "json: JsonArray expects List".to_string(),
                    ))
                }
            };
            let parts: Result<Vec<String>, RuntimeError> =
                items.iter().map(json_value_to_text).collect();
            Ok(format!("[{}]", parts?.join(",")))
        }
        Value::Constructor { name, args } if name == "JsonObject" && args.len() == 1 => {
            let pairs = match &args[0] {
                Value::List(list) => list.clone(),
                _ => {
                    return Err(RuntimeError::Message(
                        "json: JsonObject expects List".to_string(),
                    ))
                }
            };
            let parts: Result<Vec<String>, RuntimeError> = pairs
                .iter()
                .map(|pair| match pair {
                    Value::Tuple(kv) if kv.len() == 2 => {
                        let key = match &kv[0] {
                            Value::Text(s) => s.clone(),
                            _ => {
                                return Err(RuntimeError::Message(
                                    "json: JsonObject key must be Text".to_string(),
                                ))
                            }
                        };
                        let val = json_value_to_text(&kv[1])?;
                        Ok(format!("\"{}\":{}", json_escape(&key), val))
                    }
                    _ => Err(RuntimeError::Message(
                        "json: JsonObject entry must be (Text, JsonValue) tuple".to_string(),
                    )),
                })
                .collect();
            Ok(format!("{{{}}}", parts?.join(",")))
        }
        _ => Err(RuntimeError::Message(format!(
            "json: expected JsonValue, got unexpected constructor"
        ))),
    }
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

fn format_float(f: f64) -> String {
    if f.fract() == 0.0 && f.is_finite() {
        format!("{f}.0")
    } else {
        format!("{f}")
    }
}

pub(super) fn build_json_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "toJson".to_string(),
        builtin("json.toJson", 1, |mut args, _| {
            value_to_json_value(args.pop().unwrap())
        }),
    );
    Value::Record(Arc::new(fields))
}
