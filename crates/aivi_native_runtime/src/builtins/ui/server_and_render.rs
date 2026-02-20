use std::collections::HashMap;
use std::sync::Arc;

use aivi_http_server::{AiviWsMessage, WebSocketHandle};

use super::util::{builtin, expect_record};
use crate::{format_value, EffectValue, Runtime, RuntimeError, Value};

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
        "ServerHtml".to_string(),
        build_server_html_record(),
    );
    Value::Record(Arc::new(fields))
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
    // Historical helper name: used by the server-driven UI runtimes to route `/.../ws`.
    let p = normalize_path(path);
    if p == "/" {
        "/ws".to_string()
    } else {
        format!("{}/ws", p.trim_end_matches('/'))
    }
}

struct RenderState {
    handlers: HashMap<i64, ()>,
}

fn render_vnode(vnode: &Value, node_id: &str) -> (String, HashMap<i64, ()>) {
    let mut state = RenderState {
        handlers: HashMap::new(),
    };
    let html = render_vnode_inner(vnode, node_id, None, &mut state);
    (html, state.handlers)
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
                render_handler_attr(&mut out, state, node_id, "click");
            }
            Value::Constructor { name, args } if name == "OnInput" && args.len() == 1 => {
                render_handler_attr(&mut out, state, node_id, "input");
            }
            Value::Constructor { name, args } if name == "OnClickE" && args.len() == 1 => {
                render_handler_attr(&mut out, state, node_id, "click");
            }
            Value::Constructor { name, args } if name == "OnInputE" && args.len() == 1 => {
                render_handler_attr(&mut out, state, node_id, "input");
            }
            Value::Constructor { name, args } if name == "OnKeyDown" && args.len() == 1 => {
                render_handler_attr(&mut out, state, node_id, "keydown");
            }
            Value::Constructor { name, args } if name == "OnKeyUp" && args.len() == 1 => {
                render_handler_attr(&mut out, state, node_id, "keyup");
            }
            Value::Constructor { name, args } if name == "OnPointerDown" && args.len() == 1 => {
                render_handler_attr(&mut out, state, node_id, "pointerdown");
            }
            Value::Constructor { name, args } if name == "OnPointerUp" && args.len() == 1 => {
                render_handler_attr(&mut out, state, node_id, "pointerup");
            }
            Value::Constructor { name, args } if name == "OnPointerMove" && args.len() == 1 => {
                render_handler_attr(&mut out, state, node_id, "pointermove");
            }
            Value::Constructor { name, args } if name == "OnFocus" && args.len() == 1 => {
                render_handler_attr(&mut out, state, node_id, "focus");
            }
            Value::Constructor { name, args } if name == "OnBlur" && args.len() == 1 => {
                render_handler_attr(&mut out, state, node_id, "blur");
            }
            _ => {}
        }
    }
    out
}

fn render_handler_attr(out: &mut String, state: &mut RenderState, node_id: &str, kind: &str) {
    let id = event_id(kind, node_id);
    state.handlers.insert(id, ());
    out.push_str(&format!(" data-aivi-hid-{}=\"{}\"", kind, id));
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
