use std::collections::HashMap;
use std::sync::Arc;

use num_bigint::BigInt;
use num_rational::BigRational;
use regex::Regex;
use rust_decimal::Decimal;

use crate::runtime::values::{BuiltinImpl, BuiltinValue, DbPatchRuntimeMeta};
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
        Value::Thunk(_) => "Thunk",
        Value::Signal(_) => "Signal",
        Value::ChannelSend(_) => "ChannelSend",
        Value::ChannelRecv(_) => "ChannelRecv",
        Value::FileHandle(_) => "FileHandle",
        Value::Listener(_) => "Listener",
        Value::Connection(_) => "Connection",
        Value::Stream(_) => "Stream",
        Value::HttpServer(_) => "HttpServer",
        Value::WebSocket(_) => "WebSocket",
        Value::ImapSession(_) => "ImapSession",
        Value::DbConnection(_) => "DbConnection",
    }
}

pub(crate) fn builtin(
    name: &str,
    arity: usize,
    func: impl Fn(Vec<Value>, &mut Runtime) -> Result<Value, RuntimeError> + Send + Sync + 'static,
) -> Value {
    builtin_with_db_patch_meta(name, arity, None, func)
}

pub(crate) fn builtin_with_db_patch_meta(
    name: &str,
    arity: usize,
    db_patch_meta: Option<Arc<DbPatchRuntimeMeta>>,
    func: impl Fn(Vec<Value>, &mut Runtime) -> Result<Value, RuntimeError> + Send + Sync + 'static,
) -> Value {
    Value::Builtin(BuiltinValue {
        imp: Arc::new(BuiltinImpl {
            name: name.to_string(),
            arity,
            func: Arc::new(func),
            db_patch_meta,
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

pub(super) fn make_source_io_error(message: impl Into<String>) -> Value {
    Value::Constructor {
        name: "IOError".to_string(),
        args: vec![Value::Text(message.into())],
    }
}

pub(super) fn make_source_decode_error(errors: Vec<Value>) -> Value {
    Value::Constructor {
        name: "DecodeError".to_string(),
        args: vec![list_value(errors)],
    }
}

pub(super) fn make_decode_error(path: Vec<String>, message: impl Into<String>) -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "path".to_string(),
        Value::List(Arc::new(
            path.into_iter().map(Value::Text).collect::<Vec<_>>(),
        )),
    );
    fields.insert("message".to_string(), Value::Text(message.into()));
    Value::Record(Arc::new(fields))
}

pub(super) fn json_path_segments(path: &str) -> Vec<String> {
    let chars: Vec<char> = path.chars().collect();
    let mut index = 0usize;
    let mut segments = Vec::new();
    if chars.first() == Some(&'$') {
        index += 1;
    }
    while index < chars.len() {
        match chars[index] {
            '.' => {
                index += 1;
                let start = index;
                while index < chars.len() && chars[index] != '.' && chars[index] != '[' {
                    index += 1;
                }
                if start < index {
                    segments.push(chars[start..index].iter().collect());
                }
            }
            '[' => {
                index += 1;
                let start = index;
                while index < chars.len() && chars[index] != ']' {
                    index += 1;
                }
                if start < index {
                    segments.push(chars[start..index].iter().collect());
                }
                if index < chars.len() && chars[index] == ']' {
                    index += 1;
                }
            }
            _ => {
                index += 1;
            }
        }
    }
    segments
}

pub(super) fn json_mismatch_to_decode_error(
    mismatch: &crate::runtime::json_schema::JsonMismatch,
) -> Value {
    make_decode_error(
        json_path_segments(&mismatch.path),
        format!("expected {}, got {}", mismatch.expected, mismatch.got),
    )
}

pub(super) fn decode_error_list_from_value(
    value: &Value,
    ctx: &str,
) -> Result<Vec<Value>, RuntimeError> {
    let Value::List(items) = value else {
        return Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "List".to_string(),
            got: value_type_name(value).to_string(),
        });
    };
    for item in items.iter() {
        let Value::Record(fields) = item else {
            return Err(RuntimeError::TypeError {
                context: ctx.to_string(),
                expected: "DecodeError".to_string(),
                got: value_type_name(item).to_string(),
            });
        };
        match fields.get("path") {
            Some(Value::List(path_items)) => {
                for segment in path_items.iter() {
                    if !matches!(segment, Value::Text(_)) {
                        return Err(RuntimeError::Message(format!(
                            "{ctx} expects DecodeError.path to be List Text"
                        )));
                    }
                }
            }
            Some(other) => {
                return Err(RuntimeError::TypeError {
                    context: ctx.to_string(),
                    expected: "List".to_string(),
                    got: value_type_name(other).to_string(),
                });
            }
            None => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} expects DecodeError.path"
                )));
            }
        }
        match fields.get("message") {
            Some(Value::Text(_)) => {}
            Some(other) => {
                return Err(RuntimeError::TypeError {
                    context: ctx.to_string(),
                    expected: "Text".to_string(),
                    got: value_type_name(other).to_string(),
                });
            }
            None => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} expects DecodeError.message"
                )));
            }
        }
    }
    Ok(items.iter().cloned().collect())
}

