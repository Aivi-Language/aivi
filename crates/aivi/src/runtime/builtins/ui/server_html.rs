use std::sync::{Mutex, OnceLock};

use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ServerHtmlEventKind {
    Click,
    Input,
    KeyDown,
    KeyUp,
    PointerDown,
    PointerUp,
    PointerMove,
    Focus,
    Blur,
    TransitionEnd,
    AnimationEnd,
}

impl ServerHtmlEventKind {
    fn as_str(self) -> &'static str {
        match self {
            ServerHtmlEventKind::Click => "click",
            ServerHtmlEventKind::Input => "input",
            ServerHtmlEventKind::KeyDown => "keydown",
            ServerHtmlEventKind::KeyUp => "keyup",
            ServerHtmlEventKind::PointerDown => "pointerdown",
            ServerHtmlEventKind::PointerUp => "pointerup",
            ServerHtmlEventKind::PointerMove => "pointermove",
            ServerHtmlEventKind::Focus => "focus",
            ServerHtmlEventKind::Blur => "blur",
            ServerHtmlEventKind::TransitionEnd => "transitionend",
            ServerHtmlEventKind::AnimationEnd => "animationend",
        }
    }

    fn from_str(kind: &str) -> Option<Self> {
        Some(match kind {
            "click" => ServerHtmlEventKind::Click,
            "input" => ServerHtmlEventKind::Input,
            "keydown" => ServerHtmlEventKind::KeyDown,
            "keyup" => ServerHtmlEventKind::KeyUp,
            "pointerdown" => ServerHtmlEventKind::PointerDown,
            "pointerup" => ServerHtmlEventKind::PointerUp,
            "pointermove" => ServerHtmlEventKind::PointerMove,
            "focus" => ServerHtmlEventKind::Focus,
            "blur" => ServerHtmlEventKind::Blur,
            "transitionend" => ServerHtmlEventKind::TransitionEnd,
            "animationend" => ServerHtmlEventKind::AnimationEnd,
            _ => return None,
        })
    }
}

#[derive(Clone)]
enum ServerHtmlHandler {
    Msg(Value),
    FnText(Value),   // Text -> msg
    FnRecord(Value), // Record -> msg
}

#[derive(Clone)]
struct PendingEffect {
    kind: String,
    callback: Value, // Result ClipboardError a -> msg
}

struct ServerHtmlViewState {
    model: Value,
    vdom: Value,
    handlers: HashMap<i64, (ServerHtmlEventKind, ServerHtmlHandler)>,
    next_rid: i64,
    pending_effects: HashMap<i64, PendingEffect>,
}

static SERVER_HTML_VIEWS: OnceLock<Mutex<HashMap<String, ServerHtmlViewState>>> = OnceLock::new();

