use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use super::sockets::connection_from_value;
use super::util::{builtin, expect_int};
use crate::values::{StreamHandle, StreamState};
use crate::{EffectValue, RuntimeError, Value};

const DEFAULT_STREAM_CHUNK: usize = 4096;

fn stream_error_value(message: impl Into<String>) -> Value {
    let mut fields = HashMap::new();
    fields.insert("message".to_string(), Value::Text(message.into()));
    Value::Record(Arc::new(fields))
}

fn stream_from_value(value: Value, ctx: &str) -> Result<Arc<StreamHandle>, RuntimeError> {
    match value {
        Value::Stream(handle) => Ok(handle),
        _ => Err(RuntimeError::Message(format!("{ctx} expects a stream"))),
    }
}

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
    }
}

pub(super) fn build_streams_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "fromSocket".to_string(),
        builtin("streams.fromSocket", 1, |mut args, _| {
            let conn = connection_from_value(args.pop().unwrap(), "streams.fromSocket")?;
            let handle = StreamHandle {
                state: Mutex::new(StreamState::Socket {
                    stream: conn,
                    chunk_size: DEFAULT_STREAM_CHUNK,
                }),
            };
            Ok(Value::Stream(Arc::new(handle)))
        }),
    );
    fields.insert(
        "toSocket".to_string(),
        builtin("streams.toSocket", 2, |mut args, _| {
            let stream = stream_from_value(args.pop().unwrap(), "streams.toSocket")?;
            let conn = connection_from_value(args.pop().unwrap(), "streams.toSocket")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut socket = conn
                        .lock()
                        .map_err(|_| RuntimeError::Message("connection poisoned".to_string()))?;
                    while let Some(chunk) = next_chunk(&stream)? {
                        socket.write_all(&chunk).map_err(|err| {
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
            let handle = StreamHandle {
                state: Mutex::new(StreamState::Chunks {
                    source: stream,
                    size,
                    buffer: Vec::new(),
                }),
            };
            Ok(Value::Stream(Arc::new(handle)))
        }),
    );
    Value::Record(Arc::new(fields))
}
