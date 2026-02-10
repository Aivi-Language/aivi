use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::thread;

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, StatusCode};
use tokio::runtime::{Handle, Runtime};
use tokio::sync::{oneshot, Mutex as TokioMutex};
use tokio_tungstenite::WebSocketStream;

pub struct AiviRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub remote_addr: Option<String>,
}

pub struct AiviResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct AiviHttpError {
    pub message: String,
}

pub enum AiviWsMessage {
    Text(String),
    Binary(Vec<u8>),
    Ping,
    Pong,
    Close,
}

pub type HandlerFuture = Pin<Box<dyn Future<Output = Result<ServerReply, AiviHttpError>> + Send>>;
pub type Handler = Arc<dyn Fn(AiviRequest) -> HandlerFuture + Send + Sync>;
pub type WsHandlerFuture = Pin<Box<dyn Future<Output = Result<(), AiviHttpError>> + Send>>;
pub type WsHandler = Arc<dyn Fn(WebSocketHandle) -> WsHandlerFuture + Send + Sync>;

pub enum ServerReply {
    Http(AiviResponse),
    Ws(WsHandler),
}

pub struct ServerHandle {
    runtime: Arc<Runtime>,
    shutdown_tx: Mutex<Option<oneshot::Sender<()>>>,
    join_handle: Mutex<Option<thread::JoinHandle<()>>>,
}

impl ServerHandle {
    pub fn runtime_handle(&self) -> Handle {
        self.runtime.handle().clone()
    }