fn server_html_views() -> &'static Mutex<HashMap<String, ServerHtmlViewState>> {
    SERVER_HTML_VIEWS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn build_server_html_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "serveHttp".to_string(),
        builtin("ui.ServerHtml.serveHttp", 2, |mut args, runtime| {
            let req = args.pop().unwrap();
            let app = args.pop().unwrap();
            server_html_serve_http(app, req, runtime)
        }),
    );
    fields.insert(
        "serveWs".to_string(),
        builtin("ui.ServerHtml.serveWs", 2, |mut args, _runtime| {
            let socket = args.pop().unwrap();
            let app = args.pop().unwrap();
            Ok(server_html_serve_ws_effect(app, socket))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn server_html_serve_http(app: Value, req: Value, runtime: &mut Runtime) -> Result<Value, RuntimeError> {
    let app_fields = expect_record(app, "ServerHtmlApp must be a record")?;
    let init = app_fields
        .get("init")
        .cloned()
        .ok_or_else(|| RuntimeError::Message("ServerHtmlApp.init missing".to_string()))?;
    let view = app_fields
        .get("view")
        .cloned()
        .ok_or_else(|| RuntimeError::Message("ServerHtmlApp.view missing".to_string()))?;

    let req_fields = expect_record(req, "Request must be a record")?;
    let path = match req_fields.get("path") {
        Some(Value::Text(t)) => t.clone(),
        _ => "/".to_string(),
    };

    let view_id = Uuid::new_v4().to_string();
    let ws_path = live_ws_path(&path);

    let url_info = server_html_url_info_from_request(&req_fields);
    let init_ctx = server_html_init_context_value(&view_id, url_info, true);
    let model0 = runtime.apply(init, init_ctx)?;
    let vdom0 = runtime.apply(view, model0.clone())?;

    let (body_html, handlers) = server_html_render_vnode(&vdom0, "root");

    server_html_views()
        .lock()
        .expect("server html views lock")
        .insert(
            view_id.clone(),
            ServerHtmlViewState {
                model: model0,
                vdom: vdom0,
                handlers,
                next_rid: 1,
                pending_effects: HashMap::new(),
            },
        );

    let page = server_html_html_page(&body_html, &view_id, &ws_path);
    Ok(server_html_text_response(200, "text/html; charset=utf-8", page))
}

fn server_html_serve_ws_effect(app: Value, socket: Value) -> Value {
    Value::Effect(Arc::new(EffectValue::Thunk {
        func: Arc::new(move |runtime| {
            server_html_run_ws_session(runtime, app.clone(), socket.clone())?;
            Ok(Value::Unit)
        }),
    }))
}

fn server_html_run_ws_session(
    runtime: &mut Runtime,
    app: Value,
    socket_value: Value,
) -> Result<(), RuntimeError> {
    let socket = match socket_value {
        Value::WebSocket(handle) => handle,
        _ => return Err(RuntimeError::Message("serveWs expects WebSocket".to_string())),
    };

    let app_fields = expect_record(app, "ServerHtmlApp must be a record")?;
    let update = app_fields
        .get("update")
        .cloned()
        .ok_or_else(|| RuntimeError::Message("ServerHtmlApp.update missing".to_string()))?;
    let view = app_fields
        .get("view")
        .cloned()
        .ok_or_else(|| RuntimeError::Message("ServerHtmlApp.view missing".to_string()))?;
    let on_platform = app_fields.get("onPlatform").cloned();

    let hello = loop {
        let msg = socket
            .recv()
            .map_err(|err| RuntimeError::Message(err.message))?;
        match msg {
            AiviWsMessage::TextMsg(t) => match server_html_decode_hello(&t) {
                Ok(h) => break h,
                Err(e) => {
                    // Ignore non-hello / malformed messages until close.
                    let _ = server_html_send_error(&socket, &e, "PROTO");
                    continue;
                }
            },
            AiviWsMessage::Close => return Ok(()),
            _ => continue,
        }
    };

    let view_id = hello.view_id.clone();

    // Ensure the view exists (allocated by `serveHttp`).
    if !server_html_views()
        .lock()
        .expect("server html views lock")
        .contains_key(&view_id)
    {
        let _ = server_html_send_error(&socket, "unknown viewId", "PROTO");
        let _ = socket.close();
        return Ok(());
    }

    // Apply initial `online` signal, if the app wants it.
    if let Some(on_platform) = on_platform.clone() {
        let pe = Value::Constructor {
            name: "Online".to_string(),
            args: vec![server_html_online_payload_value(hello.online)],
        };
        let mapped = runtime.apply(on_platform.clone(), pe)?;
        if let Some(msg) = server_html_option_to_value(mapped) {
            server_html_apply_msg(
                runtime,
                &view_id,
                &socket,
                msg,
                &update,
                &view,
            )?;
        }
    }

    loop {
        let msg = socket
            .recv()
            .map_err(|err| RuntimeError::Message(err.message))?;
        let text = match msg {
            AiviWsMessage::TextMsg(t) => t,
            AiviWsMessage::Close => break,
            _ => continue,
        };

        match server_html_decode_client_msg(&text) {
            Ok(ServerHtmlClientMsg::Event(ev)) => {
                if ev.view_id != view_id {
                    continue;
                }
                if let Err(err) = server_html_handle_event(runtime, &view_id, &socket, ev, &update, &view, on_platform.as_ref()) {
                    let detail = runtime_error_to_text(err);
                    let code = if detail.contains("hid") {
                        "HID"
                    } else {
                        "PAYLOAD"
                    };
                    let _ = server_html_send_error(
                        &socket,
                        &detail,
                        code,
                    );
                }
            }
            Ok(ServerHtmlClientMsg::Platform(pf)) => {
                if pf.view_id != view_id {
                    continue;
                }
                if let Some(on_platform) = on_platform.clone() {
                    match server_html_platform_to_value(&pf.kind, &pf.payload) {
                        Ok(pe) => {
                            let mapped = runtime.apply(on_platform.clone(), pe)?;
                            if let Some(msg) = server_html_option_to_value(mapped) {
                                server_html_apply_msg(
                                    runtime,
                                    &view_id,
                                    &socket,
                                    msg,
                                    &update,
                                    &view,
                                )?;
                            }
                        }
                        Err(err) => {
                            let _ = server_html_send_error(&socket, &err, "PLATFORM");
                        }
                    }
                }
            }
            Ok(ServerHtmlClientMsg::EffectResult(er)) => {
                if er.view_id != view_id {
                    continue;
                }
                if let Err(err) = server_html_handle_effect_result(
                    runtime,
                    &view_id,
                    &socket,
                    er,
                    &update,
                    &view,
                    on_platform.as_ref(),
                ) {
                    let _ = server_html_send_error(
                        &socket,
                        &runtime_error_to_text(err),
                        "RID",
                    );
                }
            }
            Ok(ServerHtmlClientMsg::Hello) => {
                // Ignore repeated hello.
            }
            Err(err) => {
                let _ = server_html_send_error(&socket, &err, "DECODE");
            }
        }
    }

    // Best-effort cleanup.
    server_html_views()
        .lock()
        .expect("server html views lock")
        .remove(&view_id);
    let _ = socket.close();
    Ok(())
}

fn server_html_handle_event(
    runtime: &mut Runtime,
    view_id: &str,
    socket: &WebSocketHandle,
    ev: ServerHtmlEventMsg,
    update: &Value,
    view: &Value,
    on_platform: Option<&Value>,
) -> Result<(), RuntimeError> {
    let kind = ServerHtmlEventKind::from_str(&ev.kind)
        .ok_or_else(|| RuntimeError::Message("unknown kind".to_string()))?;
    let (handler_kind, handler) = {
        let guard = server_html_views().lock().expect("server html views lock");
        let state = guard
            .get(view_id)
            .ok_or_else(|| RuntimeError::Message("unknown viewId".to_string()))?;
        state
            .handlers
            .get(&ev.hid)
            .cloned()
            .ok_or_else(|| RuntimeError::Message("unknown hid".to_string()))?
    };
    if handler_kind != kind {
        return Err(RuntimeError::Message("hid kind mismatch".to_string()));
    }

    let msg = match handler {
        ServerHtmlHandler::Msg(msg) => msg,
        ServerHtmlHandler::FnText(f) => {
            let value = server_html_extract_text_value(kind, &ev.payload)
                .map_err(RuntimeError::Message)?;
            runtime.apply(f, Value::Text(value))?
        }
        ServerHtmlHandler::FnRecord(f) => {
            let record =
                server_html_payload_to_value(kind, &ev.payload).map_err(RuntimeError::Message)?;
            runtime.apply(f, record)?
        }
    };

    let _ = on_platform;
    server_html_apply_msg(runtime, view_id, socket, msg, update, view)?;
    Ok(())
}

fn server_html_handle_effect_result(
    runtime: &mut Runtime,
    view_id: &str,
    socket: &WebSocketHandle,
    er: ServerHtmlEffectResultMsg,
    update: &Value,
    view: &Value,
    on_platform: Option<&Value>,
) -> Result<(), RuntimeError> {
    let pending = {
        let mut guard = server_html_views().lock().expect("server html views lock");
        let state = guard
            .get_mut(view_id)
            .ok_or_else(|| RuntimeError::Message("unknown viewId".to_string()))?;
        state.pending_effects.remove(&er.rid)
    };
    let Some(pending) = pending else {
        return Ok(());
    };
    if pending.kind != er.kind {
        return Err(RuntimeError::Message("rid kind mismatch".to_string()));
    }

    let result_value = server_html_clipboard_result_value(&er).map_err(RuntimeError::Message)?;
    let msg = runtime.apply(pending.callback, result_value)?;
    let _ = on_platform;
    server_html_apply_msg(runtime, view_id, socket, msg, update, view)?;
    Ok(())
}

fn server_html_apply_msg(
    runtime: &mut Runtime,
    view_id: &str,
    socket: &WebSocketHandle,
    msg: Value,
    update: &Value,
    view: &Value,
) -> Result<(), RuntimeError> {
    // Snapshot the view state without holding the lock during user code execution.
    let (old_model, old_vdom, mut next_rid) = {
        let guard = server_html_views().lock().expect("server html views lock");
        let state = guard
            .get(view_id)
            .ok_or_else(|| RuntimeError::Message("unknown viewId".to_string()))?;
        (state.model.clone(), state.vdom.clone(), state.next_rid)
    };

    // update : msg -> model -> (model, List (Effect msg))
    let update_applied = runtime.apply(update.clone(), msg)?;
    let pair = runtime.apply(update_applied, old_model)?;
    let (new_model, effects) = server_html_expect_model_and_effects(pair)?;
    let new_vdom = runtime.apply(view.clone(), new_model.clone())?;

    let mut ops = Vec::new();
    server_html_diff_vnode(&old_vdom, &new_vdom, "root", &mut ops);
    let patch_msg = if ops.is_empty() {
        None
    } else {
        Some(serde_json::json!({
            "t": "patch",
            "ops": patch_ops_to_json_value(&Value::List(Arc::new(ops)))?
        }))
    };

    let (_html, handlers) = server_html_render_vnode(&new_vdom, "root");

    let (out_msgs, pending) = server_html_prepare_effects(&effects, &mut next_rid)?;

    {
        let mut guard = server_html_views().lock().expect("server html views lock");
        let state = guard
            .get_mut(view_id)
            .ok_or_else(|| RuntimeError::Message("unknown viewId".to_string()))?;
        state.model = new_model;
        state.vdom = new_vdom;
        state.handlers = handlers;
        state.next_rid = next_rid;
        for (rid, eff) in pending {
            state.pending_effects.insert(rid, eff);
        }
    }

    if let Some(msg) = patch_msg {
        server_html_send_json(socket, msg)?;
    }
    for msg in out_msgs {
        server_html_send_json(socket, msg)?;
    }
    Ok(())
}

fn server_html_expect_model_and_effects(value: Value) -> Result<(Value, Arc<Vec<Value>>), RuntimeError> {
    match value {
        Value::Tuple(mut items) if items.len() == 2 => {
            let effects = items.pop().unwrap();
            let model = items.pop().unwrap();
            match effects {
                Value::List(items) => Ok((model, items)),
                _ => Err(RuntimeError::Message(
                    "ServerHtmlApp.update must return (model, List (Effect msg))".to_string(),
                )),
            }
        }
        _ => Err(RuntimeError::Message(
            "ServerHtmlApp.update must return (model, List (Effect msg))".to_string(),
        )),
    }
}

fn server_html_prepare_effects(
    effects: &Arc<Vec<Value>>,
    next_rid: &mut i64,
) -> Result<(Vec<serde_json::Value>, Vec<(i64, PendingEffect)>), RuntimeError> {
    let mut out = Vec::new();
    let mut pending = Vec::new();

    for eff in effects.iter() {
        let Value::Constructor { name, args } = eff else {
            continue;
        };
        match (name.as_str(), args.as_slice()) {
            ("ClipboardReadText", [callback]) => {
                let rid = *next_rid;
                *next_rid = next_rid.saturating_add(1);
                pending.push((
                    rid,
                    PendingEffect {
                        kind: "clipboard.readText".to_string(),
                        callback: callback.clone(),
                    },
                ));
                out.push(serde_json::json!({
                    "t": "effectReq",
                    "rid": rid,
                    "op": { "kind": "clipboard.readText" }
                }));
            }
            ("ClipboardWriteText", [Value::Text(text), callback]) => {
                let rid = *next_rid;
                *next_rid = next_rid.saturating_add(1);
                pending.push((
                    rid,
                    PendingEffect {
                        kind: "clipboard.writeText".to_string(),
                        callback: callback.clone(),
                    },
                ));
                out.push(serde_json::json!({
                    "t": "effectReq",
                    "rid": rid,
                    "op": { "kind": "clipboard.writeText", "text": text }
                }));
            }
            ("SubscribeIntersection", [sub]) => {
                out.push(server_html_intersection_subscribe_value(sub)?);
            }
            ("UnsubscribeIntersection", [Value::Int(sid)]) => {
                out.push(serde_json::json!({
                    "t": "unsubscribeIntersect",
                    "sid": sid
                }));
            }
            _ => {}
        }
    }

    Ok((out, pending))
}

fn server_html_intersection_subscribe_value(sub: &Value) -> Result<serde_json::Value, RuntimeError> {
    let record = match sub {
        Value::Record(fields) => fields,
        _ => {
            return Err(RuntimeError::Message(
                "SubscribeIntersection expects a record".to_string(),
            ))
        }
    };
    let sid = match record.get("sid") {
        Some(Value::Int(v)) => *v,
        _ => {
            return Err(RuntimeError::Message(
                "SubscribeIntersection.sid must be Int".to_string(),
            ))
        }
    };
    let root_margin = match record.get("rootMargin") {
        Some(Value::Text(t)) => t.clone(),
        _ => "0px".to_string(),
    };
    let threshold = match record.get("threshold") {
        Some(Value::List(items)) => items
            .iter()
            .filter_map(|v| match v {
                Value::Float(f) => Some(*f),
                Value::Int(i) => Some(*i as f64),
                _ => None,
            })
            .collect::<Vec<f64>>(),
        _ => vec![0.0],
    };
    let targets = match record.get("targets") {
        Some(Value::List(items)) => items
            .iter()
            .filter_map(|t| match t {
                Value::Record(fields) => {
                    let tid = fields.get("tid").and_then(|v| v.as_i64());
                    let node_id = fields.get("nodeId").and_then(|v| v.as_text());
                    match (tid, node_id) {
                        (Some(tid), Some(node_id)) => {
                            Some(serde_json::json!({"tid": tid, "nodeId": node_id}))
                        }
                        _ => None,
                    }
                }
                _ => None,
            })
            .collect::<Vec<serde_json::Value>>(),
        _ => Vec::new(),
    };
    Ok(serde_json::json!({
        "t": "subscribeIntersect",
        "sid": sid,
        "options": { "rootMargin": root_margin, "threshold": threshold },
        "targets": targets
    }))
}

fn server_html_send_json(socket: &WebSocketHandle, value: serde_json::Value) -> Result<(), RuntimeError> {
    let text = serde_json::to_string(&value).map_err(|e| RuntimeError::Message(e.to_string()))?;
    socket
        .send(AiviWsMessage::TextMsg(text))
        .map_err(|err| RuntimeError::Message(err.message))?;
    Ok(())
}

fn server_html_send_error(
    socket: &WebSocketHandle,
    detail: &str,
    code: &str,
) -> Result<(), RuntimeError> {
    let payload = serde_json::json!({
        "t": "error",
        "detail": detail,
        "code": code
    });
    server_html_send_json(socket, payload)
}

fn server_html_text_response(status: i64, content_type: &str, body: String) -> Value {
    let mut header = HashMap::new();
    header.insert("name".to_string(), Value::Text("content-type".to_string()));
    header.insert("value".to_string(), Value::Text(content_type.to_string()));

    let mut fields = HashMap::new();
    fields.insert("status".to_string(), Value::Int(status));
    fields.insert(
        "headers".to_string(),
        Value::List(Arc::new(vec![Value::Record(Arc::new(header))])),
    );
    fields.insert("body".to_string(), Value::List(Arc::new(bytes_to_list(&body))));
    Value::Record(Arc::new(fields))
}

fn bytes_to_list(text: &str) -> Vec<Value> {
    text.as_bytes().iter().map(|b| Value::Int(*b as i64)).collect()
}

fn server_html_html_page(body_html: &str, view_id: &str, ws_path: &str) -> String {
    let boot = serde_json::json!({
        "viewId": view_id,
        "wsUrl": ws_path
    });
    let boot_json = serde_json::to_string(&boot).unwrap_or_else(|_| "{}".to_string());
    let client_js = include_str!("server_html_client.js");
    format!(
        "<!doctype html>\
<html><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>AIVI</title>\
</head><body>\
<div id=\"aivi-root\">{}</div>\
<script id=\"aivi-server-html-boot\" type=\"application/json\">{}</script>\
<script>{}</script>\
</body></html>",
        body_html,
        escape_html_text(&boot_json),
        client_js
    )
}

struct ServerHtmlRenderState {
    handlers: HashMap<i64, (ServerHtmlEventKind, ServerHtmlHandler)>,
}

fn server_html_render_vnode(vnode: &Value, node_id: &str) -> (String, HashMap<i64, (ServerHtmlEventKind, ServerHtmlHandler)>) {
    let mut state = ServerHtmlRenderState {
        handlers: HashMap::new(),
    };
    let html = server_html_render_vnode_inner(vnode, node_id, None, &mut state);
    (html, state.handlers)
}

fn server_html_render_vnode_inner(
    vnode: &Value,
    node_id: &str,
    keyed: Option<&str>,
    state: &mut ServerHtmlRenderState,
) -> String {
    match vnode {
        Value::Constructor { name, args } if name == "TextNode" && args.len() == 1 => {
            let text = match &args[0] {
                Value::Text(t) => t.clone(),
                other => crate::runtime::format_value(other),
            };
            let mut attrs = format!(" data-aivi-node=\"{}\"", escape_attr_value(node_id));
            if let Some(key) = keyed {
                attrs.push_str(&format!(" data-aivi-key=\"{}\"", escape_attr_value(key)));
            }
            format!("<span{attrs}>{}</span>", escape_html_text(&text), attrs = attrs)
        }
        Value::Constructor { name, args } if name == "Keyed" && args.len() == 2 => {
            let key = match &args[0] {
                Value::Text(t) => t.clone(),
                other => crate::runtime::format_value(other),
            };
            server_html_render_vnode_inner(&args[1], node_id, Some(&key), state)
        }
        Value::Constructor { name, args } if name == "Element" && args.len() == 3 => {
            let tag = match &args[0] {
                Value::Text(t) => sanitize_tag(t),
                _ => "div".to_string(),
            };
            let attrs_value = &args[1];
            let children_value = &args[2];

            let mut attrs = String::new();
            attrs.push_str(&format!(
                " data-aivi-node=\"{}\"",
                escape_attr_value(node_id)
            ));
            if let Some(key) = keyed {
                attrs.push_str(&format!(" data-aivi-key=\"{}\"", escape_attr_value(key)));
            }
            attrs.push_str(&server_html_render_attrs(attrs_value, node_id, state));

            let mut children_html = String::new();
            if let Value::List(items) = children_value {
                for (idx, child) in items.iter().enumerate() {
                    let seg = child_segment(child, idx);
                    let child_id = format!("{}/{}", node_id, seg);
                    children_html.push_str(&server_html_render_vnode_inner(child, &child_id, None, state));
                }
            }
            format!(
                "<{tag}{attrs}>{children}</{tag}>",
                tag = tag,
                attrs = attrs,
                children = children_html
            )
        }
        other => format!(
            "<span data-aivi-node=\"{}\">{}</span>",
            escape_attr_value(node_id),
            escape_html_text(&crate::runtime::format_value(other))
        ),
    }
}

fn server_html_render_attrs(attrs: &Value, node_id: &str, state: &mut ServerHtmlRenderState) -> String {
    let mut out = String::new();
    let Value::List(items) = attrs else {
        return out;
    };
    for attr in items.iter() {
        match attr {
            Value::Constructor { name, args } if name == "Class" && args.len() == 1 => {
                if let Value::Text(t) = &args[0] {
                    out.push_str(&format!(" class=\"{}\"", escape_attr_value(t)));
                }
            }
            Value::Constructor { name, args } if name == "Id" && args.len() == 1 => {
                if let Value::Text(t) = &args[0] {
                    out.push_str(&format!(" id=\"{}\"", escape_attr_value(t)));
                }
            }
            Value::Constructor { name, args } if name == "Style" && args.len() == 1 => {
                let style = style_record_to_text(&args[0]);
                out.push_str(&format!(" style=\"{}\"", escape_attr_value(&style)));
            }
            Value::Constructor { name, args } if name == "Attr" && args.len() == 2 => {
                if let (Value::Text(k), Value::Text(v)) = (&args[0], &args[1]) {
                    if is_safe_attr_name(k) {
                        out.push_str(&format!(" {}=\"{}\"", k, escape_attr_value(v)));
                    }
                }
            }

            // Legacy handlers (no payload / Text payload).
            Value::Constructor { name, args } if name == "OnClick" && args.len() == 1 => {
                server_html_add_handler(&mut out, state, node_id, ServerHtmlEventKind::Click, ServerHtmlHandler::Msg(args[0].clone()));
            }
            Value::Constructor { name, args } if name == "OnInput" && args.len() == 1 => {
                server_html_add_handler(&mut out, state, node_id, ServerHtmlEventKind::Input, ServerHtmlHandler::FnText(args[0].clone()));
            }

            // Typed handlers.
            Value::Constructor { name, args } if name == "OnClickE" && args.len() == 1 => {
                server_html_add_handler(&mut out, state, node_id, ServerHtmlEventKind::Click, ServerHtmlHandler::FnRecord(args[0].clone()));
            }
            Value::Constructor { name, args } if name == "OnInputE" && args.len() == 1 => {
                server_html_add_handler(&mut out, state, node_id, ServerHtmlEventKind::Input, ServerHtmlHandler::FnRecord(args[0].clone()));
            }
            Value::Constructor { name, args } if name == "OnKeyDown" && args.len() == 1 => {
                server_html_add_handler(&mut out, state, node_id, ServerHtmlEventKind::KeyDown, ServerHtmlHandler::FnRecord(args[0].clone()));
            }
            Value::Constructor { name, args } if name == "OnKeyUp" && args.len() == 1 => {
                server_html_add_handler(&mut out, state, node_id, ServerHtmlEventKind::KeyUp, ServerHtmlHandler::FnRecord(args[0].clone()));
            }
            Value::Constructor { name, args } if name == "OnPointerDown" && args.len() == 1 => {
                server_html_add_handler(&mut out, state, node_id, ServerHtmlEventKind::PointerDown, ServerHtmlHandler::FnRecord(args[0].clone()));
            }
            Value::Constructor { name, args } if name == "OnPointerUp" && args.len() == 1 => {
                server_html_add_handler(&mut out, state, node_id, ServerHtmlEventKind::PointerUp, ServerHtmlHandler::FnRecord(args[0].clone()));
            }
            Value::Constructor { name, args } if name == "OnPointerMove" && args.len() == 1 => {
                server_html_add_handler(&mut out, state, node_id, ServerHtmlEventKind::PointerMove, ServerHtmlHandler::FnRecord(args[0].clone()));
            }
            Value::Constructor { name, args } if name == "OnFocus" && args.len() == 1 => {
                server_html_add_handler(&mut out, state, node_id, ServerHtmlEventKind::Focus, ServerHtmlHandler::Msg(args[0].clone()));
            }
            Value::Constructor { name, args } if name == "OnBlur" && args.len() == 1 => {
                server_html_add_handler(&mut out, state, node_id, ServerHtmlEventKind::Blur, ServerHtmlHandler::Msg(args[0].clone()));
            }
            Value::Constructor { name, args } if name == "OnTransitionEnd" && args.len() == 1 => {
                server_html_add_handler(&mut out, state, node_id, ServerHtmlEventKind::TransitionEnd, ServerHtmlHandler::FnRecord(args[0].clone()));
            }
            Value::Constructor { name, args } if name == "OnAnimationEnd" && args.len() == 1 => {
                server_html_add_handler(&mut out, state, node_id, ServerHtmlEventKind::AnimationEnd, ServerHtmlHandler::FnRecord(args[0].clone()));
            }
            _ => {}
        }
    }
    out
}

fn server_html_add_handler(
    out: &mut String,
    state: &mut ServerHtmlRenderState,
    node_id: &str,
    kind: ServerHtmlEventKind,
    handler: ServerHtmlHandler,
) {
    let hid = event_id(kind.as_str(), node_id);
    state.handlers.insert(hid, (kind, handler));
    out.push_str(&format!(
        " data-aivi-hid-{}=\"{}\"",
        kind.as_str(),
        hid
    ));
}

fn server_html_diff_vnode(old: &Value, new: &Value, node_id: &str, out: &mut Vec<Value>) {
    if !same_vnode_shape(old, new) {
        let (html, _handlers) = server_html_render_vnode(new, node_id);
        out.push(Value::Constructor {
            name: "Replace".to_string(),
            args: vec![Value::Text(node_id.to_string()), Value::Text(html)],
        });
        return;
    }

    match (old, new) {
        (Value::Constructor { name: on, args: oa }, Value::Constructor { name: nn, args: na })
            if on == "TextNode" && nn == "TextNode" && oa.len() == 1 && na.len() == 1 =>
        {
            let ot = match &oa[0] {
                Value::Text(t) => t.as_str(),
                _ => "",
            };
            let nt = match &na[0] {
                Value::Text(t) => t.as_str(),
                _ => "",
            };
            if ot != nt {
                out.push(Value::Constructor {
                    name: "SetText".to_string(),
                    args: vec![
                        Value::Text(node_id.to_string()),
                        Value::Text(nt.to_string()),
                    ],
                });
            }
        }
        (Value::Constructor { name: on, args: oa }, Value::Constructor { name: nn, args: na })
            if on == "Keyed" && nn == "Keyed" && oa.len() == 2 && na.len() == 2 =>
        {
            let ok = match &oa[0] {
                Value::Text(t) => t.as_str(),
                _ => "",
            };
            let nk = match &na[0] {
                Value::Text(t) => t.as_str(),
                _ => "",
            };
            if ok != nk {
                let (html, _handlers) = server_html_render_vnode(new, node_id);
                out.push(Value::Constructor {
                    name: "Replace".to_string(),
                    args: vec![Value::Text(node_id.to_string()), Value::Text(html)],
                });
                return;
            }
            server_html_diff_vnode(&oa[1], &na[1], node_id, out);
        }
        (Value::Constructor { name: on, args: oa }, Value::Constructor { name: nn, args: na })
            if on == "Element" && nn == "Element" && oa.len() == 3 && na.len() == 3 =>
        {
            let otag = match &oa[0] {
                Value::Text(t) => t.as_str(),
                _ => "",
            };
            let ntag = match &na[0] {
                Value::Text(t) => t.as_str(),
                _ => "",
            };
            if otag != ntag {
                let (html, _handlers) = server_html_render_vnode(new, node_id);
                out.push(Value::Constructor {
                    name: "Replace".to_string(),
                    args: vec![Value::Text(node_id.to_string()), Value::Text(html)],
                });
                return;
            }

            server_html_diff_attrs(&oa[1], &na[1], node_id, out);

            let oseg = child_segments(&oa[2]);
            let nseg = child_segments(&na[2]);
            if oseg != nseg {
                let (html, _handlers) = server_html_render_vnode(new, node_id);
                out.push(Value::Constructor {
                    name: "Replace".to_string(),
                    args: vec![Value::Text(node_id.to_string()), Value::Text(html)],
                });
                return;
            }

            if let (Value::List(oc), Value::List(nc)) = (&oa[2], &na[2]) {
                for (idx, (ochild, nchild)) in oc.iter().zip(nc.iter()).enumerate() {
                    let seg = child_segment(nchild, idx);
                    let child_id = format!("{}/{}", node_id, seg);
                    server_html_diff_vnode(ochild, nchild, &child_id, out);
                }
            }
        }
        _ => {}
    }
}

fn server_html_attrs_to_map(attrs: &Value, node_id: &str) -> HashMap<String, String> {
    let mut state = ServerHtmlRenderState {
        handlers: HashMap::new(),
    };
    let s = server_html_render_attrs(attrs, node_id, &mut state);
    let mut map = HashMap::new();
    let mut i = 0usize;
    let chars: Vec<char> = s.chars().collect();
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        let start = i;
        while i < chars.len() && chars[i] != '=' && !chars[i].is_whitespace() {
            i += 1;
        }
        let key: String = chars[start..i].iter().collect();
        if key.is_empty() {
            break;
        }
        while i < chars.len() && chars[i] != '"' {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        i += 1;
        let vstart = i;
        while i < chars.len() && chars[i] != '"' {
            i += 1;
        }
        let value: String = chars[vstart..i].iter().collect();
        if i < chars.len() {
            i += 1;
        }
        map.insert(key.trim().to_string(), value);
    }
    map
}

fn server_html_diff_attrs(old: &Value, new: &Value, node_id: &str, out: &mut Vec<Value>) {
    let old_map = server_html_attrs_to_map(old, node_id);
    let new_map = server_html_attrs_to_map(new, node_id);

    let mut new_keys: Vec<&String> = new_map.keys().collect();
    new_keys.sort();
    for k in new_keys {
        let Some(v) = new_map.get(k) else {
            continue;
        };
        if old_map.get(k) != Some(v) {
            out.push(Value::Constructor {
                name: "SetAttr".to_string(),
                args: vec![
                    Value::Text(node_id.to_string()),
                    Value::Text(k.to_string()),
                    Value::Text(v.to_string()),
                ],
            });
        }
    }

    let mut old_keys: Vec<&String> = old_map.keys().collect();
    old_keys.sort();
    for k in old_keys {
        if !new_map.contains_key(k) {
            out.push(Value::Constructor {
                name: "RemoveAttr".to_string(),
                args: vec![Value::Text(node_id.to_string()), Value::Text(k.to_string())],
            });
        }
    }
}

// --- Client message decoding (JSON) ---

#[derive(Debug)]
struct ServerHtmlHello {
    view_id: String,
    online: bool,
}

#[derive(Debug)]
struct ServerHtmlEventMsg {
    view_id: String,
    hid: i64,
    kind: String,
    payload: serde_json::Value,
}

#[derive(Debug)]
struct ServerHtmlPlatformMsg {
    view_id: String,
    kind: String,
    payload: serde_json::Value,
}

#[derive(Debug)]
struct ServerHtmlEffectResultMsg {
    view_id: String,
    rid: i64,
    kind: String,
    ok: bool,
    payload: Option<serde_json::Value>,
    error: Option<String>,
}

#[derive(Debug)]
enum ServerHtmlClientMsg {
    Hello,
    Event(ServerHtmlEventMsg),
    Platform(ServerHtmlPlatformMsg),
    EffectResult(ServerHtmlEffectResultMsg),
}

fn server_html_decode_hello(text: &str) -> Result<ServerHtmlHello, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("invalid json: {e}"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| "message must be an object".to_string())?;
    let t = obj
        .get("t")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "message.t must be a string".to_string())?;
    if t != "hello" {
        return Err("expected hello".to_string());
    }
    let view_id = obj
        .get("viewId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "hello.viewId must be a string".to_string())?
        .to_string();
    let online = obj
        .get("online")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    Ok(ServerHtmlHello {
        view_id,
        online,
    })
}

fn server_html_decode_client_msg(text: &str) -> Result<ServerHtmlClientMsg, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("invalid json: {e}"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| "message must be an object".to_string())?;
    let t = obj
        .get("t")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "message.t must be a string".to_string())?;
    match t {
        "hello" => {
            let _ = server_html_decode_hello(text)?;
            Ok(ServerHtmlClientMsg::Hello)
        }
        "event" => {
            let view_id = obj
                .get("viewId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "event.viewId must be a string".to_string())?
                .to_string();
            let hid = obj
                .get("hid")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| "event.hid must be an int".to_string())?;
            let kind = obj
                .get("kind")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "event.kind must be a string".to_string())?
                .to_string();
            let payload = obj.get("p").cloned().unwrap_or_else(|| serde_json::json!({}));
            Ok(ServerHtmlClientMsg::Event(ServerHtmlEventMsg {
                view_id,
                hid,
                kind,
                payload,
            }))
        }
        "platform" => {
            let view_id = obj
                .get("viewId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "platform.viewId must be a string".to_string())?
                .to_string();
            let kind = obj
                .get("kind")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "platform.kind must be a string".to_string())?
                .to_string();
            let payload = obj.get("p").cloned().unwrap_or_else(|| serde_json::json!({}));
            Ok(ServerHtmlClientMsg::Platform(ServerHtmlPlatformMsg {
                view_id,
                kind,
                payload,
            }))
        }
        "effectResult" => {
            let view_id = obj
                .get("viewId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "effectResult.viewId must be a string".to_string())?
                .to_string();
            let rid = obj
                .get("rid")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| "effectResult.rid must be an int".to_string())?;
            let kind = obj
                .get("kind")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "effectResult.kind must be a string".to_string())?
                .to_string();
            let ok = obj
                .get("ok")
                .and_then(|v| v.as_bool())
                .ok_or_else(|| "effectResult.ok must be a bool".to_string())?;
            let payload = obj.get("p").cloned();
            let error = obj.get("error").and_then(|v| v.as_str()).map(|s| s.to_string());
            Ok(ServerHtmlClientMsg::EffectResult(ServerHtmlEffectResultMsg {
                view_id,
                rid,
                kind,
                ok,
                payload,
                error,
            }))
        }
        _ => Err("unknown message type".to_string()),
    }
}

