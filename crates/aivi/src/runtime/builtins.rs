use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{Datelike, Duration as ChronoDuration, NaiveDate};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{ToPrimitive, Zero};
use palette::{FromColor, Hsl, RgbHue, Srgb};
use regex::Regex;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;
use url::Url;
use ureq::Error as UreqError;

use super::http::build_http_server_record;
use super::{
    format_value, CancelToken, EffectValue, Env, Runtime, RuntimeContext, RuntimeError, Value,
};
use super::values::{
    BuiltinImpl, BuiltinValue, ChannelInner, ChannelRecv, ChannelSend,
};

pub(super) fn register_builtins(env: &Env) {
    env.set("Unit".to_string(), Value::Unit);
    env.set("True".to_string(), Value::Bool(true));
    env.set("False".to_string(), Value::Bool(false));
    env.set(
        "None".to_string(),
        Value::Constructor {
            name: "None".to_string(),
            args: Vec::new(),
        },
    );
    env.set("Some".to_string(), builtin_constructor("Some", 1));
    env.set("Ok".to_string(), builtin_constructor("Ok", 1));
    env.set("Err".to_string(), builtin_constructor("Err", 1));
    env.set(
        "Closed".to_string(),
        Value::Constructor {
            name: "Closed".to_string(),
            args: Vec::new(),
        },
    );

    env.set(
        "pure".to_string(),
        builtin("pure", 1, |mut args, _| {
            let value = args.remove(0);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| Ok(value.clone())),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    env.set(
        "fail".to_string(),
        builtin("fail", 1, |mut args, _| {
            let value = args.remove(0);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| Err(RuntimeError::Error(value.clone()))),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    env.set(
        "bind".to_string(),
        builtin("bind", 2, |mut args, _| {
            let func = args.pop().unwrap();
            let effect = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let value = runtime.run_effect_value(effect.clone())?;
                    let applied = runtime.apply(func.clone(), value)?;
                    runtime.run_effect_value(applied)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    env.set(
        "attempt".to_string(),
        builtin("attempt", 1, |mut args, _| {
            let effect = args.remove(0);
            let effect = EffectValue::Thunk {
                func: Arc::new(
                    move |runtime| match runtime.run_effect_value(effect.clone()) {
                        Ok(value) => Ok(Value::Constructor {
                            name: "Ok".to_string(),
                            args: vec![value],
                        }),
                        Err(RuntimeError::Error(value)) => Ok(Value::Constructor {
                            name: "Err".to_string(),
                            args: vec![value],
                        }),
                        Err(err) => Err(err),
                    },
                ),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    env.set(
        "print".to_string(),
        builtin("print", 1, |mut args, _| {
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

    env.set(
        "println".to_string(),
        builtin("println", 1, |mut args, _| {
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

    env.set(
        "load".to_string(),
        builtin("load", 1, |mut args, _| {
            let value = args.remove(0);
            match value {
                Value::Effect(_) => Ok(value),
                _ => Err(RuntimeError::Message("load expects an Effect".to_string())),
            }
        }),
    );

    env.set("file".to_string(), build_file_record());
    env.set("clock".to_string(), build_clock_record());
    env.set("random".to_string(), build_random_record());
    env.set("channel".to_string(), build_channel_record());
    env.set("concurrent".to_string(), build_concurrent_record());
    env.set("httpServer".to_string(), build_http_server_record());
    env.set("text".to_string(), build_text_record());
    env.set("regex".to_string(), build_regex_record());
    env.set("math".to_string(), build_math_record());
    env.set("calendar".to_string(), build_calendar_record());
    env.set("color".to_string(), build_color_record());
    env.set("bigint".to_string(), build_bigint_record());
    env.set("rational".to_string(), build_rational_record());
    env.set("decimal".to_string(), build_decimal_record());
    env.set("url".to_string(), build_url_record());
    env.set("http".to_string(), build_http_client_record(HttpClientMode::Http));
    env.set("https".to_string(), build_http_client_record(HttpClientMode::Https));
    env.set("console".to_string(), build_console_record());
}

pub(super) fn builtin(
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
    })
}

fn builtin_constructor(name: &str, arity: usize) -> Value {
    let name_owned = name.to_string();
    builtin(name, arity, move |args, _| {
        Ok(Value::Constructor {
            name: name_owned.clone(),
            args,
        })
    })
}

fn build_file_record() -> Value {
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

fn build_clock_record() -> Value {
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

fn build_random_record() -> Value {
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

fn build_channel_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "make".to_string(),
        builtin("channel.make", 1, |_, _| {
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let (sender, receiver) = mpsc::channel();
                    let inner = Arc::new(ChannelInner {
                        sender: Mutex::new(Some(sender)),
                        receiver: Mutex::new(receiver),
                        closed: AtomicBool::new(false),
                    });
                    let send = Value::ChannelSend(Arc::new(ChannelSend {
                        inner: inner.clone(),
                    }));
                    let recv = Value::ChannelRecv(Arc::new(ChannelRecv { inner }));
                    Ok(Value::Tuple(vec![send, recv]))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "send".to_string(),
        builtin("channel.send", 2, |mut args, _| {
            let value = args.pop().unwrap();
            let sender = match args.pop().unwrap() {
                Value::ChannelSend(handle) => handle,
                _ => {
                    return Err(RuntimeError::Message(
                        "channel.send expects a send handle".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    if sender.inner.closed.load(Ordering::SeqCst) {
                        return Err(RuntimeError::Error(Value::Constructor {
                            name: "Closed".to_string(),
                            args: Vec::new(),
                        }));
                    }
                    let sender_guard = sender
                        .inner
                        .sender
                        .lock()
                        .map_err(|_| RuntimeError::Message("channel poisoned".to_string()))?;
                    if let Some(sender) = sender_guard.as_ref() {
                        sender.send(value.clone()).map_err(|_| {
                            RuntimeError::Error(Value::Constructor {
                                name: "Closed".to_string(),
                                args: Vec::new(),
                            })
                        })?;
                        Ok(Value::Unit)
                    } else {
                        Err(RuntimeError::Error(Value::Constructor {
                            name: "Closed".to_string(),
                            args: Vec::new(),
                        }))
                    }
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "recv".to_string(),
        builtin("channel.recv", 1, |mut args, _| {
            let receiver = match args.pop().unwrap() {
                Value::ChannelRecv(handle) => handle,
                _ => {
                    return Err(RuntimeError::Message(
                        "channel.recv expects a recv handle".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| loop {
                    runtime.check_cancelled()?;
                    let recv_guard = receiver
                        .inner
                        .receiver
                        .lock()
                        .map_err(|_| RuntimeError::Message("channel poisoned".to_string()))?;
                    match recv_guard.recv_timeout(Duration::from_millis(25)) {
                        Ok(value) => {
                            return Ok(Value::Constructor {
                                name: "Ok".to_string(),
                                args: vec![value],
                            });
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            return Ok(Value::Constructor {
                                name: "Err".to_string(),
                                args: vec![Value::Constructor {
                                    name: "Closed".to_string(),
                                    args: Vec::new(),
                                }],
                            })
                        }
                    }
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "close".to_string(),
        builtin("channel.close", 1, |mut args, _| {
            let sender = match args.pop().unwrap() {
                Value::ChannelSend(handle) => handle,
                _ => {
                    return Err(RuntimeError::Message(
                        "channel.close expects a send handle".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    sender.inner.closed.store(true, Ordering::SeqCst);
                    if let Ok(mut guard) = sender.inner.sender.lock() {
                        guard.take();
                    }
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(Arc::new(fields))
}

pub(super) fn build_concurrent_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "scope".to_string(),
        builtin("concurrent.scope", 1, |mut args, runtime| {
            let effect = args.pop().unwrap();
            let ctx = runtime.ctx.clone();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let cancel = CancelToken::child(runtime.cancel.clone());
                    let mut child = Runtime::new(ctx.clone(), cancel.clone());
                    let result = child.run_effect_value(effect.clone());
                    cancel.cancel();
                    result
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "par".to_string(),
        builtin("concurrent.par", 2, |mut args, runtime| {
            let right = args.pop().unwrap();
            let left = args.pop().unwrap();
            let ctx = runtime.ctx.clone();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let left_cancel = CancelToken::child(runtime.cancel.clone());
                    let right_cancel = CancelToken::child(runtime.cancel.clone());
                    let (tx, rx) = mpsc::channel();
                    spawn_effect(
                        0,
                        left.clone(),
                        ctx.clone(),
                        left_cancel.clone(),
                        tx.clone(),
                    );
                    spawn_effect(
                        1,
                        right.clone(),
                        ctx.clone(),
                        right_cancel.clone(),
                        tx.clone(),
                    );

                    let mut left_result = None;
                    let mut right_result = None;
                    let mut cancelled = false;
                    while left_result.is_none() || right_result.is_none() {
                        if runtime.check_cancelled().is_err() {
                            cancelled = true;
                            left_cancel.cancel();
                            right_cancel.cancel();
                        }
                        let (id, result) = match rx.recv_timeout(Duration::from_millis(25)) {
                            Ok(value) => value,
                            Err(mpsc::RecvTimeoutError::Timeout) => continue,
                            Err(mpsc::RecvTimeoutError::Disconnected) => {
                                return Err(RuntimeError::Message("worker stopped".to_string()))
                            }
                        };
                        if id == 0 {
                            if result.is_err() {
                                right_cancel.cancel();
                            }
                            left_result = Some(result);
                        } else {
                            if result.is_err() {
                                left_cancel.cancel();
                            }
                            right_result = Some(result);
                        }
                    }

                    if cancelled {
                        return Err(RuntimeError::Cancelled);
                    }

                    let left_result = left_result.unwrap();
                    let right_result = right_result.unwrap();
                    match (left_result, right_result) {
                        (Ok(left_value), Ok(right_value)) => {
                            Ok(Value::Tuple(vec![left_value, right_value]))
                        }
                        (Err(err), _) => Err(err),
                        (_, Err(err)) => Err(err),
                    }
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "race".to_string(),
        builtin("concurrent.race", 2, |mut args, runtime| {
            let right = args.pop().unwrap();
            let left = args.pop().unwrap();
            let ctx = runtime.ctx.clone();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let left_cancel = CancelToken::child(runtime.cancel.clone());
                    let right_cancel = CancelToken::child(runtime.cancel.clone());
                    let (tx, rx) = mpsc::channel();
                    spawn_effect(
                        0,
                        left.clone(),
                        ctx.clone(),
                        left_cancel.clone(),
                        tx.clone(),
                    );
                    spawn_effect(
                        1,
                        right.clone(),
                        ctx.clone(),
                        right_cancel.clone(),
                        tx.clone(),
                    );

                    let mut cancelled = false;
                    let (winner, result) = loop {
                        if runtime.check_cancelled().is_err() {
                            cancelled = true;
                            left_cancel.cancel();
                            right_cancel.cancel();
                        }
                        match rx.recv_timeout(Duration::from_millis(25)) {
                            Ok(value) => break value,
                            Err(mpsc::RecvTimeoutError::Timeout) => continue,
                            Err(mpsc::RecvTimeoutError::Disconnected) => {
                                return Err(RuntimeError::Message("worker stopped".to_string()))
                            }
                        }
                    };
                    if winner == 0 {
                        right_cancel.cancel();
                    } else {
                        left_cancel.cancel();
                    }
                    while rx.recv_timeout(Duration::from_millis(25)).is_err() {
                        if runtime.check_cancelled().is_err() {
                            cancelled = true;
                            left_cancel.cancel();
                            right_cancel.cancel();
                        }
                    }
                    if cancelled {
                        return Err(RuntimeError::Cancelled);
                    }
                    result
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "spawnDetached".to_string(),
        builtin("concurrent.spawnDetached", 1, |mut args, runtime| {
            let effect_value = args.pop().unwrap();
            let ctx = runtime.ctx.clone();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let parent = runtime
                        .cancel
                        .parent()
                        .unwrap_or_else(|| runtime.cancel.clone());
                    let cancel = CancelToken::child(parent);
                    let (tx, _rx) = mpsc::channel();
                    spawn_effect(0, effect_value.clone(), ctx.clone(), cancel, tx);
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn make_some(value: Value) -> Value {
    Value::Constructor {
        name: "Some".to_string(),
        args: vec![value],
    }
}

fn make_none() -> Value {
    Value::Constructor {
        name: "None".to_string(),
        args: Vec::new(),
    }
}

fn make_ok(value: Value) -> Value {
    Value::Constructor {
        name: "Ok".to_string(),
        args: vec![value],
    }
}

fn make_err(value: Value) -> Value {
    Value::Constructor {
        name: "Err".to_string(),
        args: vec![value],
    }
}

fn list_value(items: Vec<Value>) -> Value {
    Value::List(Arc::new(items))
}

fn expect_text(value: Value, ctx: &str) -> Result<String, RuntimeError> {
    match value {
        Value::Text(text) => Ok(text),
        _ => Err(RuntimeError::Message(format!("{ctx} expects Text"))),
    }
}

fn expect_int(value: Value, ctx: &str) -> Result<i64, RuntimeError> {
    match value {
        Value::Int(value) => Ok(value),
        _ => Err(RuntimeError::Message(format!("{ctx} expects Int"))),
    }
}

fn expect_float(value: Value, ctx: &str) -> Result<f64, RuntimeError> {
    match value {
        Value::Float(value) => Ok(value),
        _ => Err(RuntimeError::Message(format!("{ctx} expects Float"))),
    }
}

fn expect_char(value: Value, ctx: &str) -> Result<char, RuntimeError> {
    let text = expect_text(value, ctx)?;
    let mut chars = text.chars();
    let first = chars.next();
    if first.is_some() && chars.next().is_none() {
        Ok(first.unwrap())
    } else {
        Err(RuntimeError::Message(format!("{ctx} expects Char")))
    }
}

fn expect_list(value: Value, ctx: &str) -> Result<Arc<Vec<Value>>, RuntimeError> {
    match value {
        Value::List(items) => Ok(items),
        _ => Err(RuntimeError::Message(format!("{ctx} expects List"))),
    }
}

fn expect_record(
    value: Value,
    ctx: &str,
) -> Result<Arc<HashMap<String, Value>>, RuntimeError> {
    match value {
        Value::Record(fields) => Ok(fields),
        _ => Err(RuntimeError::Message(format!("{ctx} expects record"))),
    }
}

fn list_floats(values: &[Value], ctx: &str) -> Result<Vec<f64>, RuntimeError> {
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        match value {
            Value::Float(value) => out.push(*value),
            _ => return Err(RuntimeError::Message(format!("{ctx} expects List Float"))),
        }
    }
    Ok(out)
}

fn list_ints(values: &[Value], ctx: &str) -> Result<Vec<i64>, RuntimeError> {
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        match value {
            Value::Int(value) => out.push(*value),
            _ => return Err(RuntimeError::Message(format!("{ctx} expects List Int"))),
        }
    }
    Ok(out)
}

fn expect_bytes(value: Value, ctx: &str) -> Result<Arc<Vec<u8>>, RuntimeError> {
    match value {
        Value::Bytes(bytes) => Ok(bytes),
        _ => Err(RuntimeError::Message(format!("{ctx} expects Bytes"))),
    }
}

fn expect_regex(value: Value, ctx: &str) -> Result<Arc<Regex>, RuntimeError> {
    match value {
        Value::Regex(regex) => Ok(regex),
        _ => Err(RuntimeError::Message(format!("{ctx} expects Regex"))),
    }
}

fn expect_bigint(value: Value, ctx: &str) -> Result<Arc<BigInt>, RuntimeError> {
    match value {
        Value::BigInt(value) => Ok(value),
        _ => Err(RuntimeError::Message(format!("{ctx} expects BigInt"))),
    }
}

fn expect_rational(value: Value, ctx: &str) -> Result<Arc<BigRational>, RuntimeError> {
    match value {
        Value::Rational(value) => Ok(value),
        _ => Err(RuntimeError::Message(format!("{ctx} expects Rational"))),
    }
}

fn expect_decimal(value: Value, ctx: &str) -> Result<Decimal, RuntimeError> {
    match value {
        Value::Decimal(value) => Ok(value),
        _ => Err(RuntimeError::Message(format!("{ctx} expects Decimal"))),
    }
}

fn char_len(text: &str) -> usize {
    text.chars().count()
}

fn take_chars(text: &str, count: usize) -> String {
    text.chars().take(count).collect()
}

fn slice_chars(text: &str, start: i64, end: i64) -> String {
    let len = char_len(text) as i64;
    let start = start.max(0).min(len);
    let end = end.max(start).min(len);
    text.chars()
        .skip(start as usize)
        .take((end - start) as usize)
        .collect()
}

fn pad_text(text: &str, width: i64, fill: &str, left: bool) -> String {
    let width = if width < 0 { 0 } else { width as usize };
    let len = char_len(text);
    if width <= len || fill.is_empty() {
        return text.to_string();
    }
    let needed = width - len;
    let mut pad = String::new();
    while char_len(&pad) < needed {
        pad.push_str(fill);
    }
    let pad = take_chars(&pad, needed);
    if left {
        format!("{pad}{text}")
    } else {
        format!("{text}{pad}")
    }
}

fn capitalize_segment(segment: &str) -> String {
    let mut graphemes = UnicodeSegmentation::graphemes(segment, true);
    let first = match graphemes.next() {
        Some(value) => value,
        None => return String::new(),
    };
    let rest: String = graphemes.collect();
    let mut out = String::new();
    out.push_str(&first.to_uppercase());
    out.push_str(&rest.to_lowercase());
    out
}

#[derive(Clone, Copy)]
enum EncodingKind {
    Utf8,
    Utf16,
    Utf32,
    Latin1,
}

fn encoding_kind(value: &Value) -> Option<EncodingKind> {
    match value {
        Value::Constructor { name, args } if args.is_empty() => match name.as_str() {
            "Utf8" => Some(EncodingKind::Utf8),
            "Utf16" => Some(EncodingKind::Utf16),
            "Utf32" => Some(EncodingKind::Utf32),
            "Latin1" => Some(EncodingKind::Latin1),
            _ => None,
        },
        _ => None,
    }
}

fn encode_text(encoding: EncodingKind, text: &str) -> Vec<u8> {
    match encoding {
        EncodingKind::Utf8 => text.as_bytes().to_vec(),
        EncodingKind::Latin1 => text
            .chars()
            .map(|ch| if (ch as u32) <= 0xFF { ch as u8 } else { b'?' })
            .collect(),
        EncodingKind::Utf16 => text
            .encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect(),
        EncodingKind::Utf32 => text
            .chars()
            .flat_map(|ch| (ch as u32).to_le_bytes())
            .collect(),
    }
}

fn decode_bytes(encoding: EncodingKind, bytes: &[u8]) -> Result<String, ()> {
    match encoding {
        EncodingKind::Utf8 => String::from_utf8(bytes.to_vec()).map_err(|_| ()),
        EncodingKind::Latin1 => Ok(bytes.iter().map(|b| char::from(*b)).collect()),
        EncodingKind::Utf16 => {
            if bytes.len() % 2 != 0 {
                return Err(());
            }
            let units = bytes
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>();
            String::from_utf16(&units).map_err(|_| ())
        }
        EncodingKind::Utf32 => {
            if bytes.len() % 4 != 0 {
                return Err(());
            }
            let mut out = String::new();
            for chunk in bytes.chunks_exact(4) {
                let value = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                let ch = char::from_u32(value).ok_or(())?;
                out.push(ch);
            }
            Ok(out)
        }
    }
}

fn build_text_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "length".to_string(),
        builtin("text.length", 1, |mut args, _| {
            let text = expect_text(args.remove(0), "text.length")?;
            Ok(Value::Int(char_len(&text) as i64))
        }),
    );
    fields.insert(
        "isEmpty".to_string(),
        builtin("text.isEmpty", 1, |mut args, _| {
            let text = expect_text(args.remove(0), "text.isEmpty")?;
            Ok(Value::Bool(text.is_empty()))
        }),
    );
    fields.insert(
        "isDigit".to_string(),
        builtin("text.isDigit", 1, |mut args, _| {
            let ch = expect_char(args.remove(0), "text.isDigit")?;
            Ok(Value::Bool(ch.is_numeric()))
        }),
    );
    fields.insert(
        "isAlpha".to_string(),
        builtin("text.isAlpha", 1, |mut args, _| {
            let ch = expect_char(args.remove(0), "text.isAlpha")?;
            Ok(Value::Bool(ch.is_alphabetic()))
        }),
    );
    fields.insert(
        "isAlnum".to_string(),
        builtin("text.isAlnum", 1, |mut args, _| {
            let ch = expect_char(args.remove(0), "text.isAlnum")?;
            Ok(Value::Bool(ch.is_alphanumeric()))
        }),
    );
    fields.insert(
        "isSpace".to_string(),
        builtin("text.isSpace", 1, |mut args, _| {
            let ch = expect_char(args.remove(0), "text.isSpace")?;
            Ok(Value::Bool(ch.is_whitespace()))
        }),
    );
    fields.insert(
        "isUpper".to_string(),
        builtin("text.isUpper", 1, |mut args, _| {
            let ch = expect_char(args.remove(0), "text.isUpper")?;
            Ok(Value::Bool(ch.is_uppercase()))
        }),
    );
    fields.insert(
        "isLower".to_string(),
        builtin("text.isLower", 1, |mut args, _| {
            let ch = expect_char(args.remove(0), "text.isLower")?;
            Ok(Value::Bool(ch.is_lowercase()))
        }),
    );
    fields.insert(
        "contains".to_string(),
        builtin("text.contains", 2, |mut args, _| {
            let needle = expect_text(args.pop().unwrap(), "text.contains")?;
            let haystack = expect_text(args.pop().unwrap(), "text.contains")?;
            Ok(Value::Bool(haystack.contains(&needle)))
        }),
    );
    fields.insert(
        "startsWith".to_string(),
        builtin("text.startsWith", 2, |mut args, _| {
            let prefix = expect_text(args.pop().unwrap(), "text.startsWith")?;
            let text = expect_text(args.pop().unwrap(), "text.startsWith")?;
            Ok(Value::Bool(text.starts_with(&prefix)))
        }),
    );
    fields.insert(
        "endsWith".to_string(),
        builtin("text.endsWith", 2, |mut args, _| {
            let suffix = expect_text(args.pop().unwrap(), "text.endsWith")?;
            let text = expect_text(args.pop().unwrap(), "text.endsWith")?;
            Ok(Value::Bool(text.ends_with(&suffix)))
        }),
    );
    fields.insert(
        "indexOf".to_string(),
        builtin("text.indexOf", 2, |mut args, _| {
            let needle = expect_text(args.pop().unwrap(), "text.indexOf")?;
            let haystack = expect_text(args.pop().unwrap(), "text.indexOf")?;
            match haystack.find(&needle) {
                Some(idx) => Ok(make_some(Value::Int(
                    haystack[..idx].chars().count() as i64,
                ))),
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "lastIndexOf".to_string(),
        builtin("text.lastIndexOf", 2, |mut args, _| {
            let needle = expect_text(args.pop().unwrap(), "text.lastIndexOf")?;
            let haystack = expect_text(args.pop().unwrap(), "text.lastIndexOf")?;
            match haystack.rfind(&needle) {
                Some(idx) => Ok(make_some(Value::Int(
                    haystack[..idx].chars().count() as i64,
                ))),
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "count".to_string(),
        builtin("text.count", 2, |mut args, _| {
            let needle = expect_text(args.pop().unwrap(), "text.count")?;
            let haystack = expect_text(args.pop().unwrap(), "text.count")?;
            Ok(Value::Int(haystack.matches(&needle).count() as i64))
        }),
    );
    fields.insert(
        "compare".to_string(),
        builtin("text.compare", 2, |mut args, _| {
            let right = expect_text(args.pop().unwrap(), "text.compare")?;
            let left = expect_text(args.pop().unwrap(), "text.compare")?;
            let ord = left.cmp(&right);
            let value = match ord {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            };
            Ok(Value::Int(value))
        }),
    );
    fields.insert(
        "slice".to_string(),
        builtin("text.slice", 3, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.slice")?;
            let end = expect_int(args.pop().unwrap(), "text.slice")?;
            let start = expect_int(args.pop().unwrap(), "text.slice")?;
            Ok(Value::Text(slice_chars(&text, start, end)))
        }),
    );
    fields.insert(
        "split".to_string(),
        builtin("text.split", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.split")?;
            let sep = expect_text(args.pop().unwrap(), "text.split")?;
            let parts = text
                .split(&sep)
                .map(|part| Value::Text(part.to_string()))
                .collect::<Vec<_>>();
            Ok(list_value(parts))
        }),
    );
    fields.insert(
        "splitLines".to_string(),
        builtin("text.splitLines", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.splitLines")?;
            let parts = text
                .lines()
                .map(|part| Value::Text(part.to_string()))
                .collect::<Vec<_>>();
            Ok(list_value(parts))
        }),
    );
    fields.insert(
        "chunk".to_string(),
        builtin("text.chunk", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.chunk")?;
            let size = expect_int(args.pop().unwrap(), "text.chunk")?;
            if size <= 0 {
                return Ok(list_value(Vec::new()));
            }
            let size = size as usize;
            let mut items = Vec::new();
            let mut iter = text.chars().peekable();
            while iter.peek().is_some() {
                let chunk: String = iter.by_ref().take(size).collect();
                items.push(Value::Text(chunk));
            }
            Ok(list_value(items))
        }),
    );
    fields.insert(
        "trim".to_string(),
        builtin("text.trim", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.trim")?;
            Ok(Value::Text(text.trim().to_string()))
        }),
    );
    fields.insert(
        "trimStart".to_string(),
        builtin("text.trimStart", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.trimStart")?;
            Ok(Value::Text(text.trim_start().to_string()))
        }),
    );
    fields.insert(
        "trimEnd".to_string(),
        builtin("text.trimEnd", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.trimEnd")?;
            Ok(Value::Text(text.trim_end().to_string()))
        }),
    );
    fields.insert(
        "padStart".to_string(),
        builtin("text.padStart", 3, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.padStart")?;
            let fill = expect_text(args.pop().unwrap(), "text.padStart")?;
            let width = expect_int(args.pop().unwrap(), "text.padStart")?;
            Ok(Value::Text(pad_text(&text, width, &fill, true)))
        }),
    );
    fields.insert(
        "padEnd".to_string(),
        builtin("text.padEnd", 3, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.padEnd")?;
            let fill = expect_text(args.pop().unwrap(), "text.padEnd")?;
            let width = expect_int(args.pop().unwrap(), "text.padEnd")?;
            Ok(Value::Text(pad_text(&text, width, &fill, false)))
        }),
    );
    fields.insert(
        "replace".to_string(),
        builtin("text.replace", 3, |mut args, _| {
            let replacement = expect_text(args.pop().unwrap(), "text.replace")?;
            let needle = expect_text(args.pop().unwrap(), "text.replace")?;
            let text = expect_text(args.pop().unwrap(), "text.replace")?;
            Ok(Value::Text(text.replacen(&needle, &replacement, 1)))
        }),
    );
    fields.insert(
        "replaceAll".to_string(),
        builtin("text.replaceAll", 3, |mut args, _| {
            let replacement = expect_text(args.pop().unwrap(), "text.replaceAll")?;
            let needle = expect_text(args.pop().unwrap(), "text.replaceAll")?;
            let text = expect_text(args.pop().unwrap(), "text.replaceAll")?;
            Ok(Value::Text(text.replace(&needle, &replacement)))
        }),
    );
    fields.insert(
        "remove".to_string(),
        builtin("text.remove", 2, |mut args, _| {
            let needle = expect_text(args.pop().unwrap(), "text.remove")?;
            let text = expect_text(args.pop().unwrap(), "text.remove")?;
            Ok(Value::Text(text.replace(&needle, "")))
        }),
    );
    fields.insert(
        "repeat".to_string(),
        builtin("text.repeat", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.repeat")?;
            let count = expect_int(args.pop().unwrap(), "text.repeat")?;
            let count = if count < 0 { 0 } else { count as usize };
            Ok(Value::Text(text.repeat(count)))
        }),
    );
    fields.insert(
        "reverse".to_string(),
        builtin("text.reverse", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.reverse")?;
            let reversed = UnicodeSegmentation::graphemes(text.as_str(), true)
                .rev()
                .collect::<String>();
            Ok(Value::Text(reversed))
        }),
    );
    fields.insert(
        "concat".to_string(),
        builtin("text.concat", 1, |mut args, _| {
            let list = expect_list(args.pop().unwrap(), "text.concat")?;
            let mut out = String::new();
            for item in list.iter() {
                match item {
                    Value::Text(text) => out.push_str(text),
                    _ => {
                        return Err(RuntimeError::Message(
                            "text.concat expects List Text".to_string(),
                        ))
                    }
                }
            }
            Ok(Value::Text(out))
        }),
    );
    fields.insert(
        "toLower".to_string(),
        builtin("text.toLower", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.toLower")?;
            Ok(Value::Text(text.to_lowercase()))
        }),
    );
    fields.insert(
        "toUpper".to_string(),
        builtin("text.toUpper", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.toUpper")?;
            Ok(Value::Text(text.to_uppercase()))
        }),
    );
    fields.insert(
        "capitalize".to_string(),
        builtin("text.capitalize", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.capitalize")?;
            Ok(Value::Text(capitalize_segment(&text)))
        }),
    );
    fields.insert(
        "titleCase".to_string(),
        builtin("text.titleCase", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.titleCase")?;
            let mut out = String::new();
            for segment in UnicodeSegmentation::split_word_bounds(text.as_str()) {
                if segment.chars().any(|ch| ch.is_alphabetic()) {
                    out.push_str(&capitalize_segment(segment));
                } else {
                    out.push_str(segment);
                }
            }
            Ok(Value::Text(out))
        }),
    );
    fields.insert(
        "caseFold".to_string(),
        builtin("text.caseFold", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.caseFold")?;
            Ok(Value::Text(text.to_lowercase()))
        }),
    );
    fields.insert(
        "normalizeNFC".to_string(),
        builtin("text.normalizeNFC", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.normalizeNFC")?;
            Ok(Value::Text(text.nfc().collect()))
        }),
    );
    fields.insert(
        "normalizeNFD".to_string(),
        builtin("text.normalizeNFD", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.normalizeNFD")?;
            Ok(Value::Text(text.nfd().collect()))
        }),
    );
    fields.insert(
        "normalizeNFKC".to_string(),
        builtin("text.normalizeNFKC", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.normalizeNFKC")?;
            Ok(Value::Text(text.nfkc().collect()))
        }),
    );
    fields.insert(
        "normalizeNFKD".to_string(),
        builtin("text.normalizeNFKD", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.normalizeNFKD")?;
            Ok(Value::Text(text.nfkd().collect()))
        }),
    );
    fields.insert(
        "toBytes".to_string(),
        builtin("text.toBytes", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.toBytes")?;
            let encoding_value = args.pop().unwrap();
            let encoding = encoding_kind(&encoding_value).ok_or_else(|| {
                RuntimeError::Message("text.toBytes expects Encoding".to_string())
            })?;
            Ok(Value::Bytes(Arc::new(encode_text(encoding, &text))))
        }),
    );
    fields.insert(
        "fromBytes".to_string(),
        builtin("text.fromBytes", 2, |mut args, _| {
            let bytes = expect_bytes(args.pop().unwrap(), "text.fromBytes")?;
            let encoding_value = args.pop().unwrap();
            let encoding = encoding_kind(&encoding_value).ok_or_else(|| {
                RuntimeError::Message("text.fromBytes expects Encoding".to_string())
            })?;
            match decode_bytes(encoding, &bytes) {
                Ok(text) => Ok(make_ok(Value::Text(text))),
                Err(()) => Ok(make_err(Value::Constructor {
                    name: "InvalidEncoding".to_string(),
                    args: vec![encoding_value],
                })),
            }
        }),
    );
    fields.insert(
        "toText".to_string(),
        builtin("text.toText", 1, |mut args, _| {
            let value = args.pop().unwrap();
            Ok(Value::Text(format_value(&value)))
        }),
    );
    fields.insert(
        "parseInt".to_string(),
        builtin("text.parseInt", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.parseInt")?;
            match text.trim().parse::<i64>() {
                Ok(value) => Ok(make_some(Value::Int(value))),
                Err(_) => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "parseFloat".to_string(),
        builtin("text.parseFloat", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.parseFloat")?;
            match text.trim().parse::<f64>() {
                Ok(value) => Ok(make_some(Value::Float(value))),
                Err(_) => Ok(make_none()),
            }
        }),
    );
    Value::Record(Arc::new(fields))
}

fn build_regex_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "compile".to_string(),
        builtin("regex.compile", 1, |mut args, _| {
            let pattern = expect_text(args.pop().unwrap(), "regex.compile")?;
            match Regex::new(&pattern) {
                Ok(regex) => Ok(make_ok(Value::Regex(Arc::new(regex)))),
                Err(err) => Ok(make_err(Value::Constructor {
                    name: "InvalidPattern".to_string(),
                    args: vec![Value::Text(err.to_string())],
                })),
            }
        }),
    );
    fields.insert(
        "test".to_string(),
        builtin("regex.test", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "regex.test")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.test")?;
            Ok(Value::Bool(regex.is_match(&text)))
        }),
    );
    fields.insert(
        "match".to_string(),
        builtin("regex.match", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "regex.match")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.match")?;
            match regex.captures(&text) {
                Some(captures) => {
                    let full = captures.get(0).map(|m| m.as_str()).unwrap_or("");
                    let mut groups = Vec::new();
                    for idx in 1..captures.len() {
                        if let Some(matched) = captures.get(idx) {
                            groups.push(make_some(Value::Text(matched.as_str().to_string())));
                        } else {
                            groups.push(make_none());
                        }
                    }
                    let mut record = HashMap::new();
                    let (start, end) = captures.get(0).map(|m| (m.start(), m.end())).unwrap_or((0, 0));
                    record.insert("full".to_string(), Value::Text(full.to_string()));
                    record.insert("groups".to_string(), list_value(groups));
                    record.insert("start".to_string(), Value::Int(start as i64));
                    record.insert("end".to_string(), Value::Int(end as i64));
                    Ok(make_some(Value::Record(Arc::new(record))))
                }
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "matches".to_string(),
        builtin("regex.matches", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "regex.matches")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.matches")?;
            let mut matches_out = Vec::new();
            for captures in regex.captures_iter(&text) {
                let full = captures.get(0).map(|m| m.as_str()).unwrap_or("");
                let mut groups = Vec::new();
                for idx in 1..captures.len() {
                    if let Some(matched) = captures.get(idx) {
                        groups.push(make_some(Value::Text(matched.as_str().to_string())));
                    } else {
                        groups.push(make_none());
                    }
                }
                let (start, end) = captures.get(0).map(|m| (m.start(), m.end())).unwrap_or((0, 0));
                let mut record = HashMap::new();
                record.insert("full".to_string(), Value::Text(full.to_string()));
                record.insert("groups".to_string(), list_value(groups));
                record.insert("start".to_string(), Value::Int(start as i64));
                record.insert("end".to_string(), Value::Int(end as i64));
                matches_out.push(Value::Record(Arc::new(record)));
            }
            Ok(list_value(matches_out))
        }),
    );
    fields.insert(
        "find".to_string(),
        builtin("regex.find", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "regex.find")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.find")?;
            match regex.find(&text) {
                Some(found) => Ok(make_some(Value::Tuple(vec![
                    Value::Int(found.start() as i64),
                    Value::Int(found.end() as i64),
                ]))),
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "findAll".to_string(),
        builtin("regex.findAll", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "regex.findAll")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.findAll")?;
            let mut out = Vec::new();
            for found in regex.find_iter(&text) {
                out.push(Value::Tuple(vec![
                    Value::Int(found.start() as i64),
                    Value::Int(found.end() as i64),
                ]));
            }
            Ok(list_value(out))
        }),
    );
    fields.insert(
        "split".to_string(),
        builtin("regex.split", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "regex.split")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.split")?;
            let parts = regex
                .split(&text)
                .map(|part| Value::Text(part.to_string()))
                .collect::<Vec<_>>();
            Ok(list_value(parts))
        }),
    );
    fields.insert(
        "replace".to_string(),
        builtin("regex.replace", 3, |mut args, _| {
            let replacement = expect_text(args.pop().unwrap(), "regex.replace")?;
            let text = expect_text(args.pop().unwrap(), "regex.replace")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.replace")?;
            Ok(Value::Text(regex.replacen(&text, 1, replacement).to_string()))
        }),
    );
    fields.insert(
        "replaceAll".to_string(),
        builtin("regex.replaceAll", 3, |mut args, _| {
            let replacement = expect_text(args.pop().unwrap(), "regex.replaceAll")?;
            let text = expect_text(args.pop().unwrap(), "regex.replaceAll")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.replaceAll")?;
            Ok(Value::Text(regex.replace_all(&text, replacement).to_string()))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn angle_from_value(value: Value, ctx: &str) -> Result<f64, RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::Message(format!("{ctx} expects Angle")));
    };
    let radians = fields
        .get("radians")
        .cloned()
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Angle.radians")))?;
    expect_float(radians, ctx)
}

fn angle_value(radians: f64) -> Value {
    let mut map = HashMap::new();
    map.insert("radians".to_string(), Value::Float(radians));
    Value::Record(Arc::new(map))
}

fn gcd_i64(mut a: i64, mut b: i64) -> i64 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

fn lcm_i64(a: i64, b: i64) -> i64 {
    if a == 0 || b == 0 {
        return 0;
    }
    (a / gcd_i64(a, b)) * b
}

fn mod_pow(mut base: i64, mut exp: i64, modulus: i64) -> i64 {
    if modulus == 1 {
        return 0;
    }
    let mut result: i64 = 1 % modulus;
    base %= modulus;
    while exp > 0 {
        if exp % 2 == 1 {
            result = (result * base) % modulus;
        }
        exp /= 2;
        base = (base * base) % modulus;
    }
    result
}

fn factorial_bigint(n: i64) -> Option<BigInt> {
    if n < 0 {
        return None;
    }
    let mut acc = BigInt::from(1);
    for i in 2..=n {
        acc *= i;
    }
    Some(acc)
}

fn comb_bigint(n: i64, k: i64) -> Option<BigInt> {
    if n < 0 || k < 0 || k > n {
        return None;
    }
    let k = std::cmp::min(k, n - k);
    let mut result = BigInt::from(1);
    for i in 0..k {
        result *= n - i;
        result /= i + 1;
    }
    Some(result)
}

fn perm_bigint(n: i64, k: i64) -> Option<BigInt> {
    if n < 0 || k < 0 || k > n {
        return None;
    }
    let mut result = BigInt::from(1);
    for i in 0..k {
        result *= n - i;
    }
    Some(result)
}

fn next_after(from: f64, to: f64) -> f64 {
    if from.is_nan() || to.is_nan() {
        return f64::NAN;
    }
    if from == to {
        return to;
    }
    if from == 0.0 {
        let tiny = f64::from_bits(1);
        return if to > 0.0 { tiny } else { -tiny };
    }
    let mut bits = from.to_bits();
    if (from < to) == (from > 0.0) {
        bits = bits.wrapping_add(1);
    } else {
        bits = bits.wrapping_sub(1);
    }
    f64::from_bits(bits)
}

fn frexp_value(value: f64) -> (f64, i64) {
    if value == 0.0 || value.is_nan() || value.is_infinite() {
        return (value, 0);
    }
    let exp = value.abs().log2().floor() as i64 + 1;
    let mantissa = value / 2.0_f64.powi(exp as i32);
    (mantissa, exp)
}

fn build_math_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert("pi".to_string(), Value::Float(std::f64::consts::PI));
    fields.insert("tau".to_string(), Value::Float(std::f64::consts::TAU));
    fields.insert("e".to_string(), Value::Float(std::f64::consts::E));
    fields.insert("inf".to_string(), Value::Float(f64::INFINITY));
    fields.insert("nan".to_string(), Value::Float(f64::NAN));
    fields.insert(
        "phi".to_string(),
        Value::Float((1.0 + 5.0_f64.sqrt()) / 2.0),
    );
    fields.insert("sqrt2".to_string(), Value::Float(std::f64::consts::SQRT_2));
    fields.insert("ln2".to_string(), Value::Float(std::f64::consts::LN_2));
    fields.insert("ln10".to_string(), Value::Float(std::f64::consts::LN_10));
    fields.insert(
        "abs".to_string(),
        builtin("math.abs", 1, |mut args, _| {
            let value = args.pop().unwrap();
            match value {
                Value::Int(value) => Ok(Value::Int(value.wrapping_abs())),
                Value::Float(value) => Ok(Value::Float(value.abs())),
                _ => Err(RuntimeError::Message("math.abs expects Int or Float".to_string())),
            }
        }),
    );
    fields.insert(
        "sign".to_string(),
        builtin("math.sign", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.sign")?;
            let out = if value > 0.0 {
                1.0
            } else if value < 0.0 {
                -1.0
            } else {
                0.0
            };
            Ok(Value::Float(out))
        }),
    );
    fields.insert(
        "copysign".to_string(),
        builtin("math.copysign", 2, |mut args, _| {
            let sign = expect_float(args.pop().unwrap(), "math.copysign")?;
            let mag = expect_float(args.pop().unwrap(), "math.copysign")?;
            Ok(Value::Float(mag.copysign(sign)))
        }),
    );
    fields.insert(
        "min".to_string(),
        builtin("math.min", 2, |mut args, _| {
            let right = expect_float(args.pop().unwrap(), "math.min")?;
            let left = expect_float(args.pop().unwrap(), "math.min")?;
            Ok(Value::Float(left.min(right)))
        }),
    );
    fields.insert(
        "max".to_string(),
        builtin("math.max", 2, |mut args, _| {
            let right = expect_float(args.pop().unwrap(), "math.max")?;
            let left = expect_float(args.pop().unwrap(), "math.max")?;
            Ok(Value::Float(left.max(right)))
        }),
    );
    fields.insert(
        "minAll".to_string(),
        builtin("math.minAll", 1, |mut args, _| {
            let list = expect_list(args.pop().unwrap(), "math.minAll")?;
            let values = list_floats(&list, "math.minAll")?;
            if values.is_empty() {
                return Ok(make_none());
            }
            let mut min = values[0];
            for value in values.iter().skip(1) {
                min = min.min(*value);
            }
            Ok(make_some(Value::Float(min)))
        }),
    );
    fields.insert(
        "maxAll".to_string(),
        builtin("math.maxAll", 1, |mut args, _| {
            let list = expect_list(args.pop().unwrap(), "math.maxAll")?;
            let values = list_floats(&list, "math.maxAll")?;
            if values.is_empty() {
                return Ok(make_none());
            }
            let mut max = values[0];
            for value in values.iter().skip(1) {
                max = max.max(*value);
            }
            Ok(make_some(Value::Float(max)))
        }),
    );
    fields.insert(
        "clamp".to_string(),
        builtin("math.clamp", 3, |mut args, _| {
            let x = expect_float(args.pop().unwrap(), "math.clamp")?;
            let high = expect_float(args.pop().unwrap(), "math.clamp")?;
            let low = expect_float(args.pop().unwrap(), "math.clamp")?;
            Ok(Value::Float(x.max(low).min(high)))
        }),
    );
    fields.insert(
        "sum".to_string(),
        builtin("math.sum", 1, |mut args, _| {
            let list = expect_list(args.pop().unwrap(), "math.sum")?;
            let values = list_floats(&list, "math.sum")?;
            Ok(Value::Float(values.into_iter().sum()))
        }),
    );
    fields.insert(
        "sumInt".to_string(),
        builtin("math.sumInt", 1, |mut args, _| {
            let list = expect_list(args.pop().unwrap(), "math.sumInt")?;
            let values = list_ints(&list, "math.sumInt")?;
            Ok(Value::Int(values.into_iter().sum()))
        }),
    );
    fields.insert(
        "floor".to_string(),
        builtin("math.floor", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.floor")?;
            Ok(Value::Float(value.floor()))
        }),
    );
    fields.insert(
        "ceil".to_string(),
        builtin("math.ceil", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.ceil")?;
            Ok(Value::Float(value.ceil()))
        }),
    );
    fields.insert(
        "trunc".to_string(),
        builtin("math.trunc", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.trunc")?;
            Ok(Value::Float(value.trunc()))
        }),
    );
    fields.insert(
        "round".to_string(),
        builtin("math.round", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.round")?;
            let trunc = value.trunc();
            let frac = value - trunc;
            let rounded = if frac.abs() == 0.5 {
                let even = (trunc as i64) % 2 == 0;
                if even {
                    trunc
                } else {
                    trunc + value.signum()
                }
            } else {
                value.round()
            };
            Ok(Value::Float(rounded))
        }),
    );
    fields.insert(
        "fract".to_string(),
        builtin("math.fract", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.fract")?;
            Ok(Value::Float(value.fract()))
        }),
    );
    fields.insert(
        "modf".to_string(),
        builtin("math.modf", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.modf")?;
            let int_part = value.trunc();
            let frac_part = value.fract();
            Ok(Value::Tuple(vec![Value::Float(int_part), Value::Float(frac_part)]))
        }),
    );
    fields.insert(
        "frexp".to_string(),
        builtin("math.frexp", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.frexp")?;
            let (mantissa, exponent) = frexp_value(value);
            Ok(Value::Tuple(vec![
                Value::Float(mantissa),
                Value::Int(exponent),
            ]))
        }),
    );
    fields.insert(
        "ldexp".to_string(),
        builtin("math.ldexp", 2, |mut args, _| {
            let exponent = expect_int(args.pop().unwrap(), "math.ldexp")?;
            let mantissa = expect_float(args.pop().unwrap(), "math.ldexp")?;
            Ok(Value::Float(mantissa * 2.0_f64.powi(exponent as i32)))
        }),
    );
    fields.insert(
        "pow".to_string(),
        builtin("math.pow", 2, |mut args, _| {
            let exp = expect_float(args.pop().unwrap(), "math.pow")?;
            let base = expect_float(args.pop().unwrap(), "math.pow")?;
            Ok(Value::Float(base.powf(exp)))
        }),
    );
    fields.insert(
        "sqrt".to_string(),
        builtin("math.sqrt", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.sqrt")?;
            Ok(Value::Float(value.sqrt()))
        }),
    );
    fields.insert(
        "cbrt".to_string(),
        builtin("math.cbrt", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.cbrt")?;
            Ok(Value::Float(value.cbrt()))
        }),
    );
    fields.insert(
        "hypot".to_string(),
        builtin("math.hypot", 2, |mut args, _| {
            let y = expect_float(args.pop().unwrap(), "math.hypot")?;
            let x = expect_float(args.pop().unwrap(), "math.hypot")?;
            Ok(Value::Float(x.hypot(y)))
        }),
    );
    fields.insert(
        "exp".to_string(),
        builtin("math.exp", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.exp")?;
            Ok(Value::Float(value.exp()))
        }),
    );
    fields.insert(
        "exp2".to_string(),
        builtin("math.exp2", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.exp2")?;
            Ok(Value::Float(value.exp2()))
        }),
    );
    fields.insert(
        "expm1".to_string(),
        builtin("math.expm1", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.expm1")?;
            Ok(Value::Float(value.exp_m1()))
        }),
    );
    fields.insert(
        "log".to_string(),
        builtin("math.log", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.log")?;
            Ok(Value::Float(value.ln()))
        }),
    );
    fields.insert(
        "log10".to_string(),
        builtin("math.log10", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.log10")?;
            Ok(Value::Float(value.log10()))
        }),
    );
    fields.insert(
        "log2".to_string(),
        builtin("math.log2", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.log2")?;
            Ok(Value::Float(value.log2()))
        }),
    );
    fields.insert(
        "log1p".to_string(),
        builtin("math.log1p", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.log1p")?;
            Ok(Value::Float(value.ln_1p()))
        }),
    );
    fields.insert(
        "sin".to_string(),
        builtin("math.sin", 1, |mut args, _| {
            let radians = angle_from_value(args.pop().unwrap(), "math.sin")?;
            Ok(Value::Float(radians.sin()))
        }),
    );
    fields.insert(
        "cos".to_string(),
        builtin("math.cos", 1, |mut args, _| {
            let radians = angle_from_value(args.pop().unwrap(), "math.cos")?;
            Ok(Value::Float(radians.cos()))
        }),
    );
    fields.insert(
        "tan".to_string(),
        builtin("math.tan", 1, |mut args, _| {
            let radians = angle_from_value(args.pop().unwrap(), "math.tan")?;
            Ok(Value::Float(radians.tan()))
        }),
    );
    fields.insert(
        "asin".to_string(),
        builtin("math.asin", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.asin")?;
            Ok(angle_value(value.asin()))
        }),
    );
    fields.insert(
        "acos".to_string(),
        builtin("math.acos", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.acos")?;
            Ok(angle_value(value.acos()))
        }),
    );
    fields.insert(
        "atan".to_string(),
        builtin("math.atan", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.atan")?;
            Ok(angle_value(value.atan()))
        }),
    );
    fields.insert(
        "atan2".to_string(),
        builtin("math.atan2", 2, |mut args, _| {
            let x = expect_float(args.pop().unwrap(), "math.atan2")?;
            let y = expect_float(args.pop().unwrap(), "math.atan2")?;
            Ok(angle_value(y.atan2(x)))
        }),
    );
    fields.insert(
        "sinh".to_string(),
        builtin("math.sinh", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.sinh")?;
            Ok(Value::Float(value.sinh()))
        }),
    );
    fields.insert(
        "cosh".to_string(),
        builtin("math.cosh", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.cosh")?;
            Ok(Value::Float(value.cosh()))
        }),
    );
    fields.insert(
        "tanh".to_string(),
        builtin("math.tanh", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.tanh")?;
            Ok(Value::Float(value.tanh()))
        }),
    );
    fields.insert(
        "asinh".to_string(),
        builtin("math.asinh", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.asinh")?;
            Ok(Value::Float(value.asinh()))
        }),
    );
    fields.insert(
        "acosh".to_string(),
        builtin("math.acosh", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.acosh")?;
            Ok(Value::Float(value.acosh()))
        }),
    );
    fields.insert(
        "atanh".to_string(),
        builtin("math.atanh", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.atanh")?;
            Ok(Value::Float(value.atanh()))
        }),
    );
    fields.insert(
        "gcd".to_string(),
        builtin("math.gcd", 2, |mut args, _| {
            let right = expect_int(args.pop().unwrap(), "math.gcd")?;
            let left = expect_int(args.pop().unwrap(), "math.gcd")?;
            Ok(Value::Int(gcd_i64(left, right)))
        }),
    );
    fields.insert(
        "lcm".to_string(),
        builtin("math.lcm", 2, |mut args, _| {
            let right = expect_int(args.pop().unwrap(), "math.lcm")?;
            let left = expect_int(args.pop().unwrap(), "math.lcm")?;
            Ok(Value::Int(lcm_i64(left, right)))
        }),
    );
    fields.insert(
        "gcdAll".to_string(),
        builtin("math.gcdAll", 1, |mut args, _| {
            let list = expect_list(args.pop().unwrap(), "math.gcdAll")?;
            let values = list_ints(&list, "math.gcdAll")?;
            if values.is_empty() {
                return Ok(make_none());
            }
            let mut value = values[0];
            for item in values.iter().skip(1) {
                value = gcd_i64(value, *item);
            }
            Ok(make_some(Value::Int(value)))
        }),
    );
    fields.insert(
        "lcmAll".to_string(),
        builtin("math.lcmAll", 1, |mut args, _| {
            let list = expect_list(args.pop().unwrap(), "math.lcmAll")?;
            let values = list_ints(&list, "math.lcmAll")?;
            if values.is_empty() {
                return Ok(make_none());
            }
            let mut value = values[0];
            for item in values.iter().skip(1) {
                value = lcm_i64(value, *item);
            }
            Ok(make_some(Value::Int(value)))
        }),
    );
    fields.insert(
        "factorial".to_string(),
        builtin("math.factorial", 1, |mut args, _| {
            let n = expect_int(args.pop().unwrap(), "math.factorial")?;
            let value = factorial_bigint(n)
                .ok_or_else(|| RuntimeError::Message("math.factorial expects n >= 0".to_string()))?;
            Ok(Value::BigInt(Arc::new(value)))
        }),
    );
    fields.insert(
        "comb".to_string(),
        builtin("math.comb", 2, |mut args, _| {
            let k = expect_int(args.pop().unwrap(), "math.comb")?;
            let n = expect_int(args.pop().unwrap(), "math.comb")?;
            let value = comb_bigint(n, k)
                .ok_or_else(|| RuntimeError::Message("math.comb expects 0 <= k <= n".to_string()))?;
            Ok(Value::BigInt(Arc::new(value)))
        }),
    );
    fields.insert(
        "perm".to_string(),
        builtin("math.perm", 2, |mut args, _| {
            let k = expect_int(args.pop().unwrap(), "math.perm")?;
            let n = expect_int(args.pop().unwrap(), "math.perm")?;
            let value = perm_bigint(n, k)
                .ok_or_else(|| RuntimeError::Message("math.perm expects 0 <= k <= n".to_string()))?;
            Ok(Value::BigInt(Arc::new(value)))
        }),
    );
    fields.insert(
        "divmod".to_string(),
        builtin("math.divmod", 2, |mut args, _| {
            let b = expect_int(args.pop().unwrap(), "math.divmod")?;
            let a = expect_int(args.pop().unwrap(), "math.divmod")?;
            if b == 0 {
                return Err(RuntimeError::Message("math.divmod expects non-zero divisor".to_string()));
            }
            let mut q = a / b;
            let mut r = a % b;
            if r < 0 {
                let adj = if b > 0 { 1 } else { -1 };
                q -= adj;
                r += b.abs();
            }
            Ok(Value::Tuple(vec![Value::Int(q), Value::Int(r)]))
        }),
    );
    fields.insert(
        "modPow".to_string(),
        builtin("math.modPow", 3, |mut args, _| {
            let modulus = expect_int(args.pop().unwrap(), "math.modPow")?;
            let exp = expect_int(args.pop().unwrap(), "math.modPow")?;
            let base = expect_int(args.pop().unwrap(), "math.modPow")?;
            if exp < 0 || modulus == 0 {
                return Err(RuntimeError::Message("math.modPow expects exp >= 0 and modulus != 0".to_string()));
            }
            Ok(Value::Int(mod_pow(base, exp, modulus)))
        }),
    );
    fields.insert(
        "isFinite".to_string(),
        builtin("math.isFinite", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.isFinite")?;
            Ok(Value::Bool(value.is_finite()))
        }),
    );
    fields.insert(
        "isInf".to_string(),
        builtin("math.isInf", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.isInf")?;
            Ok(Value::Bool(value.is_infinite()))
        }),
    );
    fields.insert(
        "isNaN".to_string(),
        builtin("math.isNaN", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.isNaN")?;
            Ok(Value::Bool(value.is_nan()))
        }),
    );
    fields.insert(
        "nextAfter".to_string(),
        builtin("math.nextAfter", 2, |mut args, _| {
            let to = expect_float(args.pop().unwrap(), "math.nextAfter")?;
            let from = expect_float(args.pop().unwrap(), "math.nextAfter")?;
            Ok(Value::Float(next_after(from, to)))
        }),
    );
    fields.insert(
        "ulp".to_string(),
        builtin("math.ulp", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.ulp")?;
            let next = next_after(value, if value.is_sign_positive() { f64::INFINITY } else { f64::NEG_INFINITY });
            Ok(Value::Float((next - value).abs()))
        }),
    );
    fields.insert(
        "fmod".to_string(),
        builtin("math.fmod", 2, |mut args, _| {
            let b = expect_float(args.pop().unwrap(), "math.fmod")?;
            let a = expect_float(args.pop().unwrap(), "math.fmod")?;
            Ok(Value::Float(a % b))
        }),
    );
    fields.insert(
        "remainder".to_string(),
        builtin("math.remainder", 2, |mut args, _| {
            let b = expect_float(args.pop().unwrap(), "math.remainder")?;
            let a = expect_float(args.pop().unwrap(), "math.remainder")?;
            Ok(Value::Float(a - (a / b).round() * b))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn date_from_value(value: Value, ctx: &str) -> Result<NaiveDate, RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::Message(format!("{ctx} expects Date")));
    };
    let year = fields
        .get("year")
        .cloned()
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Date.year")))?;
    let month = fields
        .get("month")
        .cloned()
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Date.month")))?;
    let day = fields
        .get("day")
        .cloned()
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Date.day")))?;
    let year = expect_int(year, ctx)? as i32;
    let month = expect_int(month, ctx)? as u32;
    let day = expect_int(day, ctx)? as u32;
    NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects valid Date")))
}

