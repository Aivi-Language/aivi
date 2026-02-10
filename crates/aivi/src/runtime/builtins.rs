use std::collections::HashMap;
use std::io::{Read, Seek, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::http::build_http_server_record;
use super::{
    CancelToken, EffectValue, Env, Runtime,
};
use super::values::{ChannelInner, ChannelRecv, ChannelSend};
    RuntimeContext, RuntimeError, Value, format_value,
    BuiltinImpl, BuiltinValue,
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
                    stats.insert("is_file".to_string(), Value::Bool(metadata.is_file()));
                    stats.insert(
                        "is_directory".to_string(),
                        Value::Bool(metadata.is_dir()),
                    );
                    Ok(Value::Record(stats))
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
    Value::Record(fields)
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
    Value::Record(fields)
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
    Value::Record(fields)
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
    Value::Record(fields)
}

fn build_concurrent_record() -> Value {
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
    Value::Record(fields)
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