// --- Payload conversion ---

fn server_html_payload_to_value(
    kind: ServerHtmlEventKind,
    payload: &serde_json::Value,
) -> Result<Value, String> {
    match kind {
        ServerHtmlEventKind::Click => Ok(server_html_click_event_value(payload)?),
        ServerHtmlEventKind::Input => Ok(server_html_input_event_value(payload)?),
        ServerHtmlEventKind::KeyDown | ServerHtmlEventKind::KeyUp => {
            Ok(server_html_keyboard_event_value(payload)?)
        }
        ServerHtmlEventKind::PointerDown
        | ServerHtmlEventKind::PointerUp
        | ServerHtmlEventKind::PointerMove => Ok(server_html_pointer_event_value(payload)?),
        ServerHtmlEventKind::TransitionEnd => Ok(server_html_transition_event_value(payload)?),
        ServerHtmlEventKind::AnimationEnd => Ok(server_html_animation_event_value(payload)?),
        ServerHtmlEventKind::Focus | ServerHtmlEventKind::Blur => Ok(Value::Record(Arc::new(HashMap::new()))),
    }
}

fn server_html_extract_text_value(
    kind: ServerHtmlEventKind,
    payload: &serde_json::Value,
) -> Result<String, String> {
    if kind != ServerHtmlEventKind::Input {
        return Err("text handler used for non-input kind".to_string());
    }
    payload
        .get("value")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "input.value must be a string".to_string())
}