fn date_to_value(date: NaiveDate) -> Value {
    let mut map = HashMap::new();
    map.insert("year".to_string(), Value::Int(date.year() as i64));
    map.insert("month".to_string(), Value::Int(date.month() as i64));
    map.insert("day".to_string(), Value::Int(date.day() as i64));
    Value::Record(Arc::new(map))
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_next = NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .expect("valid next month date");
    first_next.pred_opt().expect("previous day").day()
}

fn add_months(date: NaiveDate, months: i64) -> NaiveDate {
    let mut year = date.year() as i64;
    let mut month = date.month() as i64;
    let total = month - 1 + months;
    year += total.div_euclid(12);
    month = total.rem_euclid(12) + 1;
    let year_i32 = year as i32;
    let month_u32 = month as u32;
    let max_day = days_in_month(year_i32, month_u32);
    let day = date.day().min(max_day);
    NaiveDate::from_ymd_opt(year_i32, month_u32, day).expect("valid date")
}

fn build_calendar_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "isLeapYear".to_string(),
        builtin("calendar.isLeapYear", 1, |mut args, _| {
            let date = date_from_value(args.pop().unwrap(), "calendar.isLeapYear")?;
            let year = date.year();
            let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
            Ok(Value::Bool(leap))
        }),
    );
    fields.insert(
        "daysInMonth".to_string(),
        builtin("calendar.daysInMonth", 1, |mut args, _| {
            let date = date_from_value(args.pop().unwrap(), "calendar.daysInMonth")?;
            Ok(Value::Int(days_in_month(date.year(), date.month()) as i64))
        }),
    );
    fields.insert(
        "endOfMonth".to_string(),
        builtin("calendar.endOfMonth", 1, |mut args, _| {
            let date = date_from_value(args.pop().unwrap(), "calendar.endOfMonth")?;
            let max_day = days_in_month(date.year(), date.month());
            let end = NaiveDate::from_ymd_opt(date.year(), date.month(), max_day)
                .expect("valid end-of-month date");
            Ok(date_to_value(end))
        }),
    );
    fields.insert(
        "addDays".to_string(),
        builtin("calendar.addDays", 2, |mut args, _| {
            let days = expect_int(args.pop().unwrap(), "calendar.addDays")?;
            let date = date_from_value(args.pop().unwrap(), "calendar.addDays")?;
            let next = date
                .checked_add_signed(ChronoDuration::days(days))
                .ok_or_else(|| RuntimeError::Message("calendar.addDays overflow".to_string()))?;
            Ok(date_to_value(next))
        }),
    );
    fields.insert(
        "addMonths".to_string(),
        builtin("calendar.addMonths", 2, |mut args, _| {
            let months = expect_int(args.pop().unwrap(), "calendar.addMonths")?;
            let date = date_from_value(args.pop().unwrap(), "calendar.addMonths")?;
            Ok(date_to_value(add_months(date, months)))
        }),
    );
    fields.insert(
        "addYears".to_string(),
        builtin("calendar.addYears", 2, |mut args, _| {
            let years = expect_int(args.pop().unwrap(), "calendar.addYears")?;
            let date = date_from_value(args.pop().unwrap(), "calendar.addYears")?;
            let year = date.year() + years as i32;
            let max_day = days_in_month(year, date.month());
            let day = date.day().min(max_day);
            let next = NaiveDate::from_ymd_opt(year, date.month(), day)
                .ok_or_else(|| RuntimeError::Message("calendar.addYears invalid date".to_string()))?;
            Ok(date_to_value(next))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn rgb_from_value(value: Value, ctx: &str) -> Result<(f32, f32, f32), RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::Message(format!("{ctx} expects Rgb")));
    };
    let r = expect_int(
        fields
            .get("r")
            .cloned()
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Rgb.r")))?,
        ctx,
    )?;
    let g = expect_int(
        fields
            .get("g")
            .cloned()
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Rgb.g")))?,
        ctx,
    )?;
    let b = expect_int(
        fields
            .get("b")
            .cloned()
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Rgb.b")))?,
        ctx,
    )?;
    let clamp = |v: i64| v.max(0).min(255) as f32 / 255.0;
    Ok((clamp(r), clamp(g), clamp(b)))
}

