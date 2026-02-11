use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::util::{builtin, make_err, make_ok};
use crate::runtime::{format_value, EffectValue, RuntimeError, Value};
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
            Ok(Value::Effect(Arc::new(effect)))
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
                    let size = i64::try_from(metadata.len())
                        .map_err(|_| RuntimeError::Error(Value::Text("file too large".to_string())))?;
                    let created = i64::try_from(created_ms)
                        .map_err(|_| RuntimeError::Error(Value::Text("timestamp overflow".to_string())))?;
                    let modified = i64::try_from(modified_ms)
                        .map_err(|_| RuntimeError::Error(Value::Text("timestamp overflow".to_string())))?;
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
    Value::Record(Arc::new(fields))
}