fn server_html_click_event_value(payload: &serde_json::Value) -> Result<Value, String> {
    let mut fields = HashMap::new();
    fields.insert(
        "button".to_string(),
        Value::Int(payload.get("button").and_then(|v| v.as_i64()).unwrap_or(0)),
    );
    fields.insert(
        "alt".to_string(),
        Value::Bool(payload.get("alt").and_then(|v| v.as_bool()).unwrap_or(false)),
    );
    fields.insert(
        "ctrl".to_string(),
        Value::Bool(payload.get("ctrl").and_then(|v| v.as_bool()).unwrap_or(false)),
    );
    fields.insert(
        "shift".to_string(),
        Value::Bool(payload.get("shift").and_then(|v| v.as_bool()).unwrap_or(false)),
    );
    fields.insert(
        "meta".to_string(),
        Value::Bool(payload.get("meta").and_then(|v| v.as_bool()).unwrap_or(false)),
    );
    Ok(Value::Record(Arc::new(fields)))
}

fn server_html_input_event_value(payload: &serde_json::Value) -> Result<Value, String> {
    let mut fields = HashMap::new();
    let value = payload
        .get("value")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    fields.insert("value".to_string(), Value::Text(value));
    Ok(Value::Record(Arc::new(fields)))
}

fn server_html_keyboard_event_value(payload: &serde_json::Value) -> Result<Value, String> {
    let mut fields = HashMap::new();
    fields.insert(
        "key".to_string(),
        Value::Text(payload.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string()),
    );
    fields.insert(
        "code".to_string(),
        Value::Text(payload.get("code").and_then(|v| v.as_str()).unwrap_or("").to_string()),
    );
    fields.insert(
        "alt".to_string(),
        Value::Bool(payload.get("alt").and_then(|v| v.as_bool()).unwrap_or(false)),
    );
    fields.insert(
        "ctrl".to_string(),
        Value::Bool(payload.get("ctrl").and_then(|v| v.as_bool()).unwrap_or(false)),
    );
    fields.insert(
        "shift".to_string(),
        Value::Bool(payload.get("shift").and_then(|v| v.as_bool()).unwrap_or(false)),
    );
    fields.insert(
        "meta".to_string(),
        Value::Bool(payload.get("meta").and_then(|v| v.as_bool()).unwrap_or(false)),
    );
    fields.insert(
        "repeat".to_string(),
        Value::Bool(payload.get("repeat").and_then(|v| v.as_bool()).unwrap_or(false)),
    );
    fields.insert(
        "isComposing".to_string(),
        Value::Bool(payload.get("isComposing").and_then(|v| v.as_bool()).unwrap_or(false)),
    );
    Ok(Value::Record(Arc::new(fields)))
}

