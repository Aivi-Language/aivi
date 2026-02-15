use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use aivi_http_server::{
    AiviHttpError, AiviRequest, AiviResponse, AiviWsMessage, Handler, ServerReply, WebSocketHandle,
    WsHandlerFuture,
};

use super::util::{builtin, expect_record, expect_text};
use crate::values::CancelToken;
use crate::{format_value, EffectValue, Runtime, RuntimeContext, RuntimeError, Value};

#[derive(Clone)]
enum UiHandler {
    Click(Value),
    Input(Value), // Text -> msg
}

pub(super) fn build_ui_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "renderHtml".to_string(),
        builtin("ui.renderHtml", 1, |mut args, _runtime| {
            let vnode = args.pop().unwrap();
            let (html, _handlers) = render_vnode(&vnode, "root");
            Ok(Value::Text(html))
        }),
    );
    fields.insert(
        "diff".to_string(),
        builtin("ui.diff", 2, |mut args, _runtime| {
            let new = args.pop().unwrap();
            let old = args.pop().unwrap();
            let mut ops = Vec::new();
            diff_vnode(&old, &new, "root", &mut ops);
            Ok(Value::List(Arc::new(ops)))
        }),
    );
    fields.insert(
        "patchToJson".to_string(),
        builtin("ui.patchToJson", 1, |mut args, _runtime| {
            let ops = args.pop().unwrap();
            let json = patch_ops_to_json_text(&ops)?;
            Ok(Value::Text(json))
        }),
    );
    fields.insert(
        "eventFromJson".to_string(),
        builtin("ui.eventFromJson", 1, |mut args, _runtime| {
            let text = expect_text(args.pop().unwrap(), "ui.eventFromJson")?;
            match decode_event(&text) {
                Ok(value) => Ok(Value::Constructor {
                    name: "Ok".to_string(),
                    args: vec![value],
                }),
                Err(msg) => Ok(Value::Constructor {
                    name: "Err".to_string(),
                    args: vec![live_error_value(&msg)],
                }),
            }
        }),
    );
    fields.insert(
        "live".to_string(),
        builtin("ui.live", 4, |mut args, runtime| {
            let update = args.pop().unwrap();
            let view = args.pop().unwrap();
            let initial_model = args.pop().unwrap();
            let cfg = args.pop().unwrap();
            ui_live(cfg, initial_model, view, update, runtime)
        }),
    );
    fields.insert("ServerHtml".to_string(), build_server_html_record());
    Value::Record(Arc::new(fields))
}

