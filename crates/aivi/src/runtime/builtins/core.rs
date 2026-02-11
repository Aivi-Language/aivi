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
    env.set("linalg".to_string(), build_linalg_record());
    env.set("signal".to_string(), build_signal_record());
    env.set("graph".to_string(), build_graph_record());
    env.set("bigint".to_string(), build_bigint_record());
    env.set("rational".to_string(), build_rational_record());
    env.set("decimal".to_string(), build_decimal_record());
    env.set("url".to_string(), build_url_record());
    env.set("http".to_string(), build_http_client_record(HttpClientMode::Http));
    env.set("https".to_string(), build_http_client_record(HttpClientMode::Https));
    let collections = build_collections_record();
    if let Value::Record(fields) = &collections {
        if let Some(map) = fields.get("map") {
            env.set("Map".to_string(), map.clone());
        }
        if let Some(set) = fields.get("set") {
            env.set("Set".to_string(), set.clone());
        }
        if let Some(queue) = fields.get("queue") {
            env.set("Queue".to_string(), queue.clone());
        }
        if let Some(deque) = fields.get("deque") {
            env.set("Deque".to_string(), deque.clone());
        }
        if let Some(heap) = fields.get("heap") {
            env.set("Heap".to_string(), heap.clone());
        }
    }
    env.set("collections".to_string(), collections);
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

fn expect_map(
    value: Value,
    ctx: &str,
) -> Result<Arc<ImHashMap<KeyValue, Value>>, RuntimeError> {
    match value {
        Value::Map(entries) => Ok(entries),
        _ => Err(RuntimeError::Message(format!("{ctx} expects Map"))),
    }
}

fn expect_set(
    value: Value,
    ctx: &str,
) -> Result<Arc<ImHashSet<KeyValue>>, RuntimeError> {
    match value {
        Value::Set(entries) => Ok(entries),
        _ => Err(RuntimeError::Message(format!("{ctx} expects Set"))),
    }
}

fn expect_queue(
    value: Value,
    ctx: &str,
) -> Result<Arc<ImVector<Value>>, RuntimeError> {
    match value {
        Value::Queue(items) => Ok(items),
        _ => Err(RuntimeError::Message(format!("{ctx} expects Queue"))),
    }
}

fn expect_deque(
    value: Value,
    ctx: &str,
) -> Result<Arc<ImVector<Value>>, RuntimeError> {
    match value {
        Value::Deque(items) => Ok(items),
        _ => Err(RuntimeError::Message(format!("{ctx} expects Deque"))),
    }
}

fn expect_heap(
    value: Value,
    ctx: &str,
) -> Result<Arc<BinaryHeap<Reverse<KeyValue>>>, RuntimeError> {
    match value {
        Value::Heap(items) => Ok(items),
        _ => Err(RuntimeError::Message(format!("{ctx} expects Heap"))),
    }
}

fn key_from_value(value: &Value, ctx: &str) -> Result<KeyValue, RuntimeError> {
    KeyValue::try_from_value(value).ok_or_else(|| {
        RuntimeError::Message(format!("{ctx} expects a hashable key"))
    })
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

fn vec_from_value(value: Value, ctx: &str) -> Result<(i64, Vec<f64>), RuntimeError> {
    let record = expect_record(value, ctx)?;
    let size = match record.get("size") {
        Some(value) => expect_int(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Vec.size"
            )))
        }
    };
    let data_list = match record.get("data") {
        Some(value) => expect_list(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Vec.data"
            )))
        }
    };
    let data = list_floats(&data_list, ctx)?;
    if size < 0 || data.len() != size as usize {
        return Err(RuntimeError::Message(format!(
            "{ctx} Vec.size does not match data length"
        )));
    }
    Ok((size, data))
}

fn vec_to_value(size: i64, data: Vec<f64>) -> Value {
    let mut fields = HashMap::new();
    fields.insert("size".to_string(), Value::Int(size));
    let list = data.into_iter().map(Value::Float).collect();
    fields.insert("data".to_string(), Value::List(Arc::new(list)));
    Value::Record(Arc::new(fields))
}

fn mat_from_value(value: Value, ctx: &str) -> Result<(i64, i64, Vec<f64>), RuntimeError> {
    let record = expect_record(value, ctx)?;
    let rows = match record.get("rows") {
        Some(value) => expect_int(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Mat.rows"
            )))
        }
    };
    let cols = match record.get("cols") {
        Some(value) => expect_int(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Mat.cols"
            )))
        }
    };
    let data_list = match record.get("data") {
        Some(value) => expect_list(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Mat.data"
            )))
        }
    };
    let data = list_floats(&data_list, ctx)?;
    if rows < 0 || cols < 0 || data.len() != (rows * cols) as usize {
        return Err(RuntimeError::Message(format!(
            "{ctx} Mat dimensions do not match data length"
        )));
    }
    Ok((rows, cols, data))
}