fn server_html_pointer_event_value(payload: &serde_json::Value) -> Result<Value, String> {
    let mut fields = HashMap::new();
    fields.insert(
        "pointerId".to_string(),
        Value::Int(payload.get("pointerId").and_then(|v| v.as_i64()).unwrap_or(0)),
    );
    fields.insert(
        "pointerType".to_string(),
        Value::Text(
            payload
                .get("pointerType")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        ),
    );
    fields.insert(
        "button".to_string(),
        Value::Int(payload.get("button").and_then(|v| v.as_i64()).unwrap_or(0)),
    );
    fields.insert(
        "buttons".to_string(),
        Value::Int(payload.get("buttons").and_then(|v| v.as_i64()).unwrap_or(0)),
    );
    fields.insert(
        "clientX".to_string(),
        Value::Float(payload.get("clientX").and_then(|v| v.as_f64()).unwrap_or(0.0)),
    );
    fields.insert(
        "clientY".to_string(),
        Value::Float(payload.get("clientY").and_then(|v| v.as_f64()).unwrap_or(0.0)),
    );
    fields.insert(
        "alt".to_string(),
        Value::Bool(payload.get("alt").and_then(|v| v.as_bool()).unwrap_or(false)),
    );
    fields.insert(
        "ctrl".to_string(),
        Value::Bool(payload.get("ctrl").and_then(|v| v.as_bool()).unwrap_or(false)),
    );
    fields.insert(
        "shift".to_string(),
        Value::Bool(payload.get("shift").and_then(|v| v.as_bool()).unwrap_or(false)),
    );
    fields.insert(
        "meta".to_string(),
        Value::Bool(payload.get("meta").and_then(|v| v.as_bool()).unwrap_or(false)),
    );
    Ok(Value::Record(Arc::new(fields)))
}

