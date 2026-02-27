use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use super::sockets::connection_from_value;
use super::util::{builtin, expect_int, expect_list, list_value};
use crate::runtime::values::{EffectValue, StreamHandle, StreamState};
use crate::runtime::{format_value, Runtime, RuntimeError, Value};

const DEFAULT_STREAM_CHUNK: usize = 4096;

fn stream_error_value(message: impl Into<String>) -> Value {
    let mut fields = HashMap::new();
    fields.insert("message".to_string(), Value::Text(message.into()));
    Value::Record(Arc::new(fields))
}

pub(super) fn stream_from_value(value: Value, ctx: &str) -> Result<Arc<StreamHandle>, RuntimeError> {
    match value {
        Value::Stream(handle) => Ok(handle),
        _ => Err(RuntimeError::Message(format!("{ctx} expects a stream"))),
    }
}

fn expect_callable(value: Value, ctx: &str) -> Result<Value, RuntimeError> {
    match value {
        Value::Builtin(_) | Value::MultiClause(_) | Value::Thunk(_) => Ok(value),
        _ => Err(RuntimeError::Message(format!("{ctx} expects a function"))),
    }
}

fn expect_bool(value: Value, ctx: &str) -> Result<bool, RuntimeError> {
    match value {
        Value::Bool(b) => Ok(b),
        other => Err(RuntimeError::Message(format!(
            "{ctx} expects Bool, got {}",
            format_value(&other)
        ))),
    }
}

/// Convert raw bytes to `Value::List(List<Value::Int>)` (byte chunk as int list).
fn bytes_to_value(bytes: Vec<u8>) -> Value {
    let items: Vec<Value> = bytes.into_iter().map(|b| Value::Int(b as i64)).collect();
    list_value(items)
}

/// Convert `Value::List(List<Value::Int>)` back to raw bytes (for `toSocket`).
fn value_to_bytes(value: Value, ctx: &str) -> Result<Vec<u8>, RuntimeError> {
    let items = expect_list(value, ctx)?;
    items
        .iter()
        .map(|v| match v {
            Value::Int(i) => u8::try_from(*i).map_err(|_| {
                RuntimeError::Message(format!("{ctx}: byte value {i} out of range 0-255"))
            }),
            other => Err(RuntimeError::Message(format!(
                "{ctx}: expected List Int (byte list), got {}",
                format_value(other)
            ))),
        })
        .collect()
}

/// Read the next raw-byte chunk from a `Socket` or `Chunks` stream.
/// Only valid for `StreamState::Socket` and `StreamState::Chunks`.
fn next_chunk(handle: &Arc<StreamHandle>) -> Result<Option<Vec<u8>>, RuntimeError> {
    let mut guard = handle
        .state
        .lock()
        .map_err(|_| RuntimeError::Message("stream poisoned".to_string()))?;
    match &mut *guard {
        StreamState::Socket { stream, chunk_size } => {
            let mut stream = stream
                .lock()
                .map_err(|_| RuntimeError::Message("connection poisoned".to_string()))?;
            let mut buffer = vec![0u8; *chunk_size];
            let count = stream
                .read(&mut buffer)
                .map_err(|err| RuntimeError::Error(stream_error_value(err.to_string())))?;
            if count == 0 {
                Ok(None)
            } else {
                buffer.truncate(count);
                Ok(Some(buffer))
            }
        }
        StreamState::Chunks {
            source,
            size,
            buffer,
        } => loop {
            if buffer.len() >= *size {
                let out = buffer.drain(..*size).collect();
                return Ok(Some(out));
            }
            match next_chunk(source)? {
                Some(chunk) => buffer.extend_from_slice(&chunk),
                None => {
                    if buffer.is_empty() {
                        return Ok(None);
                    }
                    let out = buffer.split_off(0);
                    return Ok(Some(out));
                }
            }
        },
        _ => Err(RuntimeError::Message(
            "next_chunk called on non-byte stream".to_string(),
        )),
    }
}

