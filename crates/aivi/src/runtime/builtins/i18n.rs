use std::collections::HashMap;
use std::sync::Arc;

use im::HashMap as ImHashMap;

use super::util::{builtin, expect_record, expect_text, list_value, make_err, make_ok, make_some};
use crate::i18n::{parse_locale_tag, parse_message_template, validate_key_text, MessagePart};
use crate::runtime::values::KeyValue;
use crate::runtime::{format_value, RuntimeError, Value};

pub(super) fn build_i18n_record() -> Value {
    let mut fields = HashMap::new();

    fields.insert(
        "parseLocale".to_string(),
        builtin("i18n.parseLocale", 1, |mut args, _| {
            let tag = expect_text(args.remove(0), "i18n.parseLocale")?;
            match parse_locale_tag(&tag) {
                Ok(parts) => Ok(make_ok(locale_value(parts))),
                Err(msg) => Ok(make_err(Value::Text(msg))),
            }
        }),
    );

    fields.insert(
        "key".to_string(),
        builtin("i18n.key", 1, |mut args, _| {
            let text = expect_text(args.remove(0), "i18n.key")?;
            match validate_key_text(&text) {
                Ok(()) => Ok(make_ok(Value::Record(Arc::new(HashMap::from([
                    ("tag".to_string(), Value::Text("k".to_string())),
                    ("body".to_string(), Value::Text(text.trim().to_string())),
                    ("flags".to_string(), Value::Text(String::new())),
                ]))))),
                Err(msg) => Ok(make_err(Value::Text(msg))),
            }
        }),
    );

    fields.insert(
        "message".to_string(),
        builtin("i18n.message", 1, |mut args, _| {
            let text = expect_text(args.remove(0), "i18n.message")?;
            match parse_message_template(&text) {
                Ok(parsed) => Ok(make_ok(message_value(text, &parsed.parts))),
                Err(msg) => Ok(make_err(Value::Text(msg))),
            }
        }),
    );

    fields.insert(
        "render".to_string(),
        builtin("i18n.render", 2, |mut args, _| {
            // Accept Unit as an empty record (AIVI parses `{}` as Unit).
            let raw_args = args.pop().unwrap();
            let args_rec = match &raw_args {
                Value::Unit => Arc::new(std::collections::HashMap::new()),
                _ => expect_record(raw_args, "i18n.render")?,
            };
            let msg = expect_record(args.pop().unwrap(), "i18n.render")?;
            let body = match msg.get("body") {
                Some(Value::Text(text)) => text.clone(),
                _ => {
                    return Err(RuntimeError::Message(
                        "i18n.render expects Message with field 'body : Text'".to_string(),
                    ))
                }
            };

            let parts = if let Some(Value::List(items)) = msg.get("parts") {
                // Trusted compilation output from `i18n.message`.
                Some(items.iter().cloned().collect::<Vec<_>>())
            } else {
                None
            };

            let rendered = match parts {
                Some(items) => render_parts(&items, &args_rec),
                None => match parse_message_template(&body) {
                    Ok(parsed) => render_parsed(&parsed.parts, &args_rec),
                    Err(msg) => return Ok(make_err(Value::Text(msg))),
                },
            }?;
            Ok(make_ok(Value::Text(rendered)))
        }),
    );

    fields.insert(
        "bundleFromProperties".to_string(),
        builtin("i18n.bundleFromProperties", 2, |mut args, _| {
            let props_text = expect_text(args.pop().unwrap(), "i18n.bundleFromProperties")?;
            let locale = expect_record(args.pop().unwrap(), "i18n.bundleFromProperties")?;

            let mut entries: ImHashMap<KeyValue, Value> = ImHashMap::new();
            for (line_no, raw_line) in props_text.lines().enumerate() {
                let line = raw_line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let (key_raw, value_raw) = split_kv(line).ok_or_else(|| {
                    RuntimeError::Message(format!(
                        "invalid properties line {}: expected 'key = value'",
                        line_no + 1
                    ))
                })?;
                let key_raw = key_raw.trim();
                let value_raw = value_raw.trim();
                validate_key_text(key_raw)
                    .map_err(|msg| RuntimeError::Message(format!("line {}: {msg}", line_no + 1)))?;

                let message_text = unescape_properties_value(value_raw)
                    .map_err(|msg| RuntimeError::Message(format!("line {}: {msg}", line_no + 1)))?;
                let parsed = parse_message_template(&message_text)
                    .map_err(|msg| RuntimeError::Message(format!("line {}: {msg}", line_no + 1)))?;
                let msg_value = message_value(message_text, &parsed.parts);
                entries.insert(KeyValue::Text(key_raw.to_string()), msg_value);
            }

            let bundle = Value::Record(Arc::new(HashMap::from([
                ("locale".to_string(), Value::Record(locale)),
                ("entries".to_string(), Value::Map(Arc::new(entries))),
            ])));
            Ok(make_ok(bundle))
        }),
    );

    Value::Record(Arc::new(fields))
}

fn locale_value(parts: crate::i18n::LocaleParts) -> Value {
    let region = match parts.region {
        Some(value) => make_some(Value::Text(value)),
        None => Value::Constructor {
            name: "None".to_string(),
            args: Vec::new(),
        },
    };
    let variants = list_value(parts.variants.into_iter().map(Value::Text).collect());
    Value::Record(Arc::new(HashMap::from([
        ("language".to_string(), Value::Text(parts.language)),
        ("region".to_string(), region),
        ("variants".to_string(), variants),
        ("tag".to_string(), Value::Text(parts.tag)),
    ])))
}