fn rgb_to_value(rgb: Srgb<f32>) -> Value {
    let r = (rgb.red * 255.0).round().clamp(0.0, 255.0) as i64;
    let g = (rgb.green * 255.0).round().clamp(0.0, 255.0) as i64;
    let b = (rgb.blue * 255.0).round().clamp(0.0, 255.0) as i64;
    let mut map = HashMap::new();
    map.insert("r".to_string(), Value::Int(r));
    map.insert("g".to_string(), Value::Int(g));
    map.insert("b".to_string(), Value::Int(b));
    Value::Record(Arc::new(map))
}

fn hsl_from_value(value: Value, ctx: &str) -> Result<Hsl, RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::Message(format!("{ctx} expects Hsl")));
    };
    let h = expect_float(
        fields
            .get("h")
            .cloned()
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Hsl.h")))?,
        ctx,
    )?;
    let s = expect_float(
        fields
            .get("s")
            .cloned()
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Hsl.s")))?,
        ctx,
    )?;
    let l = expect_float(
        fields
            .get("l")
            .cloned()
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Hsl.l")))?,
        ctx,
    )?;
    let hue = RgbHue::from_degrees(h as f32);
    let s = s.clamp(0.0, 1.0) as f32;
    let l = l.clamp(0.0, 1.0) as f32;
    Ok(Hsl::new(hue, s, l))
}