/// Generic stream iterator: returns the next `Value` from any stream kind.
/// For `Socket`/`Chunks`, bytes are wrapped as `Value::List` of `Value::Int`.
pub(super) fn next_value(
    handle: &Arc<StreamHandle>,
    runtime: &mut Runtime,
) -> Result<Option<Value>, RuntimeError> {
    enum Action {
        Bytes,
        Map(Arc<StreamHandle>, Value),
        Filter(Arc<StreamHandle>, Value),
        Take(Arc<StreamHandle>),
        TakeDone,
        Drop(Arc<StreamHandle>),
        FlatMap {
            source: Arc<StreamHandle>,
            func: Value,
            inner: Option<Arc<StreamHandle>>,
        },
        Merge(Arc<StreamHandle>, Arc<StreamHandle>),
    }

    let action = {
        let mut guard = handle
            .state
            .lock()
            .map_err(|_| RuntimeError::Message("stream poisoned".to_string()))?;
        match &mut *guard {
            StreamState::Socket { .. } | StreamState::Chunks { .. } => Action::Bytes,
            StreamState::Values { items } => {
                return Ok(items.pop_front());
            }
            StreamState::Map { source, func } => Action::Map(source.clone(), func.clone()),
            StreamState::Filter { source, pred } => Action::Filter(source.clone(), pred.clone()),
            StreamState::Take { source, remaining } => {
                if *remaining == 0 {
                    Action::TakeDone
                } else {
                    *remaining -= 1;
                    Action::Take(source.clone())
                }
            }
            StreamState::Drop { source, .. } => Action::Drop(source.clone()),
            StreamState::FlatMap { source, func, inner } => Action::FlatMap {
                source: source.clone(),
                func: func.clone(),
                inner: inner.clone(),
            },
            StreamState::Merge { left, right } => {
                Action::Merge(left.clone(), right.clone())
            }
        }
    };

    match action {
        Action::Bytes => next_chunk(handle).map(|opt| opt.map(bytes_to_value)),        Action::Map(source, func) => match next_value(&source, runtime)? {
            None => Ok(None),
            Some(item) => Ok(Some(runtime.apply(func, item)?)),
        },
        Action::Filter(source, pred) => loop {
            match next_value(&source, runtime)? {
                None => return Ok(None),
                Some(item) => {
                    let keep = runtime.apply(pred.clone(), item.clone())?;
                    if expect_bool(keep, "streams.filter")? {
                        return Ok(Some(item));
                    }
                }
            }
        },
        Action::TakeDone => Ok(None),
        Action::Take(source) => next_value(&source, runtime),
        Action::Drop(source) => {
            loop {
                let to_skip = {
                    let guard = handle
                        .state
                        .lock()
                        .map_err(|_| RuntimeError::Message("stream poisoned".to_string()))?;
                    match &*guard {
                        StreamState::Drop { to_skip, .. } => *to_skip,
                        _ => 0,
                    }
                };
                if to_skip == 0 {
                    return next_value(&source, runtime);
                }
                match next_value(&source, runtime)? {
                    None => return Ok(None),
                    Some(_) => {
                        let mut guard = handle
                            .state
                            .lock()
                            .map_err(|_| RuntimeError::Message("stream poisoned".to_string()))?;
                        if let StreamState::Drop { to_skip: ref mut n, .. } = *guard {
                            if *n > 0 {
                                *n -= 1;
                            }
                        }
                    }
                }
            }
        }
        Action::FlatMap {
            source,
            func,
            inner: initial_inner,
        } => next_value_flatmap(handle, &source, func, initial_inner, runtime),
        Action::Merge(left, right) => match next_value(&left, runtime)? {
            Some(item) => Ok(Some(item)),
            None => next_value(&right, runtime),
        },
    }
}

fn next_value_flatmap(
    handle: &Arc<StreamHandle>,
    source: &Arc<StreamHandle>,
    func: Value,
    mut inner_opt: Option<Arc<StreamHandle>>,
    runtime: &mut Runtime,
) -> Result<Option<Value>, RuntimeError> {
    loop {
        if let Some(ref inner) = inner_opt {
            match next_value(inner, runtime)? {
                Some(item) => return Ok(Some(item)),
                None => {
                    let mut guard = handle
                        .state
                        .lock()
                        .map_err(|_| RuntimeError::Message("stream poisoned".to_string()))?;
                    if let StreamState::FlatMap { inner, .. } = &mut *guard {
                        *inner = None;
                    }
                    // inner_opt is overwritten below (either source yields Some and
                    // inner_opt = Some(new_inner), or source yields None and we return).
                    // No explicit inner_opt = None needed here.
                }
            }
        }
        match next_value(source, runtime)? {
            None => return Ok(None),
            Some(outer_item) => {
                let inner_val = runtime.apply(func.clone(), outer_item)?;
                let inner_handle = stream_from_value(inner_val, "streams.flatMap")?;
                {
                    let mut guard = handle
                        .state
                        .lock()
                        .map_err(|_| RuntimeError::Message("stream poisoned".to_string()))?;
                    if let StreamState::FlatMap { inner, .. } = &mut *guard {
                        *inner = Some(inner_handle.clone());
                    }
                }
                inner_opt = Some(inner_handle);
            }
        }
    }
}

fn make_stream(state: StreamState) -> Value {
    Value::Stream(Arc::new(StreamHandle {
        state: Mutex::new(state),
    }))
}