fn server_html_transition_event_value(payload: &serde_json::Value) -> Result<Value, String> {
    let mut fields = HashMap::new();
    fields.insert(
        "propertyName".to_string(),
        Value::Text(
            payload
                .get("propertyName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        ),
    );
    fields.insert(
        "elapsedTime".to_string(),
        Value::Float(payload.get("elapsedTime").and_then(|v| v.as_f64()).unwrap_or(0.0)),
    );
    fields.insert(
        "pseudoElement".to_string(),
        Value::Text(
            payload
                .get("pseudoElement")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        ),
    );
    Ok(Value::Record(Arc::new(fields)))
}

fn server_html_animation_event_value(payload: &serde_json::Value) -> Result<Value, String> {
    let mut fields = HashMap::new();
    fields.insert(
        "animationName".to_string(),
        Value::Text(
            payload
                .get("animationName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        ),
    );
    fields.insert(
        "elapsedTime".to_string(),
        Value::Float(payload.get("elapsedTime").and_then(|v| v.as_f64()).unwrap_or(0.0)),
    );
    fields.insert(
        "pseudoElement".to_string(),
        Value::Text(
            payload
                .get("pseudoElement")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        ),
    );
    Ok(Value::Record(Arc::new(fields)))
}

fn server_html_platform_to_value(kind: &str, payload: &serde_json::Value) -> Result<Value, String> {
    match kind {
        "popstate" => Ok(Value::Constructor {
            name: "PopState".to_string(),
            args: vec![server_html_url_info_from_platform(payload)],
        }),
        "hashchange" => {
            let mut fields = HashMap::new();
            fields.insert(
                "old".to_string(),
                Value::Text(payload.get("oldURL").and_then(|v| v.as_str()).unwrap_or("").to_string()),
            );
            fields.insert(
                "new".to_string(),
                Value::Text(payload.get("newURL").and_then(|v| v.as_str()).unwrap_or("").to_string()),
            );
            fields.insert(
                "hash".to_string(),
                Value::Text(payload.get("hash").and_then(|v| v.as_str()).unwrap_or("").to_string()),
            );
            fields.insert("url".to_string(), server_html_url_info_from_platform(payload));
            Ok(Value::Constructor {
                name: "HashChange".to_string(),
                args: vec![Value::Record(Arc::new(fields))],
            })
        }
        "visibility" | "visibilitychange" => {
            let mut fields = HashMap::new();
            fields.insert(
                "visibilityState".to_string(),
                Value::Text(
                    payload
                        .get("state")
                        .or_else(|| payload.get("visibilityState"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                ),
            );
            Ok(Value::Constructor {
                name: "Visibility".to_string(),
                args: vec![Value::Record(Arc::new(fields))],
            })
        }
        "focus" | "blur" => {
            let focused = payload.get("focused").and_then(|v| v.as_bool()).unwrap_or(kind == "focus");
            Ok(Value::Constructor {
                name: "WindowFocus".to_string(),
                args: vec![server_html_window_focus_payload_value(focused)],
            })
        }
        "online" => {
            let online = payload.get("online").and_then(|v| v.as_bool()).unwrap_or(true);
            Ok(Value::Constructor {
                name: "Online".to_string(),
                args: vec![server_html_online_payload_value(online)],
            })
        }
        "intersection" => {
            let sid = payload.get("sid").and_then(|v| v.as_i64()).unwrap_or(0);
            let entries = payload.get("entries").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let mut entry_values = Vec::new();
            for e in entries {
                let Some(obj) = e.as_object() else { continue };
                let tid = obj.get("tid").and_then(|v| v.as_i64()).unwrap_or(0);
                let is_intersecting = obj.get("isIntersecting").and_then(|v| v.as_bool()).unwrap_or(false);
                let ratio = obj.get("ratio").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let mut fields = HashMap::new();
                fields.insert("tid".to_string(), Value::Int(tid));
                fields.insert("isIntersecting".to_string(), Value::Bool(is_intersecting));
                fields.insert("ratio".to_string(), Value::Float(ratio));
                entry_values.push(Value::Record(Arc::new(fields)));
            }
            let mut fields = HashMap::new();
            fields.insert("sid".to_string(), Value::Int(sid));
            fields.insert("entries".to_string(), Value::List(Arc::new(entry_values)));
            Ok(Value::Constructor {
                name: "Intersection".to_string(),
                args: vec![Value::Record(Arc::new(fields))],
            })
        }
        _ => Err("unknown platform kind".to_string()),
    }
}

fn server_html_clipboard_result_value(er: &ServerHtmlEffectResultMsg) -> Result<Value, String> {
    if er.ok {
        // Ok payload depends on kind.
        if er.kind == "clipboard.readText" {
            let text = er
                .payload
                .as_ref()
                .and_then(|p| p.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Ok(Value::Constructor {
                name: "Ok".to_string(),
                args: vec![Value::Text(text)],
            })
        } else {
            Ok(Value::Constructor {
                name: "Ok".to_string(),
                args: vec![Value::Unit],
            })
        }
    } else {
        let name = er.error.clone().unwrap_or_else(|| "Error".to_string());
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), Value::Text(name));
        Ok(Value::Constructor {
            name: "Err".to_string(),
            args: vec![Value::Record(Arc::new(fields))],
        })
    }
}

// --- AIVI values for common records ---

fn server_html_init_context_value(view_id: &str, url_info: Value, online: bool) -> Value {
    let mut fields = HashMap::new();
    fields.insert("viewId".to_string(), Value::Text(view_id.to_string()));
    fields.insert("url".to_string(), url_info);
    fields.insert("online".to_string(), Value::Bool(online));
    Value::Record(Arc::new(fields))
}

fn server_html_url_info_from_request(req_fields: &HashMap<String, Value>) -> Value {
    let path_raw = req_fields
        .get("path")
        .and_then(|v| v.as_text())
        .unwrap_or("/")
        .to_string();
    let (path, query) = split_query(&path_raw);

    let host = req_fields
        .get("headers")
        .and_then(|h| headers_lookup(h, "host"))
        .unwrap_or_default();
    let url = if host.is_empty() {
        path_raw.clone()
    } else {
        format!("http://{host}{path_raw}")
    };

    let mut fields = HashMap::new();
    fields.insert("url".to_string(), Value::Text(url));
    fields.insert("path".to_string(), Value::Text(path));
    fields.insert("query".to_string(), Value::Text(query));
    fields.insert("hash".to_string(), Value::Text("".to_string()));
    Value::Record(Arc::new(fields))
}

fn server_html_url_info_from_platform(payload: &serde_json::Value) -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "url".to_string(),
        Value::Text(payload.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string()),
    );
    fields.insert(
        "path".to_string(),
        Value::Text(payload.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string()),
    );
    fields.insert(
        "query".to_string(),
        Value::Text(payload.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string()),
    );
    fields.insert(
        "hash".to_string(),
        Value::Text(payload.get("hash").and_then(|v| v.as_str()).unwrap_or("").to_string()),
    );
    Value::Record(Arc::new(fields))
}

fn server_html_online_payload_value(online: bool) -> Value {
    let mut fields = HashMap::new();
    fields.insert("online".to_string(), Value::Bool(online));
    Value::Record(Arc::new(fields))
}

fn server_html_window_focus_payload_value(focused: bool) -> Value {
    let mut fields = HashMap::new();
    fields.insert("focused".to_string(), Value::Bool(focused));
    Value::Record(Arc::new(fields))
}

fn split_query(path: &str) -> (String, String) {
    if let Some(idx) = path.find('?') {
        (path[..idx].to_string(), path[idx..].to_string())
    } else {
        (path.to_string(), "".to_string())
    }
}

fn headers_lookup(headers_value: &Value, key: &str) -> Option<String> {
    let Value::List(items) = headers_value else {
        return None;
    };
    for item in items.iter() {
        let Value::Record(fields) = item else {
            continue;
        };
        let name = fields.get("name").and_then(|v| v.as_text())?;
        if name.eq_ignore_ascii_case(key) {
            return fields
                .get("value")
                .and_then(|v| v.as_text())
                .map(|s| s.to_string());
        }
    }
    None
}

fn server_html_option_to_value(value: Value) -> Option<Value> {
    match value {
        Value::Constructor { name, mut args } if name == "Some" && args.len() == 1 => {
            Some(args.pop().unwrap())
        }
        _ => None,
    }
}

trait ValueExt {
    fn as_text(&self) -> Option<&str>;
    fn as_i64(&self) -> Option<i64>;
}

impl ValueExt for Value {
    fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(t) => Some(t.as_str()),
            _ => None,
        }
    }
    fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }
}

