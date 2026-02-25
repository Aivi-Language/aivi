use std::sync::Arc;

use num_bigint::BigInt;
use num_rational::BigRational;
use regex::Regex;
use rust_decimal::Decimal;

use crate::runtime::values::{BuiltinImpl, BuiltinValue};
use crate::runtime::{Runtime, RuntimeError, Value};

/// Return a human-readable type name for a runtime value.
pub(super) fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Unit => "Unit",
        Value::Bool(_) => "Bool",
        Value::Int(_) => "Int",
        Value::Float(_) => "Float",
        Value::Text(_) => "Text",
        Value::DateTime(_) => "DateTime",
        Value::Bytes(_) => "Bytes",
        Value::Regex(_) => "Regex",
        Value::BigInt(_) => "BigInt",
        Value::Rational(_) => "Rational",
        Value::Decimal(_) => "Decimal",
        Value::Map(_) => "Map",
        Value::Set(_) => "Set",
        Value::Queue(_) => "Queue",
        Value::Deque(_) => "Deque",
        Value::Heap(_) => "Heap",
        Value::List(_) => "List",
        Value::Tuple(_) => "Tuple",
        Value::Record(_) => "Record",
        Value::Constructor { .. } => "Constructor",
        Value::Builtin(_) | Value::MultiClause(_) => "Function",
        Value::Effect(_) => "Effect",
        Value::Source(_) => "Source",
        Value::Resource(_) => "Resource",
        Value::ChannelSend(_) => "ChannelSend",
        Value::ChannelRecv(_) => "ChannelRecv",
        Value::FileHandle(_) => "FileHandle",
        Value::Listener(_) => "Listener",
        Value::Connection(_) => "Connection",
        Value::Stream(_) => "Stream",
        Value::HttpServer(_) => "HttpServer",
        Value::WebSocket(_) => "WebSocket",
    }
}

pub(crate) fn builtin(
    name: &str,
    arity: usize,
    func: impl Fn(Vec<Value>, &mut Runtime) -> Result<Value, RuntimeError> + Send + Sync + 'static,
) -> Value {
    Value::Builtin(BuiltinValue {
        imp: Arc::new(BuiltinImpl {
            name: name.to_string(),
            arity,
            func: Arc::new(func),
        }),
        args: Vec::new(),
        tagged_args: Some(Vec::new()),
    })
}

pub(super) fn builtin_constructor(name: &str, arity: usize) -> Value {
    let name_owned = name.to_string();
    builtin(name, arity, move |args, _| {
        Ok(Value::Constructor {
            name: name_owned.clone(),
            args,
        })
    })
}

pub(super) fn make_some(value: Value) -> Value {
    Value::Constructor {
        name: "Some".to_string(),
        args: vec![value],
    }
}

pub(super) fn make_none() -> Value {
    Value::Constructor {
        name: "None".to_string(),
        args: Vec::new(),
    }
}

pub(super) fn make_ok(value: Value) -> Value {
    Value::Constructor {
        name: "Ok".to_string(),
        args: vec![value],
    }
}

pub(super) fn make_err(value: Value) -> Value {
    Value::Constructor {
        name: "Err".to_string(),
        args: vec![value],
    }
}

pub(super) fn list_value(items: Vec<Value>) -> Value {
    Value::List(Arc::new(items))
}

pub(super) fn expect_text(value: Value, ctx: &str) -> Result<String, RuntimeError> {
    match value {
        Value::Text(text) => Ok(text),
        other => Err(RuntimeError::Message(format!(
            "{ctx}: expected Text, but received {}",
            value_type_name(&other)
        ))),
    }
}

pub(super) fn expect_int(value: Value, ctx: &str) -> Result<i64, RuntimeError> {
    match value {
        Value::Int(value) => Ok(value),
        other => Err(RuntimeError::Message(format!(
            "{ctx}: expected Int, but received {}",
            value_type_name(&other)
        ))),
    }
}