pub(super) fn build_streams_record() -> Value {
    let mut fields = HashMap::new();

    fields.insert(
        "fromSocket".to_string(),
        builtin("streams.fromSocket", 1, |mut args, _| {
            let conn = connection_from_value(args.pop().unwrap(), "streams.fromSocket")?;
            Ok(make_stream(StreamState::Socket {
                stream: conn,
                chunk_size: DEFAULT_STREAM_CHUNK,
            }))
        }),
    );

    fields.insert(
        "toSocket".to_string(),
        builtin("streams.toSocket", 2, |mut args, _| {
            let stream = stream_from_value(args.pop().unwrap(), "streams.toSocket")?;
            let conn = connection_from_value(args.pop().unwrap(), "streams.toSocket")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let mut socket = conn
                        .lock()
                        .map_err(|_| RuntimeError::Message("connection poisoned".to_string()))?;
                    while let Some(item) = next_value(&stream, runtime)? {
                        let bytes = value_to_bytes(item, "streams.toSocket")?;
                        socket.write_all(&bytes).map_err(|err| {
                            RuntimeError::Error(stream_error_value(err.to_string()))
                        })?;
                    }
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "chunks".to_string(),
        builtin("streams.chunks", 2, |mut args, _| {
            let stream = stream_from_value(args.pop().unwrap(), "streams.chunks")?;
            let size = expect_int(args.pop().unwrap(), "streams.chunks")?;
            let size = usize::try_from(size).map_err(|_| {
                RuntimeError::Message("streams.chunks expects positive size".to_string())
            })?;
            if size == 0 {
                return Err(RuntimeError::Message(
                    "streams.chunks expects positive size".to_string(),
                ));
            }
            Ok(make_stream(StreamState::Chunks {
                source: stream,
                size,
                buffer: Vec::new(),
            }))
        }),
    );

    // -- Combinators --

    fields.insert(
        "map".to_string(),
        builtin("streams.map", 2, |mut args, _| {
            let stream = stream_from_value(args.pop().unwrap(), "streams.map")?;
            let func = expect_callable(args.pop().unwrap(), "streams.map")?;
            Ok(make_stream(StreamState::Map {
                source: stream,
                func,
            }))
        }),
    );

    fields.insert(
        "filter".to_string(),
        builtin("streams.filter", 2, |mut args, _| {
            let stream = stream_from_value(args.pop().unwrap(), "streams.filter")?;
            let pred = expect_callable(args.pop().unwrap(), "streams.filter")?;
            Ok(make_stream(StreamState::Filter {
                source: stream,
                pred,
            }))
        }),
    );

    fields.insert(
        "take".to_string(),
        builtin("streams.take", 2, |mut args, _| {
            let stream = stream_from_value(args.pop().unwrap(), "streams.take")?;
            let n = expect_int(args.pop().unwrap(), "streams.take")?;
            let remaining = usize::try_from(n).map_err(|_| {
                RuntimeError::Message("streams.take expects non-negative count".to_string())
            })?;
            Ok(make_stream(StreamState::Take {
                source: stream,
                remaining,
            }))
        }),
    );

    fields.insert(
        "drop".to_string(),
        builtin("streams.drop", 2, |mut args, _| {
            let stream = stream_from_value(args.pop().unwrap(), "streams.drop")?;
            let n = expect_int(args.pop().unwrap(), "streams.drop")?;
            let to_skip = usize::try_from(n).map_err(|_| {
                RuntimeError::Message("streams.drop expects non-negative count".to_string())
            })?;
            Ok(make_stream(StreamState::Drop {
                source: stream,
                to_skip,
            }))
        }),
    );

    fields.insert(
        "flatMap".to_string(),
        builtin("streams.flatMap", 2, |mut args, _| {
            let stream = stream_from_value(args.pop().unwrap(), "streams.flatMap")?;
            let func = expect_callable(args.pop().unwrap(), "streams.flatMap")?;
            Ok(make_stream(StreamState::FlatMap {
                source: stream,
                func,
                inner: None,
            }))
        }),
    );

    fields.insert(
        "merge".to_string(),
        builtin("streams.merge", 2, |mut args, _| {
            let right = stream_from_value(args.pop().unwrap(), "streams.merge")?;
            let left = stream_from_value(args.pop().unwrap(), "streams.merge")?;
            Ok(make_stream(StreamState::Merge { left, right }))
        }),
    );

    fields.insert(
        "fold".to_string(),
        builtin("streams.fold", 3, |mut args, _| {
            let stream = stream_from_value(args.pop().unwrap(), "streams.fold")?;
            let seed = args.pop().unwrap();
            let step = expect_callable(args.pop().unwrap(), "streams.fold")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let mut acc = seed.clone();
                    loop {
                        match next_value(&stream, runtime)? {
                            None => return Ok(acc),
                            Some(item) => {
                                let partial = runtime.apply(step.clone(), acc)?;
                                acc = runtime.apply(partial, item)?;
                            }
                        }
                    }
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "fromList".to_string(),
        builtin("streams.fromList", 1, |mut args, _| {
            let items = expect_list(args.pop().unwrap(), "streams.fromList")?;
            let deque: VecDeque<Value> = items.iter().cloned().collect();
            Ok(make_stream(StreamState::Values { items: deque }))
        }),
    );

    Value::Record(Arc::new(fields))
}

