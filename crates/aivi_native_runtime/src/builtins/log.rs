use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value as JsonValue;

use super::util::{builtin, expect_text};
use crate::runtime::{format_value, EffectValue, RuntimeError, Value};

fn level_name(value: Value, ctx: &str) -> Result<String, RuntimeError> {
    match value {
        Value::Constructor { name, args } if args.is_empty() => Ok(name),
        Value::Text(text) => Ok(text),
        _ => Err(RuntimeError::Message(format!("{ctx} expects Level"))),
    }
}

fn context_map(value: Value, ctx: &str) -> Result<HashMap<String, String>, RuntimeError> {
    match value {
        Value::Record(fields) => Ok(fields
            .iter()
            .map(|(key, value)| (key.clone(), format_value(value)))
            .collect()),
        Value::List(items) => {
            let mut out = HashMap::new();
            for item in items.iter() {
                match item {
                    Value::Tuple(values) if values.len() == 2 => {
                        let key = match &values[0] {
                            Value::Text(text) => text.clone(),
                            _ => {
                                return Err(RuntimeError::Message(format!(
                                    "{ctx} expects List (Text, Text)"
                                )))
                            }
                        };
                        let val = format_value(&values[1]);
                        out.insert(key, val);
                    }
                    _ => {
                        return Err(RuntimeError::Message(format!(
                            "{ctx} expects List (Text, Text)"
                        )))
                    }
                }
            }
            Ok(out)
        }
        other => Err(RuntimeError::Message(format!(
            "{ctx} expects Context, got {}",
            format_value(&other)
        ))),
    }
}

fn emit_log(level: String, message: String, context: HashMap<String, String>) -> Value {
    let effect = EffectValue::Thunk {
        func: Arc::new(move |_| {
            let ctx_json: serde_json::Map<String, JsonValue> = context
                .iter()
                .map(|(key, value)| (key.clone(), JsonValue::String(value.clone())))
                .collect();
            let payload = JsonValue::Object(
                [
                    ("level".to_string(), JsonValue::String(level.clone())),
                    ("message".to_string(), JsonValue::String(message.clone())),
                    ("context".to_string(), JsonValue::Object(ctx_json)),
                ]
                .into_iter()
                .collect(),
            );
            let line = serde_json::to_string(&payload)
                .map_err(|err| RuntimeError::Message(format!("log serialization failed: {err}")))?;
            match level.as_str() {
                "warn" | "error" => eprintln!("{line}"),
                _ => println!("{line}"),
            }
            Ok(Value::Unit)
        }),
    };
    Value::Effect(Arc::new(effect))
}

pub(super) fn build_log_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "log".to_string(),
        builtin("log.log", 3, |mut args, _| {
            let context = context_map(args.pop().unwrap(), "log.log")?;
            let message = expect_text(args.pop().unwrap(), "log.log")?;
            let level = level_name(args.pop().unwrap(), "log.log")?.to_lowercase();
            Ok(emit_log(level, message, context))
        }),
    );
    fields.insert(
        "trace".to_string(),
        builtin("log.trace", 2, |mut args, _| {
            let context = context_map(args.pop().unwrap(), "log.trace")?;
            let message = expect_text(args.pop().unwrap(), "log.trace")?;
            Ok(emit_log("trace".to_string(), message, context))
        }),
    );
    fields.insert(
        "debug".to_string(),
        builtin("log.debug", 2, |mut args, _| {
            let context = context_map(args.pop().unwrap(), "log.debug")?;
            let message = expect_text(args.pop().unwrap(), "log.debug")?;
            Ok(emit_log("debug".to_string(), message, context))
        }),
    );
    fields.insert(
        "info".to_string(),
        builtin("log.info", 2, |mut args, _| {
            let context = context_map(args.pop().unwrap(), "log.info")?;
            let message = expect_text(args.pop().unwrap(), "log.info")?;
            Ok(emit_log("info".to_string(), message, context))
        }),
    );
    fields.insert(
        "warn".to_string(),
        builtin("log.warn", 2, |mut args, _| {
            let context = context_map(args.pop().unwrap(), "log.warn")?;
            let message = expect_text(args.pop().unwrap(), "log.warn")?;
            Ok(emit_log("warn".to_string(), message, context))
        }),
    );
    fields.insert(
        "error".to_string(),
        builtin("log.error", 2, |mut args, _| {
            let context = context_map(args.pop().unwrap(), "log.error")?;
            let message = expect_text(args.pop().unwrap(), "log.error")?;
            Ok(emit_log("error".to_string(), message, context))
        }),
    );
    Value::Record(Arc::new(fields))
}
