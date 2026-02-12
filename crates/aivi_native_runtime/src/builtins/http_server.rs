use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use aivi_http_server::{
    AiviHttpError, AiviRequest, AiviResponse, AiviWsMessage, Handler, ServerReply, WebSocketHandle,
    WsHandlerFuture,
};

use super::util::{builtin, expect_record};
use crate::values::CancelToken;
use crate::{format_value, EffectValue, Runtime, RuntimeContext, RuntimeError, Value};

pub(super) fn build_http_server_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "listen".to_string(),
        builtin("httpServer.listen", 2, |mut args, runtime| {
            let handler = args.pop().unwrap();
            let config = args.pop().unwrap();
            let addr = parse_server_config(config)?;
            let ctx = runtime.ctx.clone();
            let handler_value = handler.clone();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let handler_value_clone = handler_value.clone();
                    let ctx_clone = ctx.clone();
                    let handler: Handler = Arc::new(move |req: AiviRequest| {
                        let handler_value = handler_value_clone.clone();
                        let ctx = ctx_clone.clone();
                        Box::pin(async move {
                            let req_value = request_to_value(req);
                            let ctx_for_reply = ctx.clone();
                            let result = tokio::task::spawn_blocking(move || {
                                let cancel = CancelToken::root();
                                let mut runtime =
                                    Runtime::with_cancel(ctx.clone(), cancel);
                                let applied = runtime.apply(handler_value, req_value)?;
                                runtime.run_effect_value(applied)
                            })
                            .await
                            .map_err(|err| AiviHttpError {
                                message: err.to_string(),
                            })?;
                            match result {
                                Ok(value) => server_reply_from_value(value, ctx_for_reply),
                                Err(err) => Err(runtime_error_to_http_error(err)),
                            }
                        })
                    });
                    let server = aivi_http_server::start_server(addr, handler)
                        .map_err(|err| RuntimeError::Error(http_error_value(err.message)))?;
                    Ok(Value::HttpServer(Arc::new(server)))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "stop".to_string(),
        builtin("httpServer.stop", 1, |mut args, _| {
            let server = match args.pop().unwrap() {
                Value::HttpServer(handle) => handle,
                other => {
                    return Err(RuntimeError::Message(format!(
                        "httpServer.stop expects a server handle, got {}",
                        format_value(&other)
                    )))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    server
                        .stop()
                        .map_err(|err| RuntimeError::Error(http_error_value(err.message)))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "ws_recv".to_string(),
        builtin("httpServer.ws_recv", 1, |mut args, _| {
            let socket = match args.pop().unwrap() {
                Value::WebSocket(handle) => handle,
                other => {
                    return Err(RuntimeError::Message(format!(
                        "httpServer.ws_recv expects a websocket, got {}",
                        format_value(&other)
                    )))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let msg = socket
                        .recv()
                        .map_err(|err| RuntimeError::Error(ws_error_value(err.message)))?;
                    Ok(ws_message_to_value(msg))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "ws_send".to_string(),
        builtin("httpServer.ws_send", 2, |mut args, _| {
            let message = args.pop().unwrap();
            let socket = match args.pop().unwrap() {
                Value::WebSocket(handle) => handle,
                other => {
                    return Err(RuntimeError::Message(format!(
                        "httpServer.ws_send expects a websocket, got {}",
                        format_value(&other)
                    )))
                }
            };
            let ws_message = value_to_ws_message(message)?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    socket
                        .send(ws_message.clone())
                        .map_err(|err| RuntimeError::Error(ws_error_value(err.message)))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "ws_close".to_string(),
        builtin("httpServer.ws_close", 1, |mut args, _| {
            let socket = match args.pop().unwrap() {
                Value::WebSocket(handle) => handle,
                other => {
                    return Err(RuntimeError::Message(format!(
                        "httpServer.ws_close expects a websocket, got {}",
                        format_value(&other)
                    )))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    socket
                        .close()
                        .map_err(|err| RuntimeError::Error(ws_error_value(err.message)))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn parse_server_config(value: Value) -> Result<SocketAddr, RuntimeError> {
    let record = expect_record(value, "httpServer.listen expects ServerConfig")?;
    let address = match record.get("address") {
        Some(Value::Text(text)) => text.clone(),
        _ => {
            return Err(RuntimeError::Message(
                "httpServer.listen expects ServerConfig.address Text".to_string(),
            ))
        }
    };
    address.parse().map_err(|_| {
        RuntimeError::Message("httpServer.listen address must be host:port".to_string())
    })
}

fn request_to_value(req: AiviRequest) -> Value {
    let mut fields = HashMap::new();
    fields.insert("method".to_string(), Value::Text(req.method));
    fields.insert("path".to_string(), Value::Text(req.path));
    fields.insert("headers".to_string(), headers_to_value(req.headers));
    fields.insert("body".to_string(), bytes_to_list_value(req.body));
    fields.insert(
        "remoteAddr".to_string(),
        match req.remote_addr {
            Some(value) => Value::Constructor {
                name: "Some".to_string(),
                args: vec![Value::Text(value)],
            },
            None => Value::Constructor {
                name: "None".to_string(),
                args: Vec::new(),
            },
        },
    );
    Value::Record(Arc::new(fields))
}

fn server_reply_from_value(
    value: Value,
    ctx: Arc<RuntimeContext>,
) -> Result<ServerReply, AiviHttpError> {
    match value {
        Value::Constructor { name, mut args } if name == "Http" => {
            if args.len() != 1 {
                return Err(AiviHttpError {
                    message: "Http response expects 1 argument".to_string(),
                });
            }
            let response_value = args.pop().unwrap();
            let response = response_from_value(response_value)?;
            Ok(ServerReply::Http(response))
        }
        Value::Constructor { name, mut args } if name == "Ws" => {
            if args.len() != 1 {
                return Err(AiviHttpError {
                    message: "Ws response expects 1 argument".to_string(),
                });
            }
            let ws_handler_value = args.pop().unwrap();
            let ws_handler = Arc::new(move |socket: WebSocketHandle| {
                let ctx = ctx.clone();
                let ws_handler_value = ws_handler_value.clone();
                let future: WsHandlerFuture = Box::pin(async move {
                    let socket_value = Value::WebSocket(Arc::new(socket));
                    let result = tokio::task::spawn_blocking(move || {
                        let cancel = CancelToken::root();
                        let mut runtime = Runtime::with_cancel(ctx.clone(), cancel);
                        let applied = runtime.apply(ws_handler_value, socket_value)?;
                        runtime.run_effect_value(applied)
                    })
                    .await
                    .map_err(|err| AiviHttpError {
                        message: err.to_string(),
                    })?;
                    match result {
                        Ok(Value::Unit) => Ok(()),
                        Ok(_) => Ok(()),
                        Err(err) => Err(runtime_error_to_http_error(err)),
                    }
                });
                future
            });
            Ok(ServerReply::Ws(ws_handler))
        }
        other => Err(AiviHttpError {
            message: format!(
                "expected ServerReply (Http|Ws), got {}",
                format_value(&other)
            ),
        }),
    }
}

fn response_from_value(value: Value) -> Result<AiviResponse, AiviHttpError> {
    let record = expect_record(value, "Response must be a record")
        .map_err(runtime_error_to_http_error)?;
    let status = match record.get("status") {
        Some(Value::Int(value)) => *value,
        _ => {
            return Err(AiviHttpError {
                message: "Response.status must be Int".to_string(),
            })
        }
    };
    let headers = match record.get("headers") {
        Some(Value::List(items)) => headers_from_value(items.as_ref())?,
        _ => {
            return Err(AiviHttpError {
                message: "Response.headers must be List".to_string(),
            })
        }
    };
    let body = match record.get("body") {
        Some(Value::List(items)) => list_value_to_bytes(items.as_ref())?,
        _ => {
            return Err(AiviHttpError {
                message: "Response.body must be List".to_string(),
            })
        }
    };
    Ok(AiviResponse {
        status: status as u16,
        headers,
        body,
    })
}

fn headers_to_value(headers: Vec<(String, String)>) -> Value {
    let mut out = Vec::new();
    for (k, v) in headers {
        out.push(Value::Tuple(vec![Value::Text(k), Value::Text(v)]));
    }
    Value::List(Arc::new(out))
}

fn headers_from_value(values: &[Value]) -> Result<Vec<(String, String)>, AiviHttpError> {
    let mut out = Vec::new();
    for value in values {
        match value {
            Value::Tuple(items) if items.len() == 2 => {
                let key = match &items[0] {
                    Value::Text(text) => text.clone(),
                    other => {
                        return Err(AiviHttpError {
                            message: format!("header key must be Text, got {}", format_value(other)),
                        })
                    }
                };
                let val = match &items[1] {
                    Value::Text(text) => text.clone(),
                    other => {
                        return Err(AiviHttpError {
                            message: format!("header value must be Text, got {}", format_value(other)),
                        })
                    }
                };
                out.push((key, val));
            }
            other => {
                return Err(AiviHttpError {
                    message: format!("headers must be List (Text, Text), got {}", format_value(other)),
                })
            }
        }
    }
    Ok(out)
}

fn bytes_to_list_value(bytes: Vec<u8>) -> Value {
    Value::List(Arc::new(bytes.into_iter().map(|b| Value::Int(b as i64)).collect()))
}

fn list_value_to_bytes(values: &[Value]) -> Result<Vec<u8>, AiviHttpError> {
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        match value {
            Value::Int(n) if *n >= 0 && *n <= 255 => out.push(*n as u8),
            other => {
                return Err(AiviHttpError {
                    message: format!("expected bytes (0..255), got {}", format_value(other)),
                })
            }
        }
    }
    Ok(out)
}

fn ws_message_to_value(msg: AiviWsMessage) -> Value {
    match msg {
        AiviWsMessage::TextMsg(text) => Value::Constructor {
            name: "WsText".to_string(),
            args: vec![Value::Text(text)],
        },
        AiviWsMessage::BinaryMsg(bytes) => Value::Constructor {
            name: "WsBinary".to_string(),
            args: vec![bytes_to_list_value(bytes)],
        },
        AiviWsMessage::Ping => Value::Constructor {
            name: "WsPing".to_string(),
            args: Vec::new(),
        },
        AiviWsMessage::Pong => Value::Constructor {
            name: "WsPong".to_string(),
            args: Vec::new(),
        },
        AiviWsMessage::Close => Value::Constructor {
            name: "WsClose".to_string(),
            args: Vec::new(),
        },
    }
}

fn value_to_ws_message(value: Value) -> Result<AiviWsMessage, RuntimeError> {
    match value {
        Value::Constructor { name, mut args } if name == "WsText" && args.len() == 1 => {
            match args.pop().unwrap() {
                Value::Text(text) => Ok(AiviWsMessage::TextMsg(text)),
                other => Err(RuntimeError::Message(format!(
                    "WsText expects Text, got {}",
                    format_value(&other)
                ))),
            }
        }
        Value::Constructor { name, mut args } if name == "WsBinary" && args.len() == 1 => {
            let bytes = match args.pop().unwrap() {
                Value::List(items) => list_value_to_bytes(items.as_ref())
                    .map_err(|err| RuntimeError::Message(err.message))?,
                other => {
                    return Err(RuntimeError::Message(format!(
                        "WsBinary expects Bytes list, got {}",
                        format_value(&other)
                    )))
                }
            };
            Ok(AiviWsMessage::BinaryMsg(bytes))
        }
        Value::Constructor { name, args } if name == "WsPing" && args.is_empty() => {
            Ok(AiviWsMessage::Ping)
        }
        Value::Constructor { name, args } if name == "WsPong" && args.is_empty() => {
            Ok(AiviWsMessage::Pong)
        }
        Value::Constructor { name, args } if name == "WsClose" && args.is_empty() => {
            Ok(AiviWsMessage::Close)
        }
        other => Err(RuntimeError::Message(format!(
            "expected WsMessage, got {}",
            format_value(&other)
        ))),
    }
}

fn http_error_value(message: impl Into<String>) -> Value {
    let mut fields = HashMap::new();
    fields.insert("message".to_string(), Value::Text(message.into()));
    Value::Record(Arc::new(fields))
}

fn ws_error_value(message: impl Into<String>) -> Value {
    let mut fields = HashMap::new();
    fields.insert("message".to_string(), Value::Text(message.into()));
    Value::Record(Arc::new(fields))
}

fn runtime_error_to_http_error(err: RuntimeError) -> AiviHttpError {
    AiviHttpError {
        message: err.to_string(),
    }
}
