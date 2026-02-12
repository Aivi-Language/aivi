use std::collections::HashMap;
use std::sync::Arc;

use im::HashMap as ImHashMap;

use super::util::{builtin, expect_record, expect_text, list_value, make_err, make_ok, make_some};
use crate::values::KeyValue;
use crate::{format_value, RuntimeError, Value};

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
                Ok(parts) => Ok(make_ok(message_value(text, &parts))),
                Err(msg) => Ok(make_err(Value::Text(msg))),
            }
        }),
    );

    fields.insert(
        "render".to_string(),
        builtin("i18n.render", 2, |mut args, _| {
            let args_rec = expect_record(args.pop().unwrap(), "i18n.render")?;
            let msg = expect_record(args.pop().unwrap(), "i18n.render")?;
            let body = match msg.get("body") {
                Some(Value::Text(text)) => text.clone(),
                _ => {
                    return Err(RuntimeError::Message(
                        "i18n.render expects Message with field 'body : Text'".to_string(),
                    ))
                }
            };

            let rendered = if let Some(Value::List(items)) = msg.get("parts") {
                render_compiled_parts(items, &args_rec)?
            } else {
                let parts = parse_message_template(&body).map_err(RuntimeError::Message)?;
                render_parts(&parts, &args_rec)?
            };

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
                let n = line_no + 1;
                let line = raw_line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let (key_raw, value_raw) = split_kv(line).ok_or_else(|| {
                    RuntimeError::Message(format!(
                        "invalid properties line {n}: expected 'key = value'"
                    ))
                })?;
                let key_raw = key_raw.trim();
                validate_key_text(key_raw)
                    .map_err(|msg| RuntimeError::Message(format!("line {n}: {msg}")))?;

                let message_text = unescape_properties_value(value_raw.trim())
                    .map_err(|msg| RuntimeError::Message(format!("line {n}: {msg}")))?;
                let parts = parse_message_template(&message_text)
                    .map_err(|msg| RuntimeError::Message(format!("line {n}: {msg}")))?;
                entries.insert(
                    KeyValue::Text(key_raw.to_string()),
                    message_value(message_text, &parts),
                );
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

#[derive(Debug, Clone)]
struct LocaleParts {
    language: String,
    region: Option<String>,
    variants: Vec<String>,
    tag: String,
}

fn parse_locale_tag(tag: &str) -> Result<LocaleParts, String> {
    let raw = tag.trim();
    if raw.is_empty() {
        return Err("locale tag is empty".to_string());
    }
    let subtags: Vec<&str> = raw
        .split(|c| c == '-' || c == '_')
        .filter(|s| !s.is_empty())
        .collect();
    if subtags.is_empty() {
        return Err("locale tag is empty".to_string());
    }

    let language_raw = subtags[0];
    if !language_raw.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err("language subtag must be ASCII letters".to_string());
    }
    if !(2..=8).contains(&language_raw.len()) {
        return Err("language subtag must be 2..=8 letters".to_string());
    }
    let language = language_raw.to_ascii_lowercase();

    let mut region = None;
    let mut variants = Vec::new();

    let mut i = 1;
    if i < subtags.len() {
        let s = subtags[i];
        let is_region = (s.len() == 2 && s.chars().all(|c| c.is_ascii_alphabetic()))
            || (s.len() == 3 && s.chars().all(|c| c.is_ascii_digit()));
        if is_region {
            region = Some(s.to_ascii_uppercase());
            i += 1;
        }
    }

    while i < subtags.len() {
        let s = subtags[i];
        if !s.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(format!("invalid locale subtag '{s}'"));
        }
        if !(4..=8).contains(&s.len()) {
            return Err(format!(
                "invalid locale subtag '{s}': expected 4..=8 alphanumeric"
            ));
        }
        variants.push(s.to_string());
        i += 1;
    }

    let mut normalized = language.clone();
    if let Some(r) = &region {
        normalized.push('-');
        normalized.push_str(r);
    }
    for v in &variants {
        normalized.push('-');
        normalized.push_str(v);
    }

    Ok(LocaleParts {
        language,
        region,
        variants,
        tag: normalized,
    })
}

fn locale_value(parts: LocaleParts) -> Value {
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

#[derive(Debug, Clone)]
enum MessagePart {
    Lit(String),
    Hole { name: String, ty: Option<String> },
}

fn parse_message_template(text: &str) -> Result<Vec<MessagePart>, String> {
    // Same minimal grammar as the compiler-side helper:
    // - '{{' and '}}' for literal braces
    // - placeholders: {name} or {name:Type}
    let mut parts = Vec::new();
    let mut buf = String::new();

    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                if matches!(chars.peek(), Some('{')) {
                    chars.next();
                    buf.push('{');
                    continue;
                }
                if !buf.is_empty() {
                    parts.push(MessagePart::Lit(std::mem::take(&mut buf)));
                }
                let mut inner = String::new();
                let mut closed = false;
                while let Some(next) = chars.next() {
                    if next == '}' {
                        closed = true;
                        break;
                    }
                    inner.push(next);
                }
                if !closed {
                    return Err("unclosed '{' in message template".to_string());
                }
                let inner = inner.trim();
                if inner.is_empty() {
                    return Err("empty placeholder '{}'".to_string());
                }
                let (name, ty) = match inner.split_once(':') {
                    Some((n, t)) => (n.trim(), Some(t.trim())),
                    None => (inner, None),
                };
                validate_placeholder_name(name)?;
                let ty = ty
                    .filter(|t| !t.is_empty())
                    .map(|t| validate_placeholder_type(t).map(|_| t.to_string()))
                    .transpose()?;
                parts.push(MessagePart::Hole {
                    name: name.to_string(),
                    ty,
                });
            }
            '}' => {
                if matches!(chars.peek(), Some('}')) {
                    chars.next();
                    buf.push('}');
                    continue;
                }
                return Err("unexpected '}' (use '}}' to escape)".to_string());
            }
            other => buf.push(other),
        }
    }
    if !buf.is_empty() {
        parts.push(MessagePart::Lit(buf));
    }
    Ok(parts)
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