    pub fn stop(&self) -> Result<(), AiviHttpError> {
        if let Ok(mut guard) = self.shutdown_tx.lock() {
            if let Some(tx) = guard.take() {
                let _ = tx.send(());
            }
        }
        if let Ok(mut guard) = self.join_handle.lock() {
            if let Some(handle) = guard.take() {
                handle
                    .join()
                    .map_err(|_| AiviHttpError { message: "server thread panicked".to_string() })?;
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct WebSocketHandle {
    runtime: Handle,
    socket: Arc<TokioMutex<WebSocketStream<hyper::upgrade::Upgraded>>>,
}

impl WebSocketHandle {
    fn new(runtime: Handle, socket: WebSocketStream<hyper::upgrade::Upgraded>) -> Self {
        Self {
            runtime,
            socket: Arc::new(TokioMutex::new(socket)),
        }
    }

    pub fn recv(&self) -> Result<AiviWsMessage, AiviHttpError> {
        let socket = self.socket.clone();
        let handle = self.runtime.clone();
        handle.block_on(async move {
            let mut socket = socket.lock().await;
            match socket.next().await {
                Some(Ok(msg)) => Ok(map_ws_message(msg)),
                Some(Err(err)) => Err(AiviHttpError { message: err.to_string() }),
                None => Ok(AiviWsMessage::Close),
            }
        })
    }

    pub fn send(&self, msg: AiviWsMessage) -> Result<(), AiviHttpError> {
        let socket = self.socket.clone();
        let handle = self.runtime.clone();
        handle.block_on(async move {
            let mut socket = socket.lock().await;
            let msg = to_ws_message(msg);
            socket.send(msg).await.map_err(|err| AiviHttpError { message: err.to_string() })
        })
    }

    pub fn close(&self) -> Result<(), AiviHttpError> {
        let socket = self.socket.clone();
        let handle = self.runtime.clone();
        handle.block_on(async move {
            let mut socket = socket.lock().await;
            socket
                .send(tokio_tungstenite::tungstenite::Message::Close(None))
                .await
                .map_err(|err| AiviHttpError { message: err.to_string() })
        })
    }
}

pub fn start_server(addr: SocketAddr, handler: Handler) -> Result<ServerHandle, AiviHttpError> {
    let worker_threads = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1);
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(worker_threads)
            .enable_all()
            .build()
            .map_err(|err| AiviHttpError { message: err.to_string() })?,
    );
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let runtime_clone = runtime.clone();
    let join_handle = thread::spawn(move || {
        let handler = handler.clone();
        let runtime_handle = runtime_clone.handle().clone();
        let make_service = make_service_fn(move |conn: &AddrStream| {
            let handler = handler.clone();
            let runtime_handle = runtime_handle.clone();
            let remote_addr = conn.remote_addr();
            async move {
                Ok::<_, hyper::Error>(service_fn(move |req| {
                    let handler = handler.clone();
                    let runtime_handle = runtime_handle.clone();
                    let remote_addr = remote_addr;
                    async move { handle_request(req, remote_addr, handler, runtime_handle).await }
                }))
            }
        });

        let server = hyper::Server::bind(&addr).serve(make_service);
        let graceful = server.with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        let _ = runtime_clone.block_on(graceful);
    });

    Ok(ServerHandle {
        runtime,
        shutdown_tx: Mutex::new(Some(shutdown_tx)),
        join_handle: Mutex::new(Some(join_handle)),
    })
}

async fn handle_request(
    req: Request<Body>,
    remote_addr: SocketAddr,
    handler: Handler,
    runtime_handle: Handle,
) -> Result<Response<Body>, hyper::Error> {
    let is_upgrade = hyper_tungstenite::is_upgrade_request(&req);
    let (parts, body) = req.into_parts();

    let body_bytes = if is_upgrade {
        Bytes::new()
    } else {
        hyper::body::to_bytes(body).await?
    };

    let request = match build_request(&parts, body_bytes, Some(remote_addr.to_string())) {
        Ok(value) => value,
        Err(err) => {
            let mut response = Response::new(Body::from(err.message));
            *response.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(response);
        }
    };
    let reply = match handler(request).await {
        Ok(value) => value,
        Err(err) => {
            let mut response = Response::new(Body::from(err.message));
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            return Ok(response);
        }
    };

    match reply {
        ServerReply::Http(response) => match convert_response(response) {
            Ok(response) => Ok(response),
            Err(err) => {
                let mut response = Response::new(Body::from(err.message));
                *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                Ok(response)
            }
        },
        ServerReply::Ws(ws_handler) => {
            if !is_upgrade {
                let mut response = Response::new(Body::from("upgrade required"));
                *response.status_mut() = StatusCode::BAD_REQUEST;
                return Ok(response);
            }
            let req = Request::from_parts(parts, Body::empty());
            match hyper_tungstenite::upgrade(req, None) {
                Ok((response, websocket)) => {
                    let runtime_handle = runtime_handle.clone();
                    tokio::spawn(async move {
                        match websocket.await {
                            Ok(ws_stream) => {
                                let ws_handle = WebSocketHandle::new(runtime_handle, ws_stream);
                                let _ = ws_handler(ws_handle).await;
                            }
                            Err(_) => {}
                        }
                    });
                    Ok(response)
                }
                Err(_) => {
                    let mut response = Response::new(Body::from("upgrade failed"));
                    *response.status_mut() = StatusCode::BAD_REQUEST;
                    Ok(response)
                }
            }
        }
    }
}

fn build_request(
    parts: &hyper::http::request::Parts,
    body: Bytes,
    remote_addr: Option<String>,
) -> Result<AiviRequest, AiviHttpError> {
    let method = parts.method.as_str().to_string();
    let path = parts
        .uri
        .path_and_query()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());
    let headers = headers_to_vec(&parts.headers)?;
    Ok(AiviRequest {
        method,
        path,
        headers,
        body: body.to_vec(),
        remote_addr,
    })
}

fn headers_to_vec(
    headers: &hyper::HeaderMap<hyper::header::HeaderValue>,
) -> Result<Vec<(String, String)>, AiviHttpError> {
    let mut out = Vec::new();
    for (name, value) in headers.iter() {
        let value = value.to_str().map_err(|_| AiviHttpError {
            message: "invalid header value".to_string(),
        })?;
        out.push((name.as_str().to_string(), value.to_string()));
    }
    Ok(out)
}

fn convert_response(response: AiviResponse) -> Result<Response<Body>, AiviHttpError> {
    let status = StatusCode::from_u16(response.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut builder = Response::builder().status(status);
    {
        let headers = builder.headers_mut().ok_or_else(|| AiviHttpError {
            message: "failed to access headers".to_string(),
        })?;
        for (name, value) in response.headers {
            let name = hyper::header::HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
                AiviHttpError {
                    message: "invalid header name".to_string(),
                }
            })?;
            let value = hyper::header::HeaderValue::from_str(&value).map_err(|_| AiviHttpError {
                message: "invalid header value".to_string(),
            })?;
            headers.append(name, value);
        }
    }
    builder
        .body(Body::from(response.body))
        .map_err(|_| AiviHttpError {
            message: "invalid response body".to_string(),
        })
}

fn map_ws_message(msg: tokio_tungstenite::tungstenite::Message) -> AiviWsMessage {
    match msg {
        tokio_tungstenite::tungstenite::Message::Text(text) => AiviWsMessage::Text(text),
        tokio_tungstenite::tungstenite::Message::Binary(data) => AiviWsMessage::Binary(data),
        tokio_tungstenite::tungstenite::Message::Ping(_) => AiviWsMessage::Ping,
        tokio_tungstenite::tungstenite::Message::Pong(_) => AiviWsMessage::Pong,
        tokio_tungstenite::tungstenite::Message::Close(_) => AiviWsMessage::Close,
        _ => AiviWsMessage::Close,
    }
}

fn to_ws_message(msg: AiviWsMessage) -> tokio_tungstenite::tungstenite::Message {
    match msg {
        AiviWsMessage::Text(text) => tokio_tungstenite::tungstenite::Message::Text(text),
        AiviWsMessage::Binary(data) => tokio_tungstenite::tungstenite::Message::Binary(data),
        AiviWsMessage::Ping => tokio_tungstenite::tungstenite::Message::Ping(Vec::new()),
        AiviWsMessage::Pong => tokio_tungstenite::tungstenite::Message::Pong(Vec::new()),
        AiviWsMessage::Close => tokio_tungstenite::tungstenite::Message::Close(None),
    }
}