fn hsl_to_value(hsl: Hsl) -> Value {
    let mut map = HashMap::new();
    map.insert("h".to_string(), Value::Float(hsl.hue.into_degrees() as f64));
    map.insert("s".to_string(), Value::Float(hsl.saturation as f64));
    map.insert("l".to_string(), Value::Float(hsl.lightness as f64));
    Value::Record(Arc::new(map))
}

fn build_color_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "adjustLightness".to_string(),
        builtin("color.adjustLightness", 2, |mut args, _| {
            let amount = expect_int(args.pop().unwrap(), "color.adjustLightness")?;
            let (r, g, b) = rgb_from_value(args.pop().unwrap(), "color.adjustLightness")?;
            let hsl: Hsl = Hsl::from_color(Srgb::new(r, g, b));
            let delta = amount as f32 / 100.0;
            let next = Hsl::new(hsl.hue, hsl.saturation, (hsl.lightness + delta).clamp(0.0, 1.0));
            Ok(rgb_to_value(Srgb::from_color(next)))
        }),
    );
    fields.insert(
        "adjustSaturation".to_string(),
        builtin("color.adjustSaturation", 2, |mut args, _| {
            let amount = expect_int(args.pop().unwrap(), "color.adjustSaturation")?;
            let (r, g, b) = rgb_from_value(args.pop().unwrap(), "color.adjustSaturation")?;
            let hsl: Hsl = Hsl::from_color(Srgb::new(r, g, b));
            let delta = amount as f32 / 100.0;
            let next = Hsl::new(hsl.hue, (hsl.saturation + delta).clamp(0.0, 1.0), hsl.lightness);
            Ok(rgb_to_value(Srgb::from_color(next)))
        }),
    );
    fields.insert(
        "adjustHue".to_string(),
        builtin("color.adjustHue", 2, |mut args, _| {
            let degrees = expect_int(args.pop().unwrap(), "color.adjustHue")?;
            let (r, g, b) = rgb_from_value(args.pop().unwrap(), "color.adjustHue")?;
            let hsl: Hsl = Hsl::from_color(Srgb::new(r, g, b));
            let hue = (hsl.hue.into_degrees() + degrees as f32).rem_euclid(360.0);
            let next = Hsl::new(RgbHue::from_degrees(hue), hsl.saturation, hsl.lightness);
            Ok(rgb_to_value(Srgb::from_color(next)))
        }),
    );
    fields.insert(
        "toRgb".to_string(),
        builtin("color.toRgb", 1, |mut args, _| {
            let hsl = hsl_from_value(args.pop().unwrap(), "color.toRgb")?;
            Ok(rgb_to_value(Srgb::from_color(hsl)))
        }),
    );
    fields.insert(
        "toHsl".to_string(),
        builtin("color.toHsl", 1, |mut args, _| {
            let (r, g, b) = rgb_from_value(args.pop().unwrap(), "color.toHsl")?;
            let hsl: Hsl = Hsl::from_color(Srgb::new(r, g, b));
            Ok(hsl_to_value(hsl))
        }),
    );
    fields.insert(
        "toHex".to_string(),
        builtin("color.toHex", 1, |mut args, _| {
            let (r, g, b) = rgb_from_value(args.pop().unwrap(), "color.toHex")?;
            let r = (r * 255.0).round().clamp(0.0, 255.0) as u8;
            let g = (g * 255.0).round().clamp(0.0, 255.0) as u8;
            let b = (b * 255.0).round().clamp(0.0, 255.0) as u8;
            Ok(Value::Text(format!("#{r:02x}{g:02x}{b:02x}")))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn build_bigint_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "fromInt".to_string(),
        builtin("bigint.fromInt", 1, |mut args, _| {
            let value = expect_int(args.pop().unwrap(), "bigint.fromInt")?;
            Ok(Value::BigInt(Arc::new(BigInt::from(value))))
        }),
    );
    fields.insert(
        "toInt".to_string(),
        builtin("bigint.toInt", 1, |mut args, _| {
            let value = expect_bigint(args.pop().unwrap(), "bigint.toInt")?;
            let out = value.to_i64().ok_or_else(|| {
                RuntimeError::Message("bigint.toInt overflow".to_string())
            })?;
            Ok(Value::Int(out))
        }),
    );
    fields.insert(
        "add".to_string(),
        builtin("bigint.add", 2, |mut args, _| {
            let right = expect_bigint(args.pop().unwrap(), "bigint.add")?;
            let left = expect_bigint(args.pop().unwrap(), "bigint.add")?;
            Ok(Value::BigInt(Arc::new(&*left + &*right)))
        }),
    );
    fields.insert(
        "sub".to_string(),
        builtin("bigint.sub", 2, |mut args, _| {
            let right = expect_bigint(args.pop().unwrap(), "bigint.sub")?;
            let left = expect_bigint(args.pop().unwrap(), "bigint.sub")?;
            Ok(Value::BigInt(Arc::new(&*left - &*right)))
        }),
    );
    fields.insert(
        "mul".to_string(),
        builtin("bigint.mul", 2, |mut args, _| {
            let right = expect_bigint(args.pop().unwrap(), "bigint.mul")?;
            let left = expect_bigint(args.pop().unwrap(), "bigint.mul")?;
            Ok(Value::BigInt(Arc::new(&*left * &*right)))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn build_rational_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "fromBigInts".to_string(),
        builtin("rational.fromBigInts", 2, |mut args, _| {
            let denom = expect_bigint(args.pop().unwrap(), "rational.fromBigInts")?;
            let numer = expect_bigint(args.pop().unwrap(), "rational.fromBigInts")?;
            if denom.is_zero() {
                return Err(RuntimeError::Message(
                    "rational.fromBigInts expects non-zero denominator".to_string(),
                ));
            }
            Ok(Value::Rational(Arc::new(BigRational::new(
                (*numer).clone(),
                (*denom).clone(),
            ))))
        }),
    );
    fields.insert(
        "normalize".to_string(),
        builtin("rational.normalize", 1, |mut args, _| {
            let value = expect_rational(args.pop().unwrap(), "rational.normalize")?;
            Ok(Value::Rational(Arc::new((*value).clone())))
        }),
    );
    fields.insert(
        "numerator".to_string(),
        builtin("rational.numerator", 1, |mut args, _| {
            let value = expect_rational(args.pop().unwrap(), "rational.numerator")?;
            Ok(Value::BigInt(Arc::new(value.numer().clone())))
        }),
    );
    fields.insert(
        "denominator".to_string(),
        builtin("rational.denominator", 1, |mut args, _| {
            let value = expect_rational(args.pop().unwrap(), "rational.denominator")?;
            Ok(Value::BigInt(Arc::new(value.denom().clone())))
        }),
    );
    fields.insert(
        "add".to_string(),
        builtin("rational.add", 2, |mut args, _| {
            let right = expect_rational(args.pop().unwrap(), "rational.add")?;
            let left = expect_rational(args.pop().unwrap(), "rational.add")?;
            Ok(Value::Rational(Arc::new(&*left + &*right)))
        }),
    );
    fields.insert(
        "sub".to_string(),
        builtin("rational.sub", 2, |mut args, _| {
            let right = expect_rational(args.pop().unwrap(), "rational.sub")?;
            let left = expect_rational(args.pop().unwrap(), "rational.sub")?;
            Ok(Value::Rational(Arc::new(&*left - &*right)))
        }),
    );
    fields.insert(
        "mul".to_string(),
        builtin("rational.mul", 2, |mut args, _| {
            let right = expect_rational(args.pop().unwrap(), "rational.mul")?;
            let left = expect_rational(args.pop().unwrap(), "rational.mul")?;
            Ok(Value::Rational(Arc::new(&*left * &*right)))
        }),
    );
    fields.insert(
        "div".to_string(),
        builtin("rational.div", 2, |mut args, _| {
            let right = expect_rational(args.pop().unwrap(), "rational.div")?;
            let left = expect_rational(args.pop().unwrap(), "rational.div")?;
            Ok(Value::Rational(Arc::new(&*left / &*right)))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn build_decimal_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "fromFloat".to_string(),
        builtin("decimal.fromFloat", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "decimal.fromFloat")?;
            let decimal = Decimal::from_f64(value).ok_or_else(|| {
                RuntimeError::Message("decimal.fromFloat expects finite Float".to_string())
            })?;
            Ok(Value::Decimal(decimal))
        }),
    );
    fields.insert(
        "toFloat".to_string(),
        builtin("decimal.toFloat", 1, |mut args, _| {
            let value = expect_decimal(args.pop().unwrap(), "decimal.toFloat")?;
            let out = value.to_f64().ok_or_else(|| {
                RuntimeError::Message("decimal.toFloat overflow".to_string())
            })?;
            Ok(Value::Float(out))
        }),
    );
    fields.insert(
        "round".to_string(),
        builtin("decimal.round", 2, |mut args, _| {
            let places = expect_int(args.pop().unwrap(), "decimal.round")?;
            let value = expect_decimal(args.pop().unwrap(), "decimal.round")?;
            let places = places.max(0) as u32;
            Ok(Value::Decimal(value.round_dp(places)))
        }),
    );
    fields.insert(
        "add".to_string(),
        builtin("decimal.add", 2, |mut args, _| {
            let right = expect_decimal(args.pop().unwrap(), "decimal.add")?;
            let left = expect_decimal(args.pop().unwrap(), "decimal.add")?;
            Ok(Value::Decimal(left + right))
        }),
    );
    fields.insert(
        "sub".to_string(),
        builtin("decimal.sub", 2, |mut args, _| {
            let right = expect_decimal(args.pop().unwrap(), "decimal.sub")?;
            let left = expect_decimal(args.pop().unwrap(), "decimal.sub")?;
            Ok(Value::Decimal(left - right))
        }),
    );
    fields.insert(
        "mul".to_string(),
        builtin("decimal.mul", 2, |mut args, _| {
            let right = expect_decimal(args.pop().unwrap(), "decimal.mul")?;
            let left = expect_decimal(args.pop().unwrap(), "decimal.mul")?;
            Ok(Value::Decimal(left * right))
        }),
    );
    fields.insert(
        "div".to_string(),
        builtin("decimal.div", 2, |mut args, _| {
            let right = expect_decimal(args.pop().unwrap(), "decimal.div")?;
            let left = expect_decimal(args.pop().unwrap(), "decimal.div")?;
            Ok(Value::Decimal(left / right))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn url_to_record(url: &Url) -> HashMap<String, Value> {
    let mut map = HashMap::new();
    map.insert("protocol".to_string(), Value::Text(url.scheme().to_string()));
    map.insert(
        "host".to_string(),
        Value::Text(url.host_str().unwrap_or("").to_string()),
    );
    let port = match url.port() {
        Some(port) => make_some(Value::Int(port as i64)),
        None => make_none(),
    };
    map.insert("port".to_string(), port);
    map.insert("path".to_string(), Value::Text(url.path().to_string()));
    let mut query_items = Vec::new();
    for (key, value) in url.query_pairs() {
        query_items.push(Value::Tuple(vec![
            Value::Text(key.to_string()),
            Value::Text(value.to_string()),
        ]));
    }
    map.insert("query".to_string(), list_value(query_items));
    let hash = match url.fragment() {
        Some(fragment) => make_some(Value::Text(fragment.to_string())),
        None => make_none(),
    };
    map.insert("hash".to_string(), hash);
    map
}

fn url_from_value(value: Value, ctx: &str) -> Result<Url, RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::Message(format!("{ctx} expects Url")));
    };
    let protocol = expect_text(
        fields
            .get("protocol")
            .cloned()
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Url.protocol")))?,
        ctx,
    )?;
    let host = expect_text(
        fields
            .get("host")
            .cloned()
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Url.host")))?,
        ctx,
    )?;
    let base = format!("{protocol}://{host}");
    let mut url = Url::parse(&base).map_err(|err| {
        RuntimeError::Message(format!("{ctx} invalid Url base: {err}"))
    })?;
    if let Some(port) = fields.get("port") {
        match port {
            Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
                let port = expect_int(args[0].clone(), ctx)? as u16;
                url.set_port(Some(port)).map_err(|_| {
                    RuntimeError::Message(format!("{ctx} invalid Url port"))
                })?;
            }
            Value::Constructor { name, args } if name == "None" && args.is_empty() => {}
            _ => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} expects Url.port Option"
                )))
            }
        }
    }
    if let Some(path) = fields.get("path") {
        let path = expect_text(path.clone(), ctx)?;
        url.set_path(&path);
    }
    if let Some(query) = fields.get("query") {
        let list = expect_list(query.clone(), ctx)?;
        let mut pairs = url.query_pairs_mut();
        pairs.clear();
        for item in list.iter() {
            if let Value::Tuple(items) = item {
                if items.len() == 2 {
                    let key = expect_text(items[0].clone(), ctx)?;
                    let value = expect_text(items[1].clone(), ctx)?;
                    pairs.append_pair(&key, &value);
                    continue;
                }
            }
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Url.query entries"
            )));
        }
        drop(pairs);
    }
    if let Some(hash) = fields.get("hash") {
        match hash {
            Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
                let value = expect_text(args[0].clone(), ctx)?;
                url.set_fragment(Some(&value));
            }
            Value::Constructor { name, args } if name == "None" && args.is_empty() => {
                url.set_fragment(None);
            }
            _ => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} expects Url.hash Option"
                )))
            }
        }
    }
    Ok(url)
}