fn render_compiled_parts(
    parts: &Arc<Vec<Value>>,
    args: &std::collections::HashMap<String, Value>,
) -> Result<String, RuntimeError> {
    let mut out = String::new();
    for part in parts.iter().cloned() {
        let rec = expect_record(part, "i18n.render")?;
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

fn render_parts(
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
    match (expected, value) {
        ("Text", Value::Text(_)) => true,
        ("Int", Value::Int(_)) => true,
        ("Float", Value::Float(_)) => true,
        ("Bool", Value::Bool(_)) => true,
        ("Decimal", Value::Decimal(_)) => true,
        ("DateTime", Value::DateTime(_)) => true,
        _ => false,
    }
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
        Value::Closure(_) => "Closure",
        Value::Builtin(_) => "Builtin",
        Value::Effect(_) => "Effect",
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

fn validate_key_text(text: &str) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("key is empty".to_string());
    }
    if text.starts_with('.') || text.ends_with('.') || text.contains("..") {
        return Err("key must not have empty segments".to_string());
    }
    for segment in text.split('.') {
        validate_key_segment(segment)?;
    }
    Ok(())
}

fn validate_key_segment(segment: &str) -> Result<(), String> {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return Err("key segment is empty".to_string());
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(format!(
            "invalid key segment '{segment}': must start with ASCII letter or '_'"
        ));
    }
    for ch in chars {
        if !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') {
            return Err(format!(
                "invalid key segment '{segment}': unexpected char '{ch}'"
            ));
        }
    }
    Ok(())
}

fn validate_placeholder_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err("placeholder name is empty".to_string());
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(format!(
            "invalid placeholder name '{name}': must start with ASCII letter or '_'"
        ));
    }
    for ch in chars {
        if !(ch.is_ascii_alphanumeric() || ch == '_') {
            return Err(format!(
                "invalid placeholder name '{name}': unexpected char '{ch}'"
            ));
        }
    }
    Ok(())
}

fn validate_placeholder_type(ty: &str) -> Result<(), String> {
    match ty {
        "Text" | "Int" | "Float" | "Bool" | "Decimal" | "DateTime" => Ok(()),
        other => Err(format!("unsupported placeholder type '{other}'")),
    }
}

fn split_kv(line: &str) -> Option<(&str, &str)> {
    if let Some((k, v)) = line.split_once('=') {
        return Some((k, v));
    }
    line.split_once(':')
}

fn unescape_properties_value(value: &str) -> Result<String, String> {
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