fn message_value(body: String, parts: &[MessagePart]) -> Value {
    let parts_value = list_value(
        parts
            .iter()
            .map(|part| match part {
                MessagePart::Lit(text) => Value::Record(Arc::new(HashMap::from([
                    ("kind".to_string(), Value::Text("lit".to_string())),
                    ("text".to_string(), Value::Text(text.clone())),
                ]))),
                MessagePart::Hole { name, ty } => Value::Record(Arc::new(HashMap::from([
                    ("kind".to_string(), Value::Text("hole".to_string())),
                    ("name".to_string(), Value::Text(name.clone())),
                    (
                        "ty".to_string(),
                        match ty {
                            Some(t) => make_some(Value::Text(t.clone())),
                            None => Value::Constructor {
                                name: "None".to_string(),
                                args: Vec::new(),
                            },
                        },
                    ),
                ]))),
            })
            .collect(),
    );

    Value::Record(Arc::new(HashMap::from([
        ("tag".to_string(), Value::Text("m".to_string())),
        ("body".to_string(), Value::Text(body)),
        ("flags".to_string(), Value::Text(String::new())),
        ("parts".to_string(), parts_value),
    ])))
}

fn render_parts(
    parts: &[Value],
    args: &std::collections::HashMap<String, Value>,
) -> Result<String, RuntimeError> {
    let mut out = String::new();
    for part in parts {
        let rec = expect_record(part.clone(), "i18n.render")?;
        let kind = match rec.get("kind") {
            Some(Value::Text(k)) => k.as_str(),
            _ => {
                return Err(RuntimeError::Message(
                    "i18n.render expects compiled message parts".to_string(),
                ))
            }
        };
        match kind {
            "lit" => match rec.get("text") {
                Some(Value::Text(text)) => out.push_str(text),
                _ => {
                    return Err(RuntimeError::Message(
                        "i18n.render expects lit part with 'text : Text'".to_string(),
                    ))
                }
            },
            "hole" => {
                let name = match rec.get("name") {
                    Some(Value::Text(name)) => name.as_str(),
                    _ => {
                        return Err(RuntimeError::Message(
                            "i18n.render expects hole part with 'name : Text'".to_string(),
                        ))
                    }
                };
                let expected_ty = match rec.get("ty") {
                    Some(Value::Constructor { name, args })
                        if name == "Some" && args.len() == 1 =>
                    {
                        match &args[0] {
                            Value::Text(t) => Some(t.as_str()),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                let value = args.get(name).ok_or_else(|| {
                    RuntimeError::Message(format!("missing message arg '{name}'"))
                })?;
                if let Some(ty) = expected_ty {
                    if !matches_type(ty, value) {
                        return Err(RuntimeError::Message(format!(
                            "type mismatch for arg '{name}': expected {ty}, got {}",
                            value_type_name(value)
                        )));
                    }
                }
                out.push_str(&format_value(value));
            }
            other => {
                return Err(RuntimeError::Message(format!(
                    "i18n.render: unknown part kind '{other}'"
                )))
            }
        }
    }
    Ok(out)
}

fn render_parsed(
    parts: &[MessagePart],
    args: &std::collections::HashMap<String, Value>,
) -> Result<String, RuntimeError> {
    let mut out = String::new();
    for part in parts {
        match part {
            MessagePart::Lit(text) => out.push_str(text),
            MessagePart::Hole { name, ty } => {
                let value = args.get(name).ok_or_else(|| {
                    RuntimeError::Message(format!("missing message arg '{name}'"))
                })?;
                if let Some(ty) = ty.as_deref() {
                    if !matches_type(ty, value) {
                        return Err(RuntimeError::Message(format!(
                            "type mismatch for arg '{name}': expected {ty}, got {}",
                            value_type_name(value)
                        )));
                    }
                }
                out.push_str(&format_value(value));
            }
        }
    }
    Ok(out)
}

fn matches_type(expected: &str, value: &Value) -> bool {
    matches!(
        (expected, value),
        ("Text", Value::Text(_))
            | ("Int", Value::Int(_))
            | ("Float", Value::Float(_))
            | ("Bool", Value::Bool(_))
            | ("Decimal", Value::Decimal(_))
            | ("DateTime", Value::DateTime(_))
    )
}

fn value_type_name(value: &Value) -> &'static str {
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
        Value::Builtin(_) => "Builtin",
        Value::Effect(_) => "Effect",
        Value::Source(_) => "Source",
        Value::Resource(_) => "Resource",
        Value::Thunk(_) => "Thunk",
        Value::MultiClause(_) => "MultiClause",
        Value::ChannelSend(_) => "Send",
        Value::ChannelRecv(_) => "Recv",
        Value::FileHandle(_) => "FileHandle",
        Value::Listener(_) => "Listener",
        Value::Connection(_) => "Connection",
        Value::Stream(_) => "Stream",
        Value::HttpServer(_) => "HttpServer",
        Value::WebSocket(_) => "WebSocket",
    }
}

fn split_kv(line: &str) -> Option<(&str, &str)> {
    if let Some((k, v)) = line.split_once('=') {
        return Some((k, v));
    }
    line.split_once(':')
}

fn unescape_properties_value(value: &str) -> Result<String, String> {
    // Minimal escape handling:
    // - \\n, \\r, \\t
    // - \\\\, \\"
    // Anything else keeps the escaped char as-is.
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let Some(next) = chars.next() else {
            return Err("dangling escape at end of line".to_string());
        };
        match next {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            other => out.push(other),
        }
    }
    Ok(out)
}