#[cfg(test)]
mod server_html_tests {
    use super::*;

    #[test]
    fn server_html_renders_hid_attributes() {
        let vnode = Value::Constructor {
            name: "Element".to_string(),
            args: vec![
                Value::Text("button".to_string()),
                Value::List(Arc::new(vec![Value::Constructor {
                    name: "OnClick".to_string(),
                    args: vec![Value::Int(1)],
                }])),
                Value::List(Arc::new(vec![Value::Constructor {
                    name: "TextNode".to_string(),
                    args: vec![Value::Text("Hi".to_string())],
                }])),
            ],
        };
        let (html, handlers) = server_html_render_vnode(&vnode, "root");
        assert!(html.contains("data-aivi-hid-click="));
        let hid = event_id("click", "root");
        assert!(html.contains(&format!("data-aivi-hid-click=\"{hid}\"")));
        assert!(handlers.contains_key(&hid));
    }

    #[test]
    fn server_html_decodes_event_msg() {
        let text = r#"{"t":"event","viewId":"v","hid":1,"kind":"click","p":{"button":0}}"#;
        match server_html_decode_client_msg(text).unwrap() {
            ServerHtmlClientMsg::Event(ev) => {
                assert_eq!(ev.view_id, "v");
                assert_eq!(ev.hid, 1);
                assert_eq!(ev.kind, "click");
            }
            _ => panic!("expected event"),
        }
    }

