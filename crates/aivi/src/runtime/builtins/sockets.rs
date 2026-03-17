use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};

use super::util::{builtin, expect_int, expect_list, expect_record, list_value};
use crate::runtime::{format_runtime_error, EffectValue, RuntimeError, Value};

const DEFAULT_RECV_CHUNK: usize = 4096;

fn socket_error_value(message: impl Into<String>) -> Value {
    let mut fields = HashMap::new();
    fields.insert("message".to_string(), Value::Text(message.into()));
    Value::Record(Arc::new(fields))
}

fn socket_error(message: impl Into<String>) -> RuntimeError {
    RuntimeError::Error(socket_error_value(message))
}

fn socket_error_from_runtime(err: RuntimeError) -> RuntimeError {
    match err {
        RuntimeError::Error(value) => RuntimeError::Error(value),
        RuntimeError::Cancelled => RuntimeError::Cancelled,
        RuntimeError::InvalidArgument { reason, .. } => socket_error(reason),
        RuntimeError::IOError { cause, .. } => socket_error(cause),
        RuntimeError::Message(message) => socket_error(message),
        other => socket_error(format_runtime_error(other)),
    }
}

fn address_from_value(value: Value, ctx: &str) -> Result<SocketAddr, RuntimeError> {
    let record = expect_record(value, ctx)?;
    let host = match record.get("host") {
        Some(Value::Text(text)) => text.clone(),
        _ => {
            return Err(RuntimeError::InvalidArgument {
                context: ctx.to_string(),
                reason: "missing field 'host' (Text) on Address".to_string(),
            });
        }
    };
    let port = match record.get("port") {
        Some(Value::Int(value)) => *value,
        _ => {
            return Err(RuntimeError::InvalidArgument {
                context: ctx.to_string(),
                reason: "missing field 'port' (Int) on Address".to_string(),
            });
        }
    };
    let port = u16::try_from(port).map_err(|_| RuntimeError::InvalidArgument {
        context: ctx.to_string(),
        reason: "Address.port must be in 0..65535".to_string(),
    })?;
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, port));
    }

    let addr = format!("{host}:{port}");
    addr.to_socket_addrs()
        .map_err(|_| RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: "invalid address".to_string(),
        })?
        .next()
        .ok_or_else(|| RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: "invalid address".to_string(),
        })
}

pub(super) fn connection_from_value(
    value: Value,
    ctx: &str,
) -> Result<Arc<Mutex<TcpStream>>, RuntimeError> {
    match value {
        Value::Connection(handle) => Ok(handle),
        _ => Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "Connection".to_string(),
            got: "other".to_string(),
        }),
    }
}

fn listener_from_value(
    value: Value,
    ctx: &str,
) -> Result<Arc<Mutex<Option<TcpListener>>>, RuntimeError> {
    match value {
        Value::Listener(handle) => Ok(handle),
        _ => Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "Listener".to_string(),
            got: "other".to_string(),
        }),
    }
}

fn list_int_to_bytes(value: Value, ctx: &str) -> Result<Vec<u8>, RuntimeError> {
    let items = expect_list(value, ctx)?;
    let mut out = Vec::with_capacity(items.len());
    for item in items.iter() {
        let value = expect_int(item.clone(), ctx)?;
        let byte = u8::try_from(value).map_err(|_| RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: "byte value must be in 0..255".to_string(),
        })?;
        out.push(byte);
    }
    Ok(out)
}