fn ui_live(
    cfg: Value,
    initial_model: Value,
    view: Value,
    update: Value,
    runtime: &mut Runtime,
) -> Result<Value, RuntimeError> {
    let record = expect_record(cfg, "ui.live expects LiveConfig record")?;
    let address = match record.get("address") {
        Some(Value::Text(t)) => t.clone(),
        _ => {
            return Err(RuntimeError::Error(live_error_value(
                "LiveConfig.address must be Text",
            )))
        }
    };
    let path = match record.get("path") {
        Some(Value::Text(t)) => t.clone(),
        _ => {
            return Err(RuntimeError::Error(live_error_value(
                "LiveConfig.path must be Text",
            )))
        }
    };
    let title = match record.get("title") {
        Some(Value::Text(t)) => t.clone(),
        _ => {
            return Err(RuntimeError::Error(live_error_value(
                "LiveConfig.title must be Text",
            )))
        }
    };

    let addr = SocketAddr::from_str(address.trim())
        .map_err(|err| RuntimeError::Error(live_error_value(&format!("invalid address: {err}"))))?;

    let ws_path = live_ws_path(&path);
    let ctx = runtime.ctx.clone();
    let view_value = view.clone();
    let update_value = update.clone();
    let initial_model_value = initial_model.clone();

    let effect = EffectValue::Thunk {
        func: Arc::new(move |_| {
            let view_value = view_value.clone();
            let update_value = update_value.clone();
            let initial_model_value = initial_model_value.clone();
            let ctx_clone = ctx.clone();
            let http_path = normalize_path(&path);
            let ws_path = ws_path.clone();
            let title = title.clone();

            let handler: Handler = Arc::new(move |req: AiviRequest| {
                let view_value = view_value.clone();
                let update_value = update_value.clone();
                let initial_model_value = initial_model_value.clone();
                let ctx_for_req = ctx_clone.clone();
                let http_path = http_path.clone();
                let ws_path = ws_path.clone();
                let title = title.clone();

                Box::pin(async move {
                    // HTTP initial page.
                    if req.path == http_path {
                        let html = tokio::task::spawn_blocking(move || {
                            let cancel = CancelToken::root();
                            let mut runtime = Runtime::with_cancel(ctx_for_req.clone(), cancel);
                            let vnode = runtime.apply(view_value, initial_model_value)?;
                            let (body, _handlers) = render_vnode(&vnode, "root");
                            Ok::<_, RuntimeError>(live_html_page(&title, &ws_path, &body))
                        })
                        .await
                        .map_err(|err| AiviHttpError {
                            message: err.to_string(),
                        })?
                        .map_err(|err| AiviHttpError {
                            message: runtime_error_to_text(err),
                        })?;

                        let resp = AiviResponse {
                            status: 200,
                            headers: vec![("content-type".to_string(), "text/html".to_string())],
                            body: html.into_bytes(),
                        };
                        return Ok(ServerReply::Http(resp));
                    }

                    // WebSocket endpoint.
                    if req.path == ws_path {
                        let ws_handler = Arc::new(move |socket| {
                            let ctx = ctx_for_req.clone();
                            let view_value = view_value.clone();
                            let update_value = update_value.clone();
                            let initial_model_value = initial_model_value.clone();
                            let future: WsHandlerFuture = Box::pin(async move {
                                let result = tokio::task::spawn_blocking(move || {
                                    run_ws_session(
                                        ctx,
                                        socket,
                                        initial_model_value,
                                        view_value,
                                        update_value,
                                    )
                                })
                                .await
                                .map_err(|err| AiviHttpError {
                                    message: err.to_string(),
                                })?;
                                result.map_err(|err| AiviHttpError {
                                    message: runtime_error_to_text(err),
                                })
                            });
                            future
                        });
                        return Ok(ServerReply::Ws(ws_handler));
                    }

                    // Fallback 404.
                    let resp = AiviResponse {
                        status: 404,
                        headers: vec![("content-type".to_string(), "text/plain".to_string())],
                        body: b"not found".to_vec(),
                    };
                    Ok(ServerReply::Http(resp))
                })
            });

            let server = aivi_http_server::start_server(addr, handler)
                .map_err(|err| RuntimeError::Error(live_error_value(&err.message)))?;
            Ok(Value::HttpServer(Arc::new(server)))
        }),
    };

    Ok(Value::Effect(Arc::new(effect)))
}

fn run_ws_session(
    ctx: Arc<RuntimeContext>,
    socket: WebSocketHandle,
    initial_model: Value,
    view: Value,
    update: Value,
) -> Result<(), RuntimeError> {
    let cancel = CancelToken::root();
    let mut runtime = Runtime::with_cancel(ctx.clone(), cancel);

    let mut model = initial_model;
    let mut vnode = runtime.apply(view.clone(), model.clone())?;
    let mut handlers = collect_handlers(&vnode, "root");

    // No need to send an init message: the initial HTML is delivered via HTTP.
    loop {
        let msg = socket
            .recv()
            .map_err(|err| RuntimeError::Message(err.message))?;
        let text = match msg {
            AiviWsMessage::TextMsg(t) => t,
            AiviWsMessage::Close => break,
            _ => continue,
        };
        let event = decode_event_raw(&text).map_err(RuntimeError::Message)?;

        let (event_id, payload) = match event {
            DecodedEvent::Click(id) => (id, None),
            DecodedEvent::Input(id, value) => (id, Some(value)),
        };

        let Some(handler) = handlers.get(&event_id).cloned() else {
            continue;
        };

        let msg_value = match (handler, payload) {
            (UiHandler::Click(msg), _) => msg,
            (UiHandler::Input(f), Some(value)) => runtime.apply(f, Value::Text(value))?,
            (UiHandler::Input(_), None) => continue,
        };

        let update_fn = runtime.apply(update.clone(), msg_value)?;
        model = runtime.apply(update_fn, model)?; // update : msg -> model -> model (curried)

        let new_vnode = runtime.apply(view.clone(), model.clone())?;
        let mut ops = Vec::new();
        diff_vnode(&vnode, &new_vnode, "root", &mut ops);
        vnode = new_vnode;
        handlers = collect_handlers(&vnode, "root");

        let json_ops = patch_ops_to_json_text(&Value::List(Arc::new(ops)))?;
        let payload = format!("{{\"t\":\"patch\",\"ops\":{}}}", json_ops);
        socket
            .send(AiviWsMessage::TextMsg(payload))
            .map_err(|err| RuntimeError::Message(err.message))?;
    }

    Ok(())
}

fn runtime_error_to_text(err: RuntimeError) -> String {
    match err {
        RuntimeError::Cancelled => "cancelled".to_string(),
        RuntimeError::Message(m) => m,
        RuntimeError::Error(v) => format_value(&v),
    }
}