fn mat_to_value(rows: i64, cols: i64, data: Vec<f64>) -> Value {
    let mut fields = HashMap::new();
    fields.insert("rows".to_string(), Value::Int(rows));
    fields.insert("cols".to_string(), Value::Int(cols));
    let list = data.into_iter().map(Value::Float).collect();
    fields.insert("data".to_string(), Value::List(Arc::new(list)));
    Value::Record(Arc::new(fields))
}

fn signal_from_value(value: Value, ctx: &str) -> Result<(Vec<f64>, f64), RuntimeError> {
    let record = expect_record(value, ctx)?;
    let rate = match record.get("rate") {
        Some(value) => expect_float(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Signal.rate"
            )))
        }
    };
    let samples_list = match record.get("samples") {
        Some(value) => expect_list(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Signal.samples"
            )))
        }
    };
    let samples = list_floats(&samples_list, ctx)?;
    Ok((samples, rate))
}

fn spectrum_from_value(value: Value, ctx: &str) -> Result<(Vec<FftComplex<f64>>, f64), RuntimeError> {
    let record = expect_record(value, ctx)?;
    let rate = match record.get("rate") {
        Some(value) => expect_float(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Spectrum.rate"
            )))
        }
    };
    let bins_list = match record.get("bins") {
        Some(value) => expect_list(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Spectrum.bins"
            )))
        }
    };
    let mut bins = Vec::with_capacity(bins_list.len());
    for item in bins_list.iter() {
        let record = expect_record(item.clone(), ctx)?;
        let re = match record.get("re") {
            Some(value) => expect_float(value.clone(), ctx)?,
            None => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} expects Complex.re"
                )))
            }
        };
        let im = match record.get("im") {
            Some(value) => expect_float(value.clone(), ctx)?,
            None => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} expects Complex.im"
                )))
            }
        };
        bins.push(FftComplex::new(re, im));
    }
    Ok((bins, rate))
}

fn signal_to_value(samples: Vec<f64>, rate: f64) -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "samples".to_string(),
        Value::List(Arc::new(samples.into_iter().map(Value::Float).collect())),
    );
    fields.insert("rate".to_string(), Value::Float(rate));
    Value::Record(Arc::new(fields))
}

fn spectrum_to_value(bins: Vec<FftComplex<f64>>, rate: f64) -> Value {
    let mut fields = HashMap::new();
    let list = bins
        .into_iter()
        .map(|value| {
            let mut complex = HashMap::new();
            complex.insert("re".to_string(), Value::Float(value.re));
            complex.insert("im".to_string(), Value::Float(value.im));
            Value::Record(Arc::new(complex))
        })
        .collect();
    fields.insert("bins".to_string(), Value::List(Arc::new(list)));
    fields.insert("rate".to_string(), Value::Float(rate));
    Value::Record(Arc::new(fields))
}

fn edge_from_value(value: Value, ctx: &str) -> Result<(i64, i64, f64), RuntimeError> {
    let record = expect_record(value, ctx)?;
    let from = match record.get("from") {
        Some(value) => expect_int(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Edge.from"
            )))
        }
    };
    let to = match record.get("to") {
        Some(value) => expect_int(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Edge.to"
            )))
        }
    };
    let weight = match record.get("weight") {
        Some(value) => expect_float(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Edge.weight"
            )))
        }
    };
    Ok((from, to, weight))
}

fn graph_from_value(
    value: Value,
    ctx: &str,
) -> Result<(Vec<i64>, Vec<(i64, i64, f64)>), RuntimeError> {
    let record = expect_record(value, ctx)?;
    let nodes_list = match record.get("nodes") {
        Some(value) => expect_list(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Graph.nodes"
            )))
        }
    };
    let edges_list = match record.get("edges") {
        Some(value) => expect_list(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Graph.edges"
            )))
        }
    };
    let nodes = list_ints(&nodes_list, ctx)?;
    let mut edges = Vec::with_capacity(edges_list.len());
    for edge in edges_list.iter() {
        edges.push(edge_from_value(edge.clone(), ctx)?);
    }
    Ok((nodes, edges))
}

fn graph_to_value(nodes: Vec<i64>, edges: Vec<(i64, i64, f64)>) -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "nodes".to_string(),
        Value::List(Arc::new(nodes.into_iter().map(Value::Int).collect())),
    );
    let list = edges
        .into_iter()
        .map(|(from, to, weight)| {
            let mut edge = HashMap::new();
            edge.insert("from".to_string(), Value::Int(from));
            edge.insert("to".to_string(), Value::Int(to));
            edge.insert("weight".to_string(), Value::Float(weight));
            Value::Record(Arc::new(edge))
        })
        .collect();
    fields.insert("edges".to_string(), Value::List(Arc::new(list)));
    Value::Record(Arc::new(fields))
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

