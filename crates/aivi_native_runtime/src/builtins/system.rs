use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::util::{builtin, expect_record, expect_text, make_err, make_none, make_ok, make_some};
use crate::{format_value, EffectValue, RuntimeError, SourceValue, Value};

pub(super) fn build_file_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "read".to_string(),
        builtin("file.read", 1, |mut args, _| {
            let path = match args.remove(0) {
                Value::Text(text) => text,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.read expects Text path".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| match std::fs::read_to_string(&path) {
                    Ok(text) => Ok(Value::Text(text)),
                    Err(err) => Err(RuntimeError::Error(Value::Text(err.to_string()))),
                }),
            };
            Ok(Value::Source(Arc::new(SourceValue {
                kind: "File".to_string(),
                effect: Arc::new(effect),
            })))
        }),
    );
    fields.insert(
        "open".to_string(),
        builtin("file.open", 1, |mut args, _| {
            let path = match args.remove(0) {
                Value::Text(text) => text,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.open expects Text path".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| match std::fs::File::open(&path) {
                    Ok(file) => Ok(Value::FileHandle(Arc::new(Mutex::new(file)))),
                    Err(err) => Err(RuntimeError::Error(Value::Text(err.to_string()))),
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "close".to_string(),
        builtin("file.close", 1, |mut args, _| {
            let _handle = match args.remove(0) {
                Value::FileHandle(handle) => handle,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.close expects a file handle".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| Ok(Value::Unit)),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "readAll".to_string(),
        builtin("file.readAll", 1, |mut args, _| {
            let handle = match args.remove(0) {
                Value::FileHandle(handle) => handle,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.readAll expects a file handle".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut file = handle
                        .lock()
                        .map_err(|_| RuntimeError::Message("file handle poisoned".to_string()))?;
                    let _ = std::io::Seek::seek(&mut *file, std::io::SeekFrom::Start(0));
                    let mut buffer = String::new();
                    std::io::Read::read_to_string(&mut *file, &mut buffer)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.to_string())))?;
                    Ok(Value::Text(buffer))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "write_text".to_string(),
        builtin("file.write_text", 2, |mut args, _| {
            let content = match args.remove(1) {
                Value::Text(text) => text,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.write_text expects Text content".to_string(),
                    ))
                }
            };
            let path = match args.remove(0) {
                Value::Text(text) => text,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.write_text expects Text path".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| match std::fs::write(&path, content.as_bytes()) {
                    Ok(()) => Ok(Value::Unit),
                    Err(err) => Err(RuntimeError::Error(Value::Text(err.to_string()))),
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "exists".to_string(),
        builtin("file.exists", 1, |mut args, _| {
            let path = match args.remove(0) {
                Value::Text(text) => text,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.exists expects Text path".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| Ok(Value::Bool(std::path::Path::new(&path).exists()))),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "stat".to_string(),
        builtin("file.stat", 1, |mut args, _| {
            let path = match args.remove(0) {
                Value::Text(text) => text,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.stat expects Text path".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let metadata = std::fs::metadata(&path)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.to_string())))?;
                    let created = metadata
                        .created()
                        .map_err(|err| RuntimeError::Error(Value::Text(err.to_string())))?;
                    let modified = metadata
                        .modified()
                        .map_err(|err| RuntimeError::Error(Value::Text(err.to_string())))?;
                    let created_ms = created
                        .duration_since(UNIX_EPOCH)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.to_string())))?
                        .as_millis();
                    let modified_ms = modified
                        .duration_since(UNIX_EPOCH)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.to_string())))?
                        .as_millis();
                    let size = i64::try_from(metadata.len()).map_err(|_| {
                        RuntimeError::Error(Value::Text("file too large".to_string()))
                    })?;
                    let created = i64::try_from(created_ms).map_err(|_| {
                        RuntimeError::Error(Value::Text("timestamp overflow".to_string()))
                    })?;
                    let modified = i64::try_from(modified_ms).map_err(|_| {
                        RuntimeError::Error(Value::Text("timestamp overflow".to_string()))
                    })?;
                    let mut stats = HashMap::new();
                    stats.insert("size".to_string(), Value::Int(size));
                    stats.insert("created".to_string(), Value::Int(created));
                    stats.insert("modified".to_string(), Value::Int(modified));
                    stats.insert("isFile".to_string(), Value::Bool(metadata.is_file()));
                    stats.insert("isDirectory".to_string(), Value::Bool(metadata.is_dir()));
                    Ok(Value::Record(Arc::new(stats)))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "delete".to_string(),
        builtin("file.delete", 1, |mut args, _| {
            let path = match args.remove(0) {
                Value::Text(text) => text,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.delete expects Text path".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| match std::fs::remove_file(&path) {
                    Ok(()) => Ok(Value::Unit),
                    Err(err) => Err(RuntimeError::Error(Value::Text(err.to_string()))),
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(Arc::new(fields))
}

pub(super) fn build_clock_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "now".to_string(),
        builtin("clock.now", 1, |_, _| {
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

pub(super) fn build_console_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "log".to_string(),
        builtin("console.log", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    println!("{text}");
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "println".to_string(),
        builtin("console.println", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    println!("{text}");
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "print".to_string(),
        builtin("console.print", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    print!("{text}");
                    let mut out = std::io::stdout();
                    let _ = out.flush();
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "error".to_string(),
        builtin("console.error", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    eprintln!("{text}");
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "readLine".to_string(),
        builtin("console.readLine", 1, |_, _| {
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut buffer = String::new();
                    match std::io::stdin().read_line(&mut buffer) {
                        Ok(_) => Ok(make_ok(Value::Text(
                            buffer.trim_end_matches(&['\n', '\r'][..]).to_string(),
                        ))),
                        Err(err) => Ok(make_err(Value::Text(err.to_string()))),
                    }
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "color".to_string(),
        builtin("console.color", 2, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "console.color")?;
            let color = ansi_color(args.pop().unwrap(), false, "console.color")?;
            Ok(Value::Text(apply_ansi(&[color], &value)))
        }),
    );
    fields.insert(
        "bgColor".to_string(),
        builtin("console.bgColor", 2, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "console.bgColor")?;
            let color = ansi_color(args.pop().unwrap(), true, "console.bgColor")?;
            Ok(Value::Text(apply_ansi(&[color], &value)))
        }),
    );
    fields.insert(
        "style".to_string(),
        builtin("console.style", 2, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "console.style")?;
            let style = args.pop().unwrap();
            let codes = style_codes(style, "console.style")?;
            Ok(Value::Text(apply_ansi(&codes, &value)))
        }),
    );
    fields.insert(
        "strip".to_string(),
        builtin("console.strip", 1, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "console.strip")?;
            Ok(Value::Text(strip_ansi(&value)))
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
        builtin("system.env.get", 1, |mut args, _| {
            let key = expect_text(args.pop().unwrap(), "system.env.get")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| match std::env::var(&key) {
                    Ok(value) => Ok(make_some(Value::Text(value))),
                    Err(std::env::VarError::NotPresent) => Ok(make_none()),
                    Err(err) => Err(RuntimeError::Error(Value::Text(err.to_string()))),
                }),
            };
            Ok(Value::Source(Arc::new(SourceValue {
                kind: "Env".to_string(),
                effect: Arc::new(effect),
            })))
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
        builtin("env.get", 1, |mut args, _| {
            let key = expect_text(args.pop().unwrap(), "env.get")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| match std::env::var(&key) {
                    Ok(value) => Ok(Value::Text(value)),
                    Err(std::env::VarError::NotPresent) => Err(RuntimeError::Error(Value::Text(
                        format!("env var not set: {key}"),
                    ))),
                    Err(err) => Err(RuntimeError::Error(Value::Text(err.to_string()))),
                }),
            };
            Ok(Value::Source(Arc::new(SourceValue {
                kind: "Env".to_string(),
                effect: Arc::new(effect),
            })))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn ansi_color(value: Value, is_bg: bool, ctx: &str) -> Result<i64, RuntimeError> {
    let name = match value {
        Value::Constructor { name, args } if args.is_empty() => name,
        _ => return Err(RuntimeError::Message(format!("{ctx} expects AnsiColor"))),
    };
    let base = if is_bg { 40 } else { 30 };
    let code = match name.as_str() {
        "Black" => base,
        "Red" => base + 1,
        "Green" => base + 2,
        "Yellow" => base + 3,
        "Blue" => base + 4,
        "Magenta" => base + 5,
        "Cyan" => base + 6,
        "White" => base + 7,
        "Default" => {
            if is_bg {
                49
            } else {
                39
            }
        }
        _ => return Err(RuntimeError::Message(format!("{ctx} expects AnsiColor"))),
    };
    Ok(code)
}

fn style_codes(value: Value, ctx: &str) -> Result<Vec<i64>, RuntimeError> {
    let fields = expect_record(value, ctx)?;
    let fg = fields
        .get("fg")
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects fg")))?;
    let bg = fields
        .get("bg")
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects bg")))?;
    let mut codes = Vec::new();
    if let Some(code) = option_color(fg.clone(), false, ctx)? {
        codes.push(code);
    }
    if let Some(code) = option_color(bg.clone(), true, ctx)? {
        codes.push(code);
    }
    let flags = [
        ("bold", 1),
        ("dim", 2),
        ("italic", 3),
        ("underline", 4),
        ("blink", 5),
        ("inverse", 7),
        ("hidden", 8),
        ("strike", 9),
    ];
    for (field, code) in flags {
        let value = fields
            .get(field)
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects {field}")))?;
        if expect_bool(value.clone(), ctx)? {
            codes.push(code);
        }
    }
    Ok(codes)
}

fn option_color(value: Value, is_bg: bool, ctx: &str) -> Result<Option<i64>, RuntimeError> {
    match value {
        Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
            Ok(Some(ansi_color(args[0].clone(), is_bg, ctx)?))
        }
        Value::Constructor { name, args } if name == "None" && args.is_empty() => Ok(None),
        other => Err(RuntimeError::Message(format!(
            "{ctx} expects Option AnsiColor, got {}",
            format_value(&other)
        ))),
    }
}

fn expect_bool(value: Value, ctx: &str) -> Result<bool, RuntimeError> {
    match value {
        Value::Bool(value) => Ok(value),
        other => Err(RuntimeError::Message(format!(
            "{ctx} expects Bool, got {}",
            format_value(&other)
        ))),
    }
}

fn apply_ansi(codes: &[i64], value: &str) -> String {
    if codes.is_empty() {
        return value.to_string();
    }
    let joined = codes
        .iter()
        .map(|code| code.to_string())
        .collect::<Vec<_>>()
        .join(";");
    format!("\x1b[{joined}m{value}\x1b[0m")
}

fn strip_ansi(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for code in chars.by_ref() {
                if code == 'm' {
                    break;
                }
            }
            continue;
        }
        out.push(ch);
    }
    out
}