fn normalize_path(path: &str) -> String {
    let p = path.trim();
    if p.is_empty() {
        return "/".to_string();
    }
    if p.starts_with('/') {
        p.to_string()
    } else {
        format!("/{p}")
    }
}

fn live_ws_path(path: &str) -> String {
    let http = normalize_path(path);
    if http == "/" {
        "/ws".to_string()
    } else {
        format!("{http}/ws")
    }
}

fn live_html_page(title: &str, ws_path: &str, body_html: &str) -> String {
    format!(
        "<!doctype html>\
<html><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>{}</title>\
</head><body>\
<div id=\"aivi-root\">{}</div>\
<script>{}</script>\
</body></html>",
        escape_html_text(title),
        body_html,
        live_client_js(ws_path)
    )
}

fn live_client_js(ws_path: &str) -> String {
    let ws_path = ws_path.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "(function(){{\
const wsUrl=(location.protocol==='https:'?'wss://':'ws://')+location.host+\"{ws_path}\";\
const socket=new WebSocket(wsUrl);\
function send(obj){{ try{{socket.send(JSON.stringify(obj));}}catch(_){{}} }}\
function closestWithAttr(el,attr){{ while(el&&el!==document.body){{ if(el.getAttribute&&el.getAttribute(attr)) return el; el=el.parentNode; }} return null; }}\
document.addEventListener('click',function(ev){{ const el=closestWithAttr(ev.target,'data-aivi-onclick'); if(!el) return; const id=parseInt(el.getAttribute('data-aivi-onclick'),10); if(!isFinite(id)) return; send({{t:'click',id:id}}); }});\
document.addEventListener('input',function(ev){{ const el=closestWithAttr(ev.target,'data-aivi-oninput'); if(!el) return; const id=parseInt(el.getAttribute('data-aivi-oninput'),10); if(!isFinite(id)) return; const v=('value'in ev.target)?String(ev.target.value):''; send({{t:'input',id:id,value:v}}); }});\
function findNode(id){{ return document.querySelector('[data-aivi-node=\"'+CSS.escape(id)+'\"]'); }}\
function applyOp(op){{\
  if(op.op==='replace'){{ const node=findNode(op.id); if(!node) return; node.outerHTML=op.html; return; }}\
  if(op.op==='setText'){{ const node=findNode(op.id); if(!node) return; node.textContent=op.text; return; }}\
  if(op.op==='setAttr'){{ const node=findNode(op.id); if(!node) return; node.setAttribute(op.name,op.value); return; }}\
  if(op.op==='removeAttr'){{ const node=findNode(op.id); if(!node) return; node.removeAttribute(op.name); return; }}\
}}\
socket.addEventListener('message',function(ev){{\
  let msg=null; try{{ msg=JSON.parse(ev.data); }}catch(_){{ return; }}\
  if(!msg||msg.t!=='patch'||!Array.isArray(msg.ops)) return;\
  for(const op of msg.ops) applyOp(op);\
}});\
}})();"
    )
}

struct RenderState {
    handlers: HashMap<i64, UiHandler>,
}

fn render_vnode(vnode: &Value, node_id: &str) -> (String, HashMap<i64, UiHandler>) {
    let mut state = RenderState {
        handlers: HashMap::new(),
    };
    let html = render_vnode_inner(vnode, node_id, None, &mut state);
    (html, state.handlers)
}

fn collect_handlers(vnode: &Value, node_id: &str) -> HashMap<i64, UiHandler> {
    let (_html, handlers) = render_vnode(vnode, node_id);
    handlers
}

fn render_vnode_inner(
    vnode: &Value,
    node_id: &str,
    keyed: Option<&str>,
    state: &mut RenderState,
) -> String {
    match vnode {
        Value::Constructor { name, args } if name == "TextNode" && args.len() == 1 => {
            let text = match &args[0] {
                Value::Text(t) => t.clone(),
                other => format_value(other),
            };
            let mut attrs = format!(" data-aivi-node=\"{}\"", escape_attr_value(node_id));
            if let Some(key) = keyed {
                attrs.push_str(&format!(" data-aivi-key=\"{}\"", escape_attr_value(key)));
            }
            format!(
                "<span{attrs}>{}</span>",
                escape_html_text(&text),
                attrs = attrs
            )
        }
        Value::Constructor { name, args } if name == "Keyed" && args.len() == 2 => {
            let key = match &args[0] {
                Value::Text(t) => t.clone(),
                other => format_value(other),
            };
            render_vnode_inner(&args[1], node_id, Some(&key), state)
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
            attrs.push_str(&render_attrs(attrs_value, node_id, state));

            let mut children_html = String::new();
            if let Value::List(items) = children_value {
                for (idx, child) in items.iter().enumerate() {
                    let seg = child_segment(child, idx);
                    let child_id = format!("{}/{}", node_id, seg);
                    children_html.push_str(&render_vnode_inner(child, &child_id, None, state));
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
            escape_html_text(&format_value(other))
        ),
    }
}

fn sanitize_tag(tag: &str) -> String {
    if tag.is_empty() {
        return "div".to_string();
    }
    if tag
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':'))
    {
        return tag.to_string();
    }
    "div".to_string()
}

