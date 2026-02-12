use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use super::util::{builtin, expect_int, expect_list, expect_record, list_value};
use crate::runtime::{EffectValue, RuntimeError, Value};

const DEFAULT_RECV_CHUNK: usize = 4096;

fn socket_error_value(message: impl Into<String>) -> Value {
    let mut fields = HashMap::new();
    fields.insert("message".to_string(), Value::Text(message.into()));
    Value::Record(Arc::new(fields))
}

fn address_from_value(value: Value, ctx: &str) -> Result<SocketAddr, RuntimeError> {
    let record = expect_record(value, ctx)?;
    let host = match record.get("host") {
        Some(Value::Text(text)) => text.clone(),
        _ => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Address.host Text"
            )))
        }
    };
    let port = match record.get("port") {
        Some(Value::Int(value)) => *value,
        _ => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Address.port Int"
            )))
        }
    };
    let port = u16::try_from(port)
        .map_err(|_| RuntimeError::Message(format!("{ctx} expects Address.port in 0..65535")))?;
    let addr = format!("{host}:{port}");
    addr.parse()
        .map_err(|_| RuntimeError::Message(format!("{ctx} invalid address")))
}

pub(super) fn connection_from_value(
    value: Value,
    ctx: &str,
) -> Result<Arc<Mutex<TcpStream>>, RuntimeError> {
    match value {
        Value::Connection(handle) => Ok(handle),
        _ => Err(RuntimeError::Message(format!("{ctx} expects a connection"))),
    }
}

fn listener_from_value(value: Value, ctx: &str) -> Result<Arc<TcpListener>, RuntimeError> {
    match value {
        Value::Listener(handle) => Ok(handle),
        _ => Err(RuntimeError::Message(format!("{ctx} expects a listener"))),
    }
}

fn list_int_to_bytes(value: Value, ctx: &str) -> Result<Vec<u8>, RuntimeError> {
    let items = expect_list(value, ctx)?;
    let mut out = Vec::with_capacity(items.len());
    for item in items.iter() {
        let value = expect_int(item.clone(), ctx)?;
        let byte = u8::try_from(value)
            .map_err(|_| RuntimeError::Message(format!("{ctx} expects bytes in 0..255")))?;
        out.push(byte);
    }
    Ok(out)
}

pub(super) fn build_sockets_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "listen".to_string(),
        builtin("sockets.listen", 1, |mut args, _| {
            let addr = address_from_value(args.pop().unwrap(), "sockets.listen")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let listener = TcpListener::bind(addr)
                        .map_err(|err| RuntimeError::Error(socket_error_value(err.to_string())))?;
                    Ok(Value::Listener(Arc::new(listener)))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "accept".to_string(),
        builtin("sockets.accept", 1, |mut args, _| {
            let listener = listener_from_value(args.pop().unwrap(), "sockets.accept")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let (stream, _) = listener
                        .accept()
                        .map_err(|err| RuntimeError::Error(socket_error_value(err.to_string())))?;
                    Ok(Value::Connection(Arc::new(Mutex::new(stream))))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "connect".to_string(),
        builtin("sockets.connect", 1, |mut args, _| {
            let addr = address_from_value(args.pop().unwrap(), "sockets.connect")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let stream = TcpStream::connect(addr)
                        .map_err(|err| RuntimeError::Error(socket_error_value(err.to_string())))?;
                    Ok(Value::Connection(Arc::new(Mutex::new(stream))))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "send".to_string(),
        builtin("sockets.send", 2, |mut args, _| {
            let bytes = args.pop().unwrap();
            let conn = connection_from_value(args.pop().unwrap(), "sockets.send")?;
            let bytes = list_int_to_bytes(bytes, "sockets.send")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut stream = conn
                        .lock()
                        .map_err(|_| RuntimeError::Message("connection poisoned".to_string()))?;
                    stream
                        .write_all(&bytes)
                        .map_err(|err| RuntimeError::Error(socket_error_value(err.to_string())))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "recv".to_string(),
        builtin("sockets.recv", 1, |mut args, _| {
            let conn = connection_from_value(args.pop().unwrap(), "sockets.recv")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut stream = conn
                        .lock()
                        .map_err(|_| RuntimeError::Message("connection poisoned".to_string()))?;
                    let mut buffer = vec![0u8; DEFAULT_RECV_CHUNK];
                    let count = stream
                        .read(&mut buffer)
                        .map_err(|err| RuntimeError::Error(socket_error_value(err.to_string())))?;
                    buffer.truncate(count);
                    let items = buffer
                        .into_iter()
                        .map(|byte| Value::Int(byte as i64))
                        .collect::<Vec<_>>();
                    Ok(list_value(items))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "close".to_string(),
        builtin("sockets.close", 1, |mut args, _| {
            let conn = connection_from_value(args.pop().unwrap(), "sockets.close")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let stream = conn
                        .lock()
                        .map_err(|_| RuntimeError::Message("connection poisoned".to_string()))?;
                    let _ = stream.shutdown(Shutdown::Both);
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "closeListener".to_string(),
        builtin("sockets.closeListener", 1, |mut args, _| {
            let _listener = listener_from_value(args.pop().unwrap(), "sockets.closeListener")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| Ok(Value::Unit)),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(Arc::new(fields))
}
