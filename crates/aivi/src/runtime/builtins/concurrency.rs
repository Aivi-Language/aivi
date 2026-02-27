use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use super::util::{builtin, expect_int};
use crate::runtime::values::{ChannelInner, ChannelRecv, ChannelSend, ChannelSender};
use crate::runtime::{CancelToken, EffectValue, Runtime, RuntimeContext, RuntimeError, Value};

pub(super) fn build_channel_record() -> Value {
    let mut fields = std::collections::HashMap::new();
    fields.insert(
        "make".to_string(),
        builtin("channel.make", 1, |_, _| {
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let (sender, receiver) = mpsc::channel();
                    let inner = Arc::new(ChannelInner {
                        sender: Mutex::new(Some(ChannelSender::Unbounded(sender))),
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
        "makeBounded".to_string(),
        builtin("channel.makeBounded", 1, |mut args, _| {
            let capacity = expect_int(args.pop().unwrap(), "channel.makeBounded")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    if capacity <= 0 {
                        return Err(RuntimeError::Message(
                            "channel.makeBounded expects capacity > 0".to_string(),
                        ));
                    }
                    let (sender, receiver) = mpsc::sync_channel(capacity as usize);
                    let inner = Arc::new(ChannelInner {
                        sender: Mutex::new(Some(ChannelSender::Bounded(sender))),
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
                        return Err(RuntimeError::Message("channel is closed".to_string()));
                    }
                    let sender_guard = sender
                        .inner
                        .sender
                        .lock()
                        .map_err(|_| RuntimeError::Message("channel poisoned".to_string()))?;
                    if let Some(sender) = sender_guard.as_ref() {
                        match sender {
                            ChannelSender::Unbounded(inner) => {
                                inner.send(value.clone()).map_err(|_| {
                                    RuntimeError::Message("channel is closed".to_string())
                                })?
                            }
                            ChannelSender::Bounded(inner) => {
                                inner.send(value.clone()).map_err(|_| {
                                    RuntimeError::Message("channel is closed".to_string())
                                })?
                            }
                        }
                        Ok(Value::Unit)
                    } else {
                        Err(RuntimeError::Message("channel is closed".to_string()))
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
                    // Pump GTK events first on every iteration so input events
                    // (clicks, motion, key presses) are processed immediately
                    // rather than waiting up to 25ms for the next timeout.
                    super::gtk4_real::pump_gtk_events();
                    let recv_guard = receiver
                        .inner
                        .receiver
                        .lock()
                        .map_err(|_| RuntimeError::Message("channel poisoned".to_string()))?;
                    match recv_guard.recv_timeout(Duration::from_millis(1)) {
                        Ok(value) => {
                            return Ok(Value::Constructor {
                                name: "Ok".to_string(),
                                args: vec![value],
                            });
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            drop(recv_guard);
                            continue;
                        }
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

fn spawn_task(effect_value: Value, ctx: Arc<RuntimeContext>, parent: Arc<CancelToken>) -> Value {
    let cancel = CancelToken::child(parent);
    let result = Arc::new(Mutex::new(None::<Result<Value, RuntimeError>>));

    {
        let result = result.clone();
        let cancel_for_thread = cancel.clone();
        std::thread::spawn(move || {
            let mut runtime = Runtime::new(ctx, cancel_for_thread);
            let output = runtime.run_effect_value(effect_value);
            if let Ok(mut guard) = result.lock() {
                *guard = Some(output);
            }
        });
    }

    let mut fields = std::collections::HashMap::new();
    fields.insert(
        "join".to_string(),
        Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new({
                let result = result.clone();
                move |runtime| loop {
                    runtime.check_cancelled()?;
                    let mut guard = result
                        .lock()
                        .map_err(|_| RuntimeError::Message("task poisoned".to_string()))?;
                    if let Some(output) = guard.take() {
                        return output;
                    }
                    drop(guard);
                    std::thread::sleep(Duration::from_millis(25));
                }
            }),
        })),
    );
    fields.insert(
        "cancel".to_string(),
        Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new({
                let cancel = cancel.clone();
                move |_| {
                    cancel.cancel();
                    Ok(Value::Unit)
                }
            }),
        })),
    );
    fields.insert(
        "isCancelled".to_string(),
        Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new(move |_| Ok(Value::Bool(cancel.is_cancelled()))),
        })),
    );

    Value::Record(Arc::new(fields))
}

fn make_cancel_token() -> Value {
    let token = Arc::new(AtomicBool::new(false));
    let mut fields = std::collections::HashMap::new();
    fields.insert(
        "cancel".to_string(),
        Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new({
                let token = token.clone();
                move |_| {
                    token.store(true, Ordering::SeqCst);
                    Ok(Value::Unit)
                }
            }),
        })),
    );
    fields.insert(
        "isCancelled".to_string(),
        Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new(move |_| Ok(Value::Bool(token.load(Ordering::SeqCst)))),
        })),
    );
    Value::Record(Arc::new(fields))
}

pub(crate) fn build_concurrent_record() -> Value {
    let mut fields = std::collections::HashMap::new();
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
        "fork".to_string(),
        builtin("concurrent.fork", 1, |mut args, runtime| {
            let effect_value = args.pop().unwrap();
            let parent = runtime.cancel.clone();
            let task = spawn_task(effect_value, runtime.ctx.clone(), parent);
            Ok(task)
        }),
    );
    fields.insert(
        "sleep".to_string(),
        builtin("concurrent.sleep", 1, |mut args, _| {
            let millis = expect_int(args.pop().unwrap(), "concurrent.sleep")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    if millis <= 0 {
                        return Ok(Value::Unit);
                    }
                    let deadline = std::time::Instant::now() + Duration::from_millis(millis as u64);
                    while std::time::Instant::now() < deadline {
                        runtime.check_cancelled()?;
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "timeoutWith".to_string(),
        builtin("concurrent.timeoutWith", 3, |mut args, runtime| {
            let effect_value = args.pop().unwrap();
            let timeout_error = args.pop().unwrap();
            let millis = expect_int(args.pop().unwrap(), "concurrent.timeoutWith")?;
            let ctx = runtime.ctx.clone();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let left_cancel = CancelToken::child(runtime.cancel.clone());
                    let right_cancel = CancelToken::child(runtime.cancel.clone());
                    let (tx, rx) = mpsc::channel();

                    spawn_effect(
                        0,
                        effect_value.clone(),
                        ctx.clone(),
                        left_cancel.clone(),
                        tx.clone(),
                    );

                    std::thread::spawn({
                        let tx = tx.clone();
                        let right_cancel = right_cancel.clone();
                        move || {
                            if millis > 0 {
                                std::thread::sleep(Duration::from_millis(millis as u64));
                            }
                            if !right_cancel.is_cancelled() {
                                let _ = tx.send((1, Ok(Value::Unit)));
                            }
                        }
                    });

                    let (winner, result) = loop {
                        runtime.check_cancelled()?;
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
                        result
                    } else {
                        left_cancel.cancel();
                        Err(RuntimeError::Error(timeout_error.clone()))
                    }
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "cancelToken".to_string(),
        builtin("concurrent.cancelToken", 1, |_, _| Ok(make_cancel_token())),
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