    #[test]
    fn server_html_page_includes_boot() {
        let html = server_html_html_page("<div data-aivi-node=\"root\"></div>", "vid", "/ws");
        assert!(html.contains("aivi-server-html-boot"));
        assert!(html.contains("viewId"));
        assert!(html.contains("wsUrl"));
    }

    #[test]
    fn server_html_prepares_clipboard_effects() {
        let effects = Value::List(Arc::new(vec![
            Value::Constructor {
                name: "ClipboardReadText".to_string(),
                args: vec![Value::Int(0)],
            },
            Value::Constructor {
                name: "ClipboardWriteText".to_string(),
                args: vec![Value::Text("x".to_string()), Value::Int(0)],
            },
        ]));
        let Value::List(items) = effects else {
            panic!("expected list");
        };
        let mut next_rid = 10i64;
        let (out, pending) = match server_html_prepare_effects(&items, &mut next_rid) {
            Ok(v) => v,
            Err(_) => panic!("effects failed"),
        };
        assert_eq!(next_rid, 12);
        assert_eq!(pending.len(), 2);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0]["t"], "effectReq");
        assert_eq!(out[1]["t"], "effectReq");
        assert!(out[0].to_string().contains("clipboard.readText"));
        assert!(out[1].to_string().contains("clipboard.writeText"));
    }

    #[test]
    fn server_html_renders_transition_handler() {
        let vnode = Value::Constructor {
            name: "Element".to_string(),
            args: vec![
                Value::Text("div".to_string()),
                Value::List(Arc::new(vec![Value::Constructor {
                    name: "OnTransitionEnd".to_string(),
                    args: vec![Value::Int(1)],
                }])),
                Value::List(Arc::new(vec![])),
            ],
        };
        let (html, handlers) = server_html_render_vnode(&vnode, "root");
        let hid = event_id("transitionend", "root");
        assert!(html.contains(&format!("data-aivi-hid-transitionend=\"{hid}\"")));
        assert!(handlers.contains_key(&hid));
    }
}