pub(super) fn expect_float(value: Value, ctx: &str) -> Result<f64, RuntimeError> {
    match value {
        Value::Float(value) => Ok(value),
        other => Err(RuntimeError::Message(format!(
            "{ctx}: expected Float, but received {}",
            value_type_name(&other)
        ))),
    }
}

pub(super) fn expect_char(value: Value, ctx: &str) -> Result<char, RuntimeError> {
    let text = expect_text(value, ctx)?;
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(ch), None) => Ok(ch),
        _ => Err(RuntimeError::Message(format!(
            "{ctx}: expected a single character, but received a string of length {}",
            text.len()
        ))),
    }
}

pub(super) fn expect_list(value: Value, ctx: &str) -> Result<Arc<Vec<Value>>, RuntimeError> {
    match value {
        Value::List(items) => Ok(items),
        other => Err(RuntimeError::Message(format!(
            "{ctx}: expected List, but received {}",
            value_type_name(&other)
        ))),
    }
}

pub(super) fn expect_record(
    value: Value,
    ctx: &str,
) -> Result<Arc<std::collections::HashMap<String, Value>>, RuntimeError> {
    match value {
        Value::Record(fields) => Ok(fields),
        other => Err(RuntimeError::Message(format!(
            "{ctx}: expected Record, but received {}",
            value_type_name(&other)
        ))),
    }
}

pub(super) fn list_floats(values: &[Value], ctx: &str) -> Result<Vec<f64>, RuntimeError> {
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        match value {
            Value::Float(value) => out.push(*value),
            other => {
                return Err(RuntimeError::Message(format!(
                    "{ctx}: expected List Float, but element is {}",
                    value_type_name(other)
                )))
            }
        }
    }
    Ok(out)
}

pub(super) fn list_ints(values: &[Value], ctx: &str) -> Result<Vec<i64>, RuntimeError> {
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        match value {
            Value::Int(value) => out.push(*value),
            other => {
                return Err(RuntimeError::Message(format!(
                    "{ctx}: expected List Int, but element is {}",
                    value_type_name(other)
                )))
            }
        }
    }
    Ok(out)
}

pub(super) fn expect_bytes(value: Value, ctx: &str) -> Result<Arc<Vec<u8>>, RuntimeError> {
    match value {
        Value::Bytes(bytes) => Ok(bytes),
        other => Err(RuntimeError::Message(format!(
            "{ctx}: expected Bytes, but received {}",
            value_type_name(&other)
        ))),
    }
}

pub(super) fn expect_regex(value: Value, ctx: &str) -> Result<Arc<Regex>, RuntimeError> {
    match value {
        Value::Regex(regex) => Ok(regex),
        other => Err(RuntimeError::Message(format!(
            "{ctx}: expected Regex, but received {}",
            value_type_name(&other)
        ))),
    }
}

pub(super) fn expect_bigint(value: Value, ctx: &str) -> Result<Arc<BigInt>, RuntimeError> {
    match value {
        Value::BigInt(value) => Ok(value),
        other => Err(RuntimeError::Message(format!(
            "{ctx}: expected BigInt, but received {}",
            value_type_name(&other)
        ))),
    }
}

pub(super) fn expect_rational(value: Value, ctx: &str) -> Result<Arc<BigRational>, RuntimeError> {
    match value {
        Value::Rational(value) => Ok(value),
        other => Err(RuntimeError::Message(format!(
            "{ctx}: expected Rational, but received {}",
            value_type_name(&other)
        ))),
    }
}

pub(super) fn expect_decimal(value: Value, ctx: &str) -> Result<Decimal, RuntimeError> {
    match value {
        Value::Decimal(value) => Ok(value),
        other => Err(RuntimeError::Message(format!(
            "{ctx}: expected Decimal, but received {}",
            value_type_name(&other)
        ))),
    }
}