fn build_url_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "parse".to_string(),
        builtin("url.parse", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "url.parse")?;
            match Url::parse(&text) {
                Ok(url) => Ok(make_ok(Value::Record(Arc::new(url_to_record(&url))))),
                Err(err) => Ok(make_err(Value::Text(err.to_string()))),
            }
        }),
    );
    fields.insert(
        "toString".to_string(),
        builtin("url.toString", 1, |mut args, _| {
            let url = url_from_value(args.pop().unwrap(), "url.toString")?;
            Ok(Value::Text(url.to_string()))
        }),
    );
    Value::Record(Arc::new(fields))
}

#[derive(Copy, Clone)]
enum HttpClientMode {
    Http,
    Https,
}

fn build_http_client_record(mode: HttpClientMode) -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "get".to_string(),
        builtin("http.get", 1, move |mut args, _| {
            let url = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let url = url_from_value(url.clone(), "http.get")?;
                    ensure_http_scheme(&url, mode, "http.get")?;
                    http_request("GET", &url, Vec::new(), None)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "post".to_string(),
        builtin("http.post", 2, move |mut args, _| {
            let body = args.pop().unwrap();
            let url = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let url = url_from_value(url.clone(), "http.post")?;
                    ensure_http_scheme(&url, mode, "http.post")?;
                    let body = expect_text(body.clone(), "http.post")?;
                    http_request("POST", &url, Vec::new(), Some(body))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "fetch".to_string(),
        builtin("http.fetch", 1, move |mut args, _| {
            let request = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let record = expect_record(request.clone(), "http.fetch expects Request")?;
                    let method = match record.get("method") {
                        Some(Value::Text(text)) => text.clone(),
                        _ => {
                            return Err(RuntimeError::Message(
                                "http.fetch expects Request.method Text".to_string(),
                            ))
                        }
                    };
                    let url_value = record.get("url").cloned().ok_or_else(|| {
                        RuntimeError::Message("http.fetch expects Request.url".to_string())
                    })?;
                    let url = url_from_value(url_value, "http.fetch")?;
                    ensure_http_scheme(&url, mode, "http.fetch")?;
                    let headers = match record.get("headers") {
                        Some(value) => headers_from_value(value, "http.fetch")?,
                        None => Vec::new(),
                    };
                    let body = match record.get("body") {
                        Some(value) => text_option_from_value(value.clone(), "http.fetch")?,
                        None => None,
                    };
                    http_request(&method, &url, headers, body)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn ensure_http_scheme(url: &Url, mode: HttpClientMode, ctx: &str) -> Result<(), RuntimeError> {
    match mode {
        HttpClientMode::Http => Ok(()),
        HttpClientMode::Https => {
            if url.scheme() == "https" {
                Ok(())
            } else {
                Err(RuntimeError::Message(format!(
                    "{ctx} expects an https URL"
                )))
            }
        }
    }
}

fn http_request(
    method: &str,
    url: &Url,
    headers: Vec<(String, String)>,
    body: Option<String>,
) -> Result<Value, RuntimeError> {
    let url_text = url.to_string();
    let mut request = ureq::request(method, &url_text);
    for (name, value) in headers {
        request = request.set(&name, &value);
    }
    let response = match body {
        Some(text) => request.send_string(&text),
        None => request.call(),
    };
    match response {
        Ok(resp) => Ok(make_ok(http_response_to_value(resp)?)),
        Err(err) => Ok(make_err(http_error_record(http_error_message(err)?))),
    }
}

fn http_error_message(err: UreqError) -> Result<String, RuntimeError> {
    match err {
        UreqError::Status(code, response) => {
            let body = response
                .into_string()
                .unwrap_or_else(|_| String::new());
            if body.is_empty() {
                Ok(format!("http status {code}"))
            } else {
                Ok(format!("http status {code}: {body}"))
            }
        }
        UreqError::Transport(err) => Ok(err.to_string()),
    }
}

fn http_response_to_value(resp: ureq::Response) -> Result<Value, RuntimeError> {
    let status = resp.status() as i64;
    let headers = headers_to_value(
        resp.headers_names()
            .into_iter()
            .filter_map(|name| {
                resp.header(&name)
                    .map(|value| (name.to_string(), value.to_string()))
            })
            .collect(),
    );
    let body = resp
        .into_string()
        .map_err(|err| RuntimeError::Error(Value::Text(err.to_string())))?;
    let mut fields = HashMap::new();
    fields.insert("status".to_string(), Value::Int(status));
    fields.insert("headers".to_string(), headers);
    fields.insert("body".to_string(), Value::Text(body));
    Ok(Value::Record(Arc::new(fields)))
}

fn headers_from_value(value: &Value, ctx: &str) -> Result<Vec<(String, String)>, RuntimeError> {
    let list = match value {
        Value::List(items) => items,
        _ => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Request.headers List"
            )))
        }
    };
    let mut headers = Vec::with_capacity(list.len());
    for item in list.iter() {
        let record = match item {
            Value::Record(fields) => fields,
            _ => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} expects header records"
                )))
            }
        };
        let name = match record.get("name") {
            Some(Value::Text(text)) => text.clone(),
            _ => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} expects header.name Text"
                )))
            }
        };
        let value = match record.get("value") {
            Some(Value::Text(text)) => text.clone(),
            _ => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} expects header.value Text"
                )))
            }
        };
        headers.push((name, value));
    }
    Ok(headers)
}

