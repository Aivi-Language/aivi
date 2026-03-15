mod console;
mod file;

pub(super) use console::build_console_record;
pub(super) use file::build_file_record;

use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value as JsonValue;

use super::util::{
    builtin, expect_list, expect_text, make_decode_error, make_none, make_some,
    make_source_decode_error, value_type_name,
};
use crate::runtime::{EffectValue, RuntimeError, SourceValue, Value};

pub(super) fn build_clock_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "now".to_string(),
        builtin("clock.now", 1, |_args, _runtime| {
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or(Duration::from_secs(0));
                    let text = format!("{}.{:09}Z", now.as_secs(), now.subsec_nanos());
                    Ok(Value::DateTime(text))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(Arc::new(fields))
}

pub(super) fn build_random_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "int".to_string(),
        builtin("random.int", 2, |mut args, _runtime| {
            let max = match args.pop().unwrap() {
                Value::Int(value) => value,
                _ => {
                    return Err(RuntimeError::Message(
                        "random.int expects Int bounds".to_string(),
                    ))
                }
            };
            let min = match args.pop().unwrap() {
                Value::Int(value) => value,
                _ => {
                    return Err(RuntimeError::Message(
                        "random.int expects Int bounds".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let (low, high) = if min <= max { (min, max) } else { (max, min) };
                    let span = (high - low + 1) as u64;
                    let value = (runtime.next_u64() % span) as i64 + low;
                    Ok(Value::Int(value))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(Arc::new(fields))
}

pub(super) fn build_system_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert("env".to_string(), build_env_record());
    fields.insert(
        "args".to_string(),
        builtin("system.args", 1, |_, _| {
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let args: Vec<Value> = std::env::args().skip(1).map(Value::Text).collect();
                    Ok(Value::List(Arc::new(args)))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "localeTag".to_string(),
        builtin("system.localeTag", 1, |_, _| {
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| match system_locale_tag_best_effort() {
                    Some(tag) => Ok(make_some(Value::Text(tag))),
                    None => Ok(make_none()),
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "exit".to_string(),
        builtin("system.exit", 1, |mut args, _| {
            let code = match args.pop().unwrap() {
                Value::Int(value) => value,
                _ => return Err(RuntimeError::Message("system.exit expects Int".to_string())),
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| std::process::exit(code as i32)),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "run".to_string(),
        builtin("system.run", 2, |mut args, _| {
            let raw_args = expect_list(args.pop().unwrap(), "system.run")?;
            let command = expect_text(args.pop().unwrap(), "system.run")?;
            let command_args = text_args_from_values(raw_args, "system.run")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| run_command_effect(&command, &command_args)),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn system_locale_tag_best_effort() -> Option<String> {
    let raw = std::env::var("LC_ALL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            std::env::var("LC_MESSAGES")
                .ok()
                .filter(|v| !v.trim().is_empty())
        })
        .or_else(|| std::env::var("LANG").ok().filter(|v| !v.trim().is_empty()))?;

    let tag = raw.trim();
    let base = tag.split('.').next().unwrap_or(tag);
    let clean = base.split('@').next().unwrap_or(base).trim();
    if clean.is_empty() {
        None
    } else {
        Some(clean.to_string())
    }
}

fn build_env_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "get".to_string(),
        builtin("system.env.get", 1, |mut args, _runtime| {
            let key = expect_text(args.pop().unwrap(), "system.env.get")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| match std::env::var(&key) {
                    Ok(value) => Ok(Value::Text(value)),
                    Err(_) => Err(RuntimeError::Error(Value::Text(format!(
                        "env var not found: {key}"
                    )))),
                }),
            };
            Ok(Value::Source(Arc::new(SourceValue::new(
                "Env".to_string(),
                Arc::new(effect),
            ))))
        }),
    );
    fields.insert(
        "decode".to_string(),
        builtin("system.env.decode", 1, |mut args, _runtime| {
            let prefix = env_decode_prefix(args.pop().unwrap(), "system.env.decode")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut map = HashMap::new();
                    for (key, value) in std::env::vars() {
                        if key.starts_with(&prefix) {
                            let suffix = key.trim_start_matches(&prefix).trim_start_matches('_');
                            let out_key = if suffix.is_empty() {
                                key
                            } else {
                                suffix.to_lowercase()
                            };
                            map.insert(out_key, scalar_text_to_value(&value));
                        }
                    }
                    if map.is_empty() {
                        return Err(RuntimeError::Error(make_source_decode_error(vec![
                            make_decode_error(
                                Vec::new(),
                                format!("no environment variables found for prefix `{prefix}`"),
                            ),
                        ])));
                    }
                    Ok(Value::Record(Arc::new(map)))
                }),
            };
            Ok(Value::Source(Arc::new(SourceValue::new(
                "Env".to_string(),
                Arc::new(effect),
            ))))
        }),
    );
    fields.insert(
        "set".to_string(),
        builtin("system.env.set", 2, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "system.env.set")?;
            let key = expect_text(args.pop().unwrap(), "system.env.set")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    std::env::set_var(&key, &value);
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "remove".to_string(),
        builtin("system.env.remove", 1, |mut args, _| {
            let key = expect_text(args.pop().unwrap(), "system.env.remove")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    std::env::remove_var(&key);
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(Arc::new(fields))
}

pub(super) fn build_env_source_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "get".to_string(),
        builtin("env.get", 1, |mut args, _runtime| {
            let key = expect_text(args.pop().unwrap(), "env.get")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| match std::env::var(&key) {
                    Ok(value) => Ok(Value::Text(value)),
                    Err(_) => Err(RuntimeError::Error(Value::Text(format!(
                        "env var not found: {key}"
                    )))),
                }),
            };
            Ok(Value::Source(Arc::new(SourceValue::new(
                "Env".to_string(),
                Arc::new(effect),
            ))))
        }),
    );
    fields.insert(
        "decode".to_string(),
        builtin("env.decode", 1, |mut args, _runtime| {
            let prefix = env_decode_prefix(args.pop().unwrap(), "env.decode")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut map = HashMap::new();
                    for (key, value) in std::env::vars() {
                        if key.starts_with(&prefix) {
                            let suffix = key.trim_start_matches(&prefix).trim_start_matches('_');
                            let out_key = if suffix.is_empty() {
                                key
                            } else {
                                suffix.to_lowercase()
                            };
                            map.insert(out_key, scalar_text_to_value(&value));
                        }
                    }
                    if map.is_empty() {
                        return Err(RuntimeError::Error(make_source_decode_error(vec![
                            make_decode_error(
                                Vec::new(),
                                format!("no environment variables found for prefix `{prefix}`"),
                            ),
                        ])));
                    }
                    Ok(Value::Record(Arc::new(map)))
                }),
            };
            Ok(Value::Source(Arc::new(SourceValue::new(
                "Env".to_string(),
                Arc::new(effect),
            ))))
        }),
    );
    fields.insert(
        "set".to_string(),
        builtin("env.set", 2, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "env.set")?;
            let key = expect_text(args.pop().unwrap(), "env.set")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    std::env::set_var(&key, &value);
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "remove".to_string(),
        builtin("env.remove", 1, |mut args, _| {
            let key = expect_text(args.pop().unwrap(), "env.remove")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    std::env::remove_var(&key);
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn scalar_text_to_value(raw: &str) -> Value {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }
    if trimmed.eq_ignore_ascii_case("null") {
        return make_none();
    }
    if let Ok(int) = trimmed.parse::<i64>() {
        return Value::Int(int);
    }
    if let Ok(float) = trimmed.parse::<f64>() {
        return Value::Float(float);
    }
    Value::Text(raw.to_string())
}

fn env_decode_prefix(arg: Value, ctx: &str) -> Result<String, RuntimeError> {
    match arg {
        Value::Text(prefix) => Ok(prefix),
        Value::Record(record) => match record.get("prefix") {
            Some(Value::Text(prefix)) => Ok(prefix.clone()),
            Some(other) => Err(RuntimeError::TypeError {
                context: ctx.to_string(),
                expected: "Text".to_string(),
                got: super::util::value_type_name(other).to_string(),
            }),
            None => Err(RuntimeError::Message(format!(
                "{ctx} expects config.prefix"
            ))),
        },
        other => Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "Text or Record".to_string(),
            got: super::util::value_type_name(&other).to_string(),
        }),
    }
}

fn text_args_from_values(values: Arc<Vec<Value>>, ctx: &str) -> Result<Vec<String>, RuntimeError> {
    values
        .iter()
        .map(|value| match value {
            Value::Text(text) => Ok(text.clone()),
            other => Err(RuntimeError::TypeError {
                context: ctx.to_string(),
                expected: "List Text".to_string(),
                got: value_type_name(other).to_string(),
            }),
        })
        .collect()
}

fn run_command_effect(command: &str, args: &[String]) -> Result<Value, RuntimeError> {
    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|err| RuntimeError::Error(Value::Text(format!(
            "system.run failed for `{command}`: {err}"
        ))))?;

    let mut fields = HashMap::new();
    fields.insert(
        "status".to_string(),
        Value::Int(output.status.code().unwrap_or(-1) as i64),
    );
    fields.insert(
        "stdout".to_string(),
        Value::Text(String::from_utf8_lossy(&output.stdout).to_string()),
    );
    fields.insert(
        "stderr".to_string(),
        Value::Text(String::from_utf8_lossy(&output.stderr).to_string()),
    );
    Ok(Value::Record(Arc::new(fields)))
}

pub(in crate::runtime::builtins) fn json_to_runtime(value: &JsonValue) -> Value {
    json_to_runtime_with_schema(value, None)
}

#[cfg(test)]
mod tests {
    use super::run_command_effect;
    use crate::runtime::Value;

    #[test]
    fn system_run_captures_stdout() {
        let result = match run_command_effect("sh", &["-c".to_string(), "printf hello".to_string()]) {
            Ok(value) => value,
            Err(_) => panic!("command should run"),
        };
        let Value::Record(fields) = result else {
            panic!("expected record result");
        };
        assert!(matches!(fields.get("status"), Some(Value::Int(0))));
        assert!(matches!(fields.get("stdout"), Some(Value::Text(text)) if text == "hello"));
        assert!(matches!(fields.get("stderr"), Some(Value::Text(text)) if text.is_empty()));
    }

    #[test]
    fn system_run_captures_nonzero_status_and_stderr() {
        let result = match run_command_effect(
            "sh",
            &["-c".to_string(), "printf fail >&2; exit 3".to_string()],
        )
        {
            Ok(value) => value,
            Err(_) => panic!("command should run"),
        };
        let Value::Record(fields) = result else {
            panic!("expected record result");
        };
        assert!(matches!(fields.get("status"), Some(Value::Int(3))));
        assert!(matches!(fields.get("stdout"), Some(Value::Text(text)) if text.is_empty()));
        assert!(matches!(fields.get("stderr"), Some(Value::Text(text)) if text == "fail"));
    }
}

/// Schema-aware JSON→Value conversion.  When a schema node is
/// `Option(inner)`, non-null JSON values are wrapped in `Some(…)` so that
/// the runtime `??` operator (which pattern-matches on `Some`/`None`)
/// works correctly.
pub(in crate::runtime::builtins) fn json_to_runtime_with_schema(
    value: &JsonValue,
    schema: Option<&crate::runtime::json_schema::JsonSchema>,
) -> Value {
    use crate::runtime::json_schema::JsonSchema;

    // If the schema says Option, handle the Some/None wrapping here.
    if let Some(JsonSchema::Option(inner)) = schema {
        return match value {
            JsonValue::Null => make_none(),
            other => make_some(json_to_runtime_with_schema(other, Some(inner))),
        };
    }

    match value {
        JsonValue::Null => make_none(),
        JsonValue::Bool(v) => Value::Bool(*v),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Text(n.to_string())
            }
        }
        JsonValue::String(s) if matches!(schema, Some(JsonSchema::Enum(_))) => {
            let constructor_name = match schema {
                Some(JsonSchema::Enum(variants)) => crate::runtime::json_schema::constructor_name_for_enum_value(variants, s)
                    .unwrap_or(s.as_str()),
                _ => s.as_str(),
            };
            Value::Constructor {
                name: constructor_name.to_string(),
                args: Vec::new(),
            }
        }
        JsonValue::String(s) => Value::Text(s.clone()),
        JsonValue::Array(items) => {
            let elem_schema = match schema {
                Some(JsonSchema::List(inner)) => Some(inner.as_ref()),
                _ => None,
            };
            Value::List(Arc::new(
                items
                    .iter()
                    .map(|item| json_to_runtime_with_schema(item, elem_schema))
                    .collect::<Vec<_>>(),
            ))
        }
        JsonValue::Object(map) => {
            let record_schema = match schema {
                Some(JsonSchema::Record(fields)) => Some(fields),
                _ => None,
            };
            let mut out = HashMap::new();
            for (key, value) in map {
                let field_schema = record_schema.and_then(|fields| fields.get(key.as_str()));
                out.insert(
                    key.clone(),
                    json_to_runtime_with_schema(value, field_schema),
                );
            }
            // Emit None for Option fields that are absent from the JSON object.
            if let Some(fields) = record_schema {
                for (key, field_schema) in fields {
                    if !out.contains_key(key.as_str())
                        && matches!(field_schema, JsonSchema::Option(_))
                    {
                        out.insert(key.clone(), make_none());
                    }
                }
            }
            Value::Record(Arc::new(out))
        }
    }
}

fn source_transport_error(kind: &str, context: &str, message: &str) -> String {
    format!("\x1b[31mtransport error\x1b[0m [{kind}] {context}\n\x1b[2m{message}\x1b[0m")
}

#[allow(clippy::too_many_arguments)]
fn source_decode_error(
    kind: &str,
    path: &str,
    expected: &str,
    received: &str,
    snippet: &str,
    line: usize,
    column: usize,
    context: &str,
) -> String {
    let mut out = format!(
        "\x1b[31mfailed to parse source\x1b[0m [{kind}] at \x1b[36m{path}\x1b[0m\n\
         expected \x1b[32m{expected}\x1b[0m but received \x1b[31m{received}\x1b[0m\n\
         {context}"
    );
    if !snippet.is_empty() {
        let line_text = snippet.lines().nth(line.saturating_sub(1)).unwrap_or("");
        let caret_col = column.saturating_sub(1);
        let caret = format!("{}^^^^", " ".repeat(caret_col));
        out.push('\n');
        out.push_str(&format!(
            "\x1b[2m{line:>4} |\x1b[0m {line_text}\n\x1b[2m     |\x1b[0m \x1b[33m{caret}\x1b[0m"
        ));
    }
    out
}

#[cfg(test)]
mod system_json_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_to_runtime_with_schema_maps_enum_strings_to_constructors() {
        let schema = crate::runtime::json_schema::JsonSchema::Enum(vec![
            crate::runtime::json_schema::EnumVariant {
                json_value: "notification".to_string(),
                constructor_name: "Notification".to_string(),
            },
            crate::runtime::json_schema::EnumVariant {
                json_value: "billing".to_string(),
                constructor_name: "Billing".to_string(),
            },
        ]);
        let value = json_to_runtime_with_schema(&json!("notification"), Some(&schema));
        assert!(matches!(
            value,
            Value::Constructor { name, args } if name == "Notification" && args.is_empty()
        ));
    }

    #[test]
    fn json_to_runtime_with_schema_wraps_optional_enums_in_some_constructor() {
        let schema = crate::runtime::json_schema::JsonSchema::Option(Box::new(
            crate::runtime::json_schema::JsonSchema::Enum(vec![
                crate::runtime::json_schema::EnumVariant {
                    json_value: "critical".to_string(),
                    constructor_name: "Critical".to_string(),
                },
                crate::runtime::json_schema::EnumVariant {
                    json_value: "normal".to_string(),
                    constructor_name: "Normal".to_string(),
                },
            ]),
        ));
        let value = json_to_runtime_with_schema(&json!("critical"), Some(&schema));
        assert!(matches!(
            value,
            Value::Constructor { name, args }
                if name == "Some"
                    && matches!(
                        args.as_slice(),
                        [Value::Constructor { name, args }] if name == "Critical" && args.is_empty()
                    )
        ));
    }
}