pub(super) fn build_sockets_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "listen".to_string(),
        builtin("sockets.listen", 1, |mut args, _| {
            let address = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let addr = address_from_value(address.clone(), "sockets.listen")
                        .map_err(socket_error_from_runtime)?;
                    let listener = TcpListener::bind(addr)
                        .map_err(|err| socket_error(err.to_string()))?;
                    Ok(Value::Listener(Arc::new(Mutex::new(Some(listener)))))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "accept".to_string(),
        builtin("sockets.accept", 1, |mut args, _| {
            let listener_lock = listener_from_value(args.pop().unwrap(), "sockets.accept")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let listener = listener_lock
                        .lock()
                        .map_err(|_| socket_error("listener lock poisoned"))?;
                    let listener = listener
                        .as_ref()
                        .ok_or_else(|| socket_error("listener closed"))?;
                    let (stream, _) = listener
                        .accept()
                        .map_err(|err| socket_error(err.to_string()))?;
                    Ok(Value::Connection(Arc::new(Mutex::new(stream))))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "connect".to_string(),
        builtin("sockets.connect", 1, |mut args, _| {
            let address = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let addr = address_from_value(address.clone(), "sockets.connect")
                        .map_err(socket_error_from_runtime)?;
                    let stream = TcpStream::connect(addr)
                        .map_err(|err| socket_error(err.to_string()))?;
                    Ok(Value::Connection(Arc::new(Mutex::new(stream))))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "send".to_string(),
        builtin("sockets.send", 2, |mut args, _| {
            let bytes_value = args.pop().unwrap();
            let conn = connection_from_value(args.pop().unwrap(), "sockets.send")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let bytes = list_int_to_bytes(bytes_value.clone(), "sockets.send")
                        .map_err(socket_error_from_runtime)?;
                    let mut stream = conn
                        .lock()
                        .map_err(|_| socket_error("connection poisoned"))?;
                    stream
                        .write_all(&bytes)
                        .map_err(|err| socket_error(err.to_string()))?;
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
                        .map_err(|_| socket_error("connection poisoned"))?;
                    let mut buffer = vec![0u8; DEFAULT_RECV_CHUNK];
                    let count = stream
                        .read(&mut buffer)
                        .map_err(|err| socket_error(err.to_string()))?;
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
                        .map_err(|_| socket_error("connection poisoned"))?;
                    stream
                        .shutdown(Shutdown::Both)
                        .map_err(|err| socket_error(err.to_string()))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "closeListener".to_string(),
        builtin("sockets.closeListener", 1, |mut args, _| {
            let listener_lock = listener_from_value(args.pop().unwrap(), "sockets.closeListener")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut listener = listener_lock
                        .lock()
                        .map_err(|_| socket_error("listener lock poisoned"))?;
                    *listener = None;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(Arc::new(fields))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(host: &str, port: i64) -> Value {
        let mut fields = HashMap::new();
        fields.insert("host".to_string(), Value::Text(host.to_string()));
        fields.insert("port".to_string(), Value::Int(port));
        Value::Record(Arc::new(fields))
    }

    #[test]
    fn address_from_value_supports_ipv4_and_hostnames() {
        let loopback = match address_from_value(address("127.0.0.1", 80), "ctx") {
            Ok(value) => value,
            Err(_) => panic!("ipv4"),
        };
        assert_eq!(loopback, "127.0.0.1:80".parse().unwrap());

        let localhost = match address_from_value(address("localhost", 8080), "ctx") {
            Ok(value) => value,
            Err(_) => panic!("hostname"),
        };
        assert_eq!(localhost.port(), 8080);
    }

    #[test]
    fn address_from_value_supports_ipv6_literals() {
        let addr = match address_from_value(address("::1", 443), "ctx") {
            Ok(value) => value,
            Err(_) => panic!("ipv6"),
        };
        assert_eq!(addr, "[::1]:443".parse().unwrap());
    }

    #[test]
    fn address_from_value_rejects_out_of_range_ports() {
        let err = address_from_value(address("127.0.0.1", 70000), "ctx").unwrap_err();
        assert!(matches!(
            err,
            RuntimeError::InvalidArgument { reason, .. } if reason == "Address.port must be in 0..65535"
        ));
    }

    #[test]
    fn list_int_to_bytes_rejects_out_of_range_bytes() {
        let err = list_int_to_bytes(Value::List(Arc::new(vec![Value::Int(256)])), "ctx").unwrap_err();
        assert!(matches!(
            err,
            RuntimeError::InvalidArgument { reason, .. } if reason == "byte value must be in 0..255"
        ));
    }
}