fn child_segment(child: &Value, index: usize) -> String {
    if let Value::Constructor { name, args } = child {
        if name == "Keyed" && args.len() == 2 {
            if let Value::Text(key) = &args[0] {
                return format!("k:{}", key);
            }
        }
    }
    index.to_string()
}

fn render_attrs(attrs: &Value, node_id: &str, state: &mut RenderState) -> String {
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
            Value::Constructor { name, args } if name == "OnClick" && args.len() == 1 => {
                let id = event_id("click", node_id);
                state.handlers.insert(id, UiHandler::Click(args[0].clone()));
                out.push_str(&format!(" data-aivi-onclick=\"{}\"", id));
            }
            Value::Constructor { name, args } if name == "OnInput" && args.len() == 1 => {
                let id = event_id("input", node_id);
                state.handlers.insert(id, UiHandler::Input(args[0].clone()));
                out.push_str(&format!(" data-aivi-oninput=\"{}\"", id));
            }
            _ => {}
        }
    }
    out
}

fn is_safe_attr_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':'))
}

fn style_record_to_text(value: &Value) -> String {
    let Value::Record(fields) = value else {
        return String::new();
    };
    let mut keys: Vec<&String> = fields.keys().collect();
    keys.sort();
    let mut parts: Vec<String> = Vec::new();
    for k in keys {
        if !is_safe_css_prop(k) {
            continue;
        }
        let Some(v) = fields.get(k) else {
            continue;
        };
        let rendered = css_value_to_text(v);
        if rendered.is_empty() {
            continue;
        }
        parts.push(format!("{k}: {rendered}"));
    }
    parts.join("; ")
}

fn is_safe_css_prop(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn css_value_to_text(value: &Value) -> String {
    match value {
        Value::Text(t) => t.clone(),
        Value::Int(v) => v.to_string(),
        Value::Float(v) => trim_float(*v),
        Value::Bool(true) => "true".to_string(),
        Value::Bool(false) => "false".to_string(),
        Value::Constructor { name, args } if args.len() == 1 => match (name.as_str(), &args[0]) {
            ("Px", Value::Int(v)) => format!("{v}px"),
            ("Px", Value::Float(v)) => format!("{}px", trim_float(*v)),
            ("Em", Value::Int(v)) => format!("{v}em"),
            ("Em", Value::Float(v)) => format!("{}em", trim_float(*v)),
            ("Rem", Value::Int(v)) => format!("{v}rem"),
            ("Rem", Value::Float(v)) => format!("{}rem", trim_float(*v)),
            ("Vh", Value::Int(v)) => format!("{v}vh"),
            ("Vh", Value::Float(v)) => format!("{}vh", trim_float(*v)),
            ("Vw", Value::Int(v)) => format!("{v}vw"),
            ("Vw", Value::Float(v)) => format!("{}vw", trim_float(*v)),
            ("Pct", Value::Int(v)) => format!("{v}%"),
            ("Pct", Value::Float(v)) => format!("{}%", trim_float(*v)),
            _ => format_value(value),
        },
        Value::Record(fields) => {
            if let (Some(Value::Int(r)), Some(Value::Int(g)), Some(Value::Int(b))) =
                (fields.get("r"), fields.get("g"), fields.get("b"))
            {
                return format!(
                    "#{:02x}{:02x}{:02x}",
                    clamp_u8(*r),
                    clamp_u8(*g),
                    clamp_u8(*b)
                );
            }
            format_value(value)
        }
        other => format_value(other),
    }
}

fn clamp_u8(v: i64) -> u8 {
    if v < 0 {
        0
    } else if v > 255 {
        255
    } else {
        v as u8
    }
}

fn trim_float(v: f64) -> String {
    let mut s = v.to_string();
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

fn escape_html_text(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_attr_value(text: &str) -> String {
    escape_html_text(text)
}

fn event_id(kind: &str, node_id: &str) -> i64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in kind
        .as_bytes()
        .iter()
        .chain([b':'].iter())
        .chain(node_id.as_bytes().iter())
    {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    (hash & 0x7fff_ffff_ffff_ffff) as i64
}