fn headers_to_value(entries: Vec<(String, String)>) -> Value {
    let mut list = Vec::with_capacity(entries.len());
    for (name, value) in entries {
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), Value::Text(name));
        fields.insert("value".to_string(), Value::Text(value));
        list.push(Value::Record(Arc::new(fields)));
    }
    Value::List(Arc::new(list))
}

fn text_option_from_value(value: Value, ctx: &str) -> Result<Option<String>, RuntimeError> {
    match value {
        Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
            Ok(Some(expect_text(args[0].clone(), ctx)?))
        }
        Value::Constructor { name, args } if name == "None" && args.is_empty() => Ok(None),
        _ => Err(RuntimeError::Message(format!(
            "{ctx} expects Option Text"
        ))),
    }
}

fn http_error_record(message: String) -> Value {
    let mut fields = HashMap::new();
    fields.insert("message".to_string(), Value::Text(message));
    Value::Record(Arc::new(fields))
}

fn build_console_record() -> Value {
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

fn spawn_effect(
    id: usize,
    effect: Value,
    ctx: Arc<RuntimeContext>,
    cancel: Arc<CancelToken>,
    sender: mpsc::Sender<(usize, Result<Value, RuntimeError>)>,
) {
    std::thread::spawn(move || {
        let mut runtime = Runtime::new(ctx, cancel);
        let result = runtime.run_effect_value(effect);
        let _ = sender.send((id, result));
    });
}