pub(super) fn list_value(items: Vec<Value>) -> Value {
    Value::List(Arc::new(items))
}

pub(super) fn expect_text(value: Value, ctx: &str) -> Result<String, RuntimeError> {
    match value {
        Value::Text(text) => Ok(text),
        other => Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "Text".to_string(),
            got: value_type_name(&other).to_string(),
        }),
    }
}

pub(super) fn expect_int(value: Value, ctx: &str) -> Result<i64, RuntimeError> {
    match value {
        Value::Int(value) => Ok(value),
        other => Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "Int".to_string(),
            got: value_type_name(&other).to_string(),
        }),
    }
}

pub(super) fn expect_float(value: Value, ctx: &str) -> Result<f64, RuntimeError> {
    match value {
        Value::Float(value) => Ok(value),
        other => Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "Float".to_string(),
            got: value_type_name(&other).to_string(),
        }),
    }
}

pub(super) fn expect_char(value: Value, ctx: &str) -> Result<char, RuntimeError> {
    let text = expect_text(value, ctx)?;
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(ch), None) => Ok(ch),
        _ => Err(RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: format!(
                "expected a single character, got string of length {}",
                text.len()
            ),
        }),
    }
}

pub(super) fn expect_list(value: Value, ctx: &str) -> Result<Arc<Vec<Value>>, RuntimeError> {
    match value {
        Value::List(items) => Ok(items),
        other => Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "List".to_string(),
            got: value_type_name(&other).to_string(),
        }),
    }
}

pub(super) fn expect_record(
    value: Value,
    ctx: &str,
) -> Result<Arc<std::collections::HashMap<String, Value>>, RuntimeError> {
    match value {
        Value::Record(fields) => Ok(fields),
        other => Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "Record".to_string(),
            got: value_type_name(&other).to_string(),
        }),
    }
}

pub(super) fn list_floats(values: &[Value], ctx: &str) -> Result<Vec<f64>, RuntimeError> {
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        match value {
            Value::Float(value) => out.push(*value),
            other => {
                return Err(RuntimeError::TypeError {
                    context: ctx.to_string(),
                    expected: "Float".to_string(),
                    got: value_type_name(other).to_string(),
                })
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
                return Err(RuntimeError::TypeError {
                    context: ctx.to_string(),
                    expected: "Int".to_string(),
                    got: value_type_name(other).to_string(),
                })
            }
        }
    }
    Ok(out)
}

pub(super) fn expect_bytes(value: Value, ctx: &str) -> Result<Arc<Vec<u8>>, RuntimeError> {
    match value {
        Value::Bytes(bytes) => Ok(bytes),
        other => Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "Bytes".to_string(),
            got: value_type_name(&other).to_string(),
        }),
    }
}

pub(super) fn expect_regex(value: Value, ctx: &str) -> Result<Arc<Regex>, RuntimeError> {
    match value {
        Value::Regex(regex) => Ok(regex),
        other => Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "Regex".to_string(),
            got: value_type_name(&other).to_string(),
        }),
    }
}

pub(super) fn expect_bigint(value: Value, ctx: &str) -> Result<Arc<BigInt>, RuntimeError> {
    match value {
        Value::BigInt(value) => Ok(value),
        other => Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "BigInt".to_string(),
            got: value_type_name(&other).to_string(),
        }),
    }
}

pub(super) fn expect_rational(value: Value, ctx: &str) -> Result<Arc<BigRational>, RuntimeError> {
    match value {
        Value::Rational(value) => Ok(value),
        other => Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "Rational".to_string(),
            got: value_type_name(&other).to_string(),
        }),
    }
}

pub(super) fn expect_decimal(value: Value, ctx: &str) -> Result<Decimal, RuntimeError> {
    match value {
        Value::Decimal(value) => Ok(value),
        other => Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "Decimal".to_string(),
            got: value_type_name(&other).to_string(),
        }),
    }
}
