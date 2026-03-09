use std::collections::HashMap;
use std::sync::Arc;

use super::util::{builtin, expect_text};
use crate::runtime::{EffectValue, RuntimeError, Value};

fn decode_text(value: &Value) -> Option<String> {
    match value {
        Value::Text(text) => Some(text.clone()),
        Value::Int(value) => Some(value.to_string()),
        Value::Float(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::DateTime(value) => Some(value.clone()),
        _ => None,
    }
}

/// Create a stub gtk4 builtin that returns an error effect.
fn gtk4_stub(name: &'static str, arity: usize) -> Value {
    let full_name = format!("gtk4.{name}");
    builtin(&full_name, arity, move |_args, _| {
        let msg = format!("gtk4.{name}: GTK4 runtime is not available");
        Ok(Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new(move |_| Err(RuntimeError::Error(Value::Text(msg.clone())))),
        })))
    })
}

fn reactive_init_builtin() -> Value {
    builtin("gtk4.reactiveInit", 1, |mut args, _| {
        let model = args.remove(0);
        Ok(Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new(move |runtime| {
                runtime.reactive_init_host(model.clone());
                Ok(Value::Unit)
            }),
        })))
    })
}

fn reactive_commit_builtin() -> Value {
    builtin("gtk4.reactiveCommit", 2, |mut args, _| {
        let next = args.remove(1);
        let previous = args.remove(0);
        Ok(Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new(move |runtime| {
                runtime.reactive_commit_host(previous.clone(), next.clone());
                Ok(Value::Unit)
            }),
        })))
    })
}

fn derive_builtin() -> Value {
    builtin("gtk4.derive", 1, |mut args, _| {
        let derive = args.remove(0);
        Ok(builtin("gtk4.derive.read", 1, move |mut args, runtime| {
            let model = args.remove(0);
            runtime.apply(derive.clone(), model)
        }))
    })
}

fn memo_builtin() -> Value {
    builtin("gtk4.memo", 2, |mut args, _| {
        let derive = args.remove(1);
        let key = expect_text(args.remove(0), "gtk4.memo key")?;
        Ok(builtin("gtk4.memo.read", 1, move |mut args, runtime| {
            let model = args.remove(0);
            runtime.reactive_read_computed(&key, derive.clone(), model)
        }))
    })
}

fn is_reactive_signal(value: &Value) -> bool {
    matches!(
        value,
        Value::Builtin(builtin)
            if builtin.args.is_empty()
                && builtin.imp.arity == 1
                && matches!(
                    builtin.imp.name.as_str(),
                    "gtk4.derive.read" | "gtk4.memo.read"
                )
    )
}

pub(super) fn resolve_reactive_attr_value(
    value: Value,
    runtime: &mut crate::runtime::Runtime,
) -> Result<Value, RuntimeError> {
    let value = runtime.force_value(value)?;
    if matches!(value, Value::Signal(_)) {
        return runtime.reactive_get_signal(value);
    }
    if !is_reactive_signal(&value) {
        return Ok(value);
    }

    let model = runtime
        .reactive_host
        .as_ref()
        .map(|state| state.current_model.clone())
        .ok_or_else(|| RuntimeError::InvalidArgument {
            context: "gtk4 reactive binding".to_string(),
            reason:
                "derived values inside gtk sigils require an initialized reactive host"
                    .to_string(),
        })?;
    runtime.apply(value, model)
}

fn serialize_attr_text(value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Text(text) => Ok(text.clone()),
        Value::Int(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Float(value) => Ok(value.to_string()),
        Value::DateTime(value) => Ok(value.clone()),
        Value::BigInt(value) => Ok(value.to_string()),
        Value::Rational(value) => Ok(value.to_string()),
        Value::Decimal(value) => Ok(value.to_string()),
        Value::Constructor { name, args } if args.is_empty() => Ok(name.clone()),
        Value::Constructor { name, args } => {
            let values = args
                .iter()
                .map(crate::runtime::format_value)
                .collect::<Vec<_>>();
            Ok(format!("{}({})", name, values.join(", ")))
        }
        other => Err(RuntimeError::TypeError {
            context: "gtk4.serializeAttr".to_string(),
            expected: "Text-compatible value".to_string(),
            got: crate::runtime::format_value(other),
        }),
    }
}

fn serialize_attr_builtin() -> Value {
    builtin("gtk4.serializeAttr", 1, |mut args, runtime| {
        let value = args.remove(0);
        let resolved = resolve_reactive_attr_value(value, runtime)?;
        Ok(Value::Text(serialize_attr_text(&resolved)?))
    })
}

fn capture_binding_builtin() -> Value {
    builtin("gtk4.captureBinding", 1, |mut args, runtime| {
        let value = args.remove(0);
        Ok(Value::Int(runtime.ctx.capture_gtk_binding(value)))
    })
}

fn serialize_signal_text(value: &Value) -> Option<String> {
    Some(match value {
        Value::Text(text) => text.clone(),
        Value::Int(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Float(value) => value.to_string(),
        Value::Constructor { name, args } if args.is_empty() => name.clone(),
        Value::Constructor { name, args } => {
            let values = args
                .iter()
                .map(crate::runtime::format_value)
                .collect::<Vec<_>>();
            format!("{}({})", name, values.join(","))
        }
        _ => return None,
    })
}

fn serialize_signal_builtin() -> Value {
    builtin("gtk4.serializeSignal", 1, |mut args, _| {
        let value = args.remove(0);
        Ok(Value::Text(
            serialize_signal_text(&value).unwrap_or_default(),
        ))
    })
}

#[derive(Clone)]
pub(super) enum ResolvedGtkAttr {
    StaticAttr { name: String, value: String },
    BoundAttr { name: String, value: Value },
    StaticProp { name: String, value: String },
    BoundProp { name: String, value: Value },
    EventProp { name: String, handler: Value },
    Id(String),
    Ref(String),
}

#[derive(Clone)]
pub(super) enum ResolvedGtkNode {
    Element {
        tag: String,
        attrs: Vec<ResolvedGtkAttr>,
        children: Vec<ResolvedGtkNode>,
    },
    Text(String),
    DynamicText(Value),
    Show {
        when: Value,
        child: Box<ResolvedGtkNode>,
    },
    Each {
        items: Value,
        template: Value,
        _key: Option<Value>,
    },
}

fn decode_binding_handle(
    value: &Value,
    runtime: &crate::runtime::Runtime,
    context: &str,
) -> Result<Value, RuntimeError> {
    let Value::Int(handle) = value else {
        return Err(RuntimeError::TypeError {
            context: context.to_string(),
            expected: "GtkBindingHandle".to_string(),
            got: crate::runtime::format_value(value),
        });
    };
    runtime
        .ctx
        .resolve_gtk_binding(*handle)
        .ok_or_else(|| RuntimeError::InvalidArgument {
            context: context.to_string(),
            reason: format!("unknown gtk binding handle {handle}"),
        })
}

fn decode_resolved_attr(
    value: &Value,
    runtime: &crate::runtime::Runtime,
) -> Result<ResolvedGtkAttr, RuntimeError> {
    let Value::Constructor { name, args } = value else {
        return Err(RuntimeError::Message(
            "gtk4 expected a GtkAttr constructor".to_string(),
        ));
    };
    match (name.as_str(), args.as_slice()) {
        ("GtkStaticAttr", [name, value]) => Ok(ResolvedGtkAttr::StaticAttr {
            name: decode_text(name).ok_or_else(|| {
                RuntimeError::Message("gtk4 invalid static attr name".to_string())
            })?,
            value: decode_text(value).ok_or_else(|| {
                RuntimeError::Message("gtk4 invalid static attr value".to_string())
            })?,
        }),
        ("GtkBoundAttr", [name, value]) => Ok(ResolvedGtkAttr::BoundAttr {
            name: decode_text(name)
                .ok_or_else(|| RuntimeError::Message("gtk4 invalid bound attr name".to_string()))?,
            value: decode_binding_handle(value, runtime, "gtk4 bound attr")?,
        }),
        ("GtkStaticProp", [name, value]) => Ok(ResolvedGtkAttr::StaticProp {
            name: decode_text(name).ok_or_else(|| {
                RuntimeError::Message("gtk4 invalid static prop name".to_string())
            })?,
            value: decode_text(value).ok_or_else(|| {
                RuntimeError::Message("gtk4 invalid static prop value".to_string())
            })?,
        }),
        ("GtkBoundProp", [name, value]) => Ok(ResolvedGtkAttr::BoundProp {
            name: decode_text(name)
                .ok_or_else(|| RuntimeError::Message("gtk4 invalid bound prop name".to_string()))?,
            value: decode_binding_handle(value, runtime, "gtk4 bound prop")?,
        }),
        ("GtkEventProp", [name, handler]) => Ok(ResolvedGtkAttr::EventProp {
            name: decode_text(name)
                .ok_or_else(|| RuntimeError::Message("gtk4 invalid event prop name".to_string()))?,
            handler: decode_binding_handle(handler, runtime, "gtk4 event prop")?,
        }),
        ("GtkIdAttr", [value]) => Ok(ResolvedGtkAttr::Id(
            decode_text(value)
                .ok_or_else(|| RuntimeError::Message("gtk4 invalid id attr".to_string()))?,
        )),
        ("GtkRefAttr", [value]) => Ok(ResolvedGtkAttr::Ref(
            decode_text(value)
                .ok_or_else(|| RuntimeError::Message("gtk4 invalid ref attr".to_string()))?,
        )),
        ("GtkAttribute", [name, value]) => {
            let key = decode_text(name).ok_or_else(|| {
                RuntimeError::Message("gtk4 invalid legacy attr name".to_string())
            })?;
            let raw = decode_text(value).ok_or_else(|| {
                RuntimeError::Message("gtk4 invalid legacy attr value".to_string())
            })?;
            if key == "id" {
                Ok(ResolvedGtkAttr::Id(raw))
            } else if key == "ref" {
                Ok(ResolvedGtkAttr::Ref(raw))
            } else if let Some(signal_name) = key.strip_prefix("signal:") {
                Ok(ResolvedGtkAttr::StaticAttr {
                    name: format!("signal:{signal_name}"),
                    value: raw,
                })
            } else if let Some(prop_name) = key.strip_prefix("prop:") {
                Ok(ResolvedGtkAttr::StaticProp {
                    name: prop_name.to_string(),
                    value: raw,
                })
            } else {
                Ok(ResolvedGtkAttr::StaticAttr {
                    name: key,
                    value: raw,
                })
            }
        }
        _ => Err(RuntimeError::Message(format!(
            "gtk4 expected a GtkAttr constructor, got {name}"
        ))),
    }
}

pub(super) fn resolve_gtk_node(
    node: &Value,
    runtime: &crate::runtime::Runtime,
) -> Result<ResolvedGtkNode, RuntimeError> {
    let Value::Constructor { name, args } = node else {
        return Err(RuntimeError::Message(
            "gtk4.buildFromNode expects GtkNode".to_string(),
        ));
    };
    match (name.as_str(), args.as_slice()) {
        ("GtkTextNode", [text]) => {
            Ok(ResolvedGtkNode::Text(decode_text(text).ok_or_else(
                || RuntimeError::Message("gtk4 invalid GtkTextNode text".to_string()),
            )?))
        }
        ("GtkBoundText", [value]) => Ok(ResolvedGtkNode::DynamicText(decode_binding_handle(
            value,
            runtime,
            "gtk4 bound text",
        )?)),
        ("GtkShowNode", [when, child]) => Ok(ResolvedGtkNode::Show {
            when: decode_binding_handle(when, runtime, "gtk4 show binding")?,
            child: Box::new(resolve_gtk_node(child, runtime)?),
        }),
        ("GtkEachNode", [items, template, key]) => {
            let key = match key {
                Value::Constructor { name, args } if name == "None" && args.is_empty() => None,
                Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
                    Some(decode_binding_handle(&args[0], runtime, "gtk4 each key")?)
                }
                other => {
                    return Err(RuntimeError::TypeError {
                        context: "gtk4 each key".to_string(),
                        expected: "Option GtkBindingHandle".to_string(),
                        got: crate::runtime::format_value(other),
                    })
                }
            };
            Ok(ResolvedGtkNode::Each {
                items: decode_binding_handle(items, runtime, "gtk4 each items")?,
                template: decode_binding_handle(template, runtime, "gtk4 each template")?,
                _key: key,
            })
        }
        ("GtkElement", [tag, attrs, children]) => {
            let tag = decode_text(tag)
                .ok_or_else(|| RuntimeError::Message("gtk4 invalid GtkElement tag".to_string()))?;
            let Value::List(attrs) = attrs else {
                return Err(RuntimeError::Message(
                    "gtk4 GtkElement attrs must be a List".to_string(),
                ));
            };
            let Value::List(children) = children else {
                return Err(RuntimeError::Message(
                    "gtk4 GtkElement children must be a List".to_string(),
                ));
            };
            let attrs = attrs
                .iter()
                .map(|attr| decode_resolved_attr(attr, runtime))
                .collect::<Result<Vec<_>, _>>()?;
            let children = children
                .iter()
                .map(|child| resolve_gtk_node(child, runtime))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ResolvedGtkNode::Element {
                tag,
                attrs,
                children,
            })
        }
        _ => Err(RuntimeError::Message(
            "gtk4.buildFromNode expects GtkNode".to_string(),
        )),
    }
}

fn collect_auto_bindings_into(
    node: &ResolvedGtkNode,
    runtime: &mut crate::runtime::Runtime,
    named_handlers: &mut HashMap<(String, String), String>,
    unique_handlers_by_signal: &mut HashMap<String, Option<String>>,
) -> Result<(), RuntimeError> {
    match node {
        ResolvedGtkNode::Text(_) | ResolvedGtkNode::DynamicText(_) => Ok(()),
        ResolvedGtkNode::Show { child, .. } => {
            collect_auto_bindings_into(child, runtime, named_handlers, unique_handlers_by_signal)
        }
        ResolvedGtkNode::Each {
            items,
            template,
            _key: _,
        } => {
            let items = resolve_reactive_attr_value(items.clone(), runtime)?;
            let items = runtime.force_value(items)?;
            let Value::List(items) = items else {
                return Err(RuntimeError::TypeError {
                    context: "gtk4.autoBindingsSet each items".to_string(),
                    expected: "List".to_string(),
                    got: crate::runtime::format_value(&items),
                });
            };
            for item in items.iter() {
                let node = runtime.apply(template.clone(), item.clone())?;
                let node = runtime.force_value(node)?;
                let node = resolve_gtk_node(&node, runtime)?;
                collect_auto_bindings_into(
                    &node,
                    runtime,
                    named_handlers,
                    unique_handlers_by_signal,
                )?;
            }
            Ok(())
        }
        ResolvedGtkNode::Element {
            attrs, children, ..
        } => {
            let mut widget_name = String::new();
            let mut signal_handlers = Vec::new();
            for attr in attrs {
                match attr {
                    ResolvedGtkAttr::Id(value) => widget_name = value.clone(),
                    ResolvedGtkAttr::EventProp { name, handler } => {
                        if let Some(serialized) = serialize_signal_text(handler) {
                            signal_handlers.push((name.clone(), serialized));
                        }
                    }
                    ResolvedGtkAttr::StaticAttr { name, value } if name.starts_with("signal:") => {
                        signal_handlers.push((
                            name.trim_start_matches("signal:").to_string(),
                            value.clone(),
                        ));
                    }
                    _ => {}
                }
            }

            for (signal_name, handler) in signal_handlers {
                if !widget_name.is_empty() {
                    named_handlers
                        .insert((widget_name.clone(), signal_name.clone()), handler.clone());
                }
                unique_handlers_by_signal
                    .entry(signal_name)
                    .and_modify(|existing| match existing {
                        Some(existing_handler) if existing_handler == &handler => {}
                        _ => *existing = None,
                    })
                    .or_insert_with(|| Some(handler));
            }

            for child in children {
                collect_auto_bindings_into(
                    child,
                    runtime,
                    named_handlers,
                    unique_handlers_by_signal,
                )?;
            }
            Ok(())
        }
    }
}

fn auto_bindings_set_builtin() -> Value {
    builtin("gtk4.autoBindingsSet", 1, |mut args, _| {
        let node = args.remove(0);
        Ok(Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new(move |runtime| {
                let node = runtime.force_value(node.clone())?;
                let node = resolve_gtk_node(&node, runtime)?;
                let mut named_handlers = HashMap::new();
                let mut unique_handlers_by_signal = HashMap::new();
                collect_auto_bindings_into(
                    &node,
                    runtime,
                    &mut named_handlers,
                    &mut unique_handlers_by_signal,
                )?;
                *runtime.ctx.gtk_auto_bindings.write() =
                    crate::runtime::environment::GtkAutoBindingsState {
                        named_handlers,
                        unique_handlers_by_signal,
                    };
                Ok(Value::Unit)
            }),
        })))
    })
}

fn parse_auto_arg(text: &str) -> Value {
    if text == "True" {
        return Value::Bool(true);
    }
    if text == "False" {
        return Value::Bool(false);
    }
    if let Ok(value) = text.parse::<i64>() {
        return Value::Int(value);
    }
    if let Ok(value) = text.parse::<f64>() {
        return Value::Float(value);
    }
    Value::Text(text.to_string())
}

fn split_auto_args(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for ch in text.chars() {
        match ch {
            '(' | '[' | '{' => {
                depth = depth.saturating_add(1);
                current.push(ch);
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if depth == 0 => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }
    parts
}

fn construct_auto_message(handler: &str, payload: Option<Value>) -> Option<Value> {
    if handler.is_empty() {
        return None;
    }
    if let Some(paren_pos) = handler.find('(') {
        if !handler.ends_with(')') {
            return None;
        }
        let name = handler[..paren_pos].trim();
        if name.is_empty() {
            return None;
        }
        let inner = &handler[paren_pos + 1..handler.len().saturating_sub(1)];
        let args = split_auto_args(inner)
            .into_iter()
            .map(|arg| parse_auto_arg(&arg))
            .collect::<Vec<_>>();
        return Some(Value::Constructor {
            name: name.to_string(),
            args,
        });
    }

    let args = payload.into_iter().collect::<Vec<_>>();
    Some(Value::Constructor {
        name: handler.trim().to_string(),
        args,
    })
}

fn auto_to_msg_builtin() -> Value {
    builtin("gtk4.autoToMsg", 1, |mut args, runtime| {
        let event = runtime.force_value(args.remove(0))?;
        let bindings = runtime.ctx.gtk_auto_bindings.read().clone();
        let (signal_name, widget_name, handler_from_event, payload) = match event {
            Value::Constructor { name, args } if name == "GtkClicked" && args.len() == 2 => (
                "clicked".to_string(),
                decode_text(&args[1]).unwrap_or_default(),
                None,
                None,
            ),
            Value::Constructor { name, args } if name == "GtkInputChanged" && args.len() == 3 => (
                "changed".to_string(),
                decode_text(&args[1]).unwrap_or_default(),
                None,
                decode_text(&args[2]).map(Value::Text),
            ),
            Value::Constructor { name, args } if name == "GtkActivated" && args.len() == 2 => (
                "activate".to_string(),
                decode_text(&args[1]).unwrap_or_default(),
                None,
                None,
            ),
            Value::Constructor { name, args } if name == "GtkToggled" && args.len() == 3 => (
                "toggled".to_string(),
                decode_text(&args[1]).unwrap_or_default(),
                None,
                match &args[2] {
                    Value::Bool(value) => Some(Value::Bool(*value)),
                    _ => None,
                },
            ),
            Value::Constructor { name, args } if name == "GtkValueChanged" && args.len() == 3 => (
                "value-changed".to_string(),
                decode_text(&args[1]).unwrap_or_default(),
                None,
                match &args[2] {
                    Value::Float(value) => Some(Value::Float(*value)),
                    Value::Int(value) => Some(Value::Float(*value as f64)),
                    _ => None,
                },
            ),
            Value::Constructor { name, args } if name == "GtkFocusIn" && args.len() == 2 => (
                "focus-enter".to_string(),
                decode_text(&args[1]).unwrap_or_default(),
                None,
                None,
            ),
            Value::Constructor { name, args } if name == "GtkFocusOut" && args.len() == 2 => (
                "focus-leave".to_string(),
                decode_text(&args[1]).unwrap_or_default(),
                None,
                None,
            ),
            Value::Constructor { name, args } if name == "GtkTick" && args.is_empty() => {
                ("tick".to_string(), String::new(), None, None)
            }
            Value::Constructor { name, args } if name == "GtkUnknownSignal" && args.len() == 5 => (
                decode_text(&args[2]).unwrap_or_default(),
                decode_text(&args[1]).unwrap_or_default(),
                decode_text(&args[3]),
                decode_text(&args[4])
                    .filter(|text| !text.is_empty())
                    .map(Value::Text),
            ),
            _ => {
                return Ok(Value::Constructor {
                    name: "None".to_string(),
                    args: vec![],
                })
            }
        };

        let handler = handler_from_event
            .or_else(|| {
                if widget_name.is_empty() {
                    None
                } else {
                    bindings
                        .named_handlers
                        .get(&(widget_name.clone(), signal_name.clone()))
                        .cloned()
                }
            })
            .or_else(|| {
                bindings
                    .unique_handlers_by_signal
                    .get(&signal_name)
                    .and_then(|handler| handler.clone())
            });

        let Some(message) = handler.and_then(|handler| construct_auto_message(&handler, payload))
        else {
            return Ok(Value::Constructor {
                name: "None".to_string(),
                args: vec![],
            });
        };

        Ok(Value::Constructor {
            name: "Some".to_string(),
            args: vec![message],
        })
    })
}

pub(super) fn build_gtk4_record() -> Value {
    if let Some(real) = super::gtk4_real::build_gtk4_record_real(build_gtk4_stubs) {
        return real;
    }
    build_gtk4_stubs()
}

fn build_gtk4_stubs() -> Value {
    let stubs: &[(&str, usize)] = &[
        ("buildFromNode", 1),
        ("buildWithIds", 1),
        ("reconcileNode", 2),
        ("signalPoll", 1),
        ("signalEmit", 4),
        ("signalStream", 1),
        ("setInterval", 1),
        ("dbusServerStart", 1),
        ("init", 1),
        ("appNew", 1),
        ("appRun", 1),
        ("appSetCss", 2),
        ("windowNew", 4),
        ("windowSetTitle", 2),
        ("windowSetTitlebar", 2),
        ("windowSetChild", 2),
        ("windowPresent", 1),
        ("windowSetHideOnClose", 2),
        ("widgetGetBoolProperty", 2),
        ("widgetShow", 1),
        ("widgetHide", 1),
        ("widgetSetSizeRequest", 3),
        ("widgetSetHexpand", 2),
        ("widgetSetVexpand", 2),
        ("widgetSetHalign", 2),
        ("widgetSetValign", 2),
        ("widgetSetMarginStart", 2),
        ("widgetSetMarginEnd", 2),
        ("widgetSetMarginTop", 2),
        ("widgetSetMarginBottom", 2),
        ("widgetAddCssClass", 2),
        ("widgetRemoveCssClass", 2),
        ("widgetSetTooltipText", 2),
        ("widgetSetOpacity", 2),
        ("widgetSetCss", 2),
        ("widgetAddController", 2),
        ("widgetAddShortcut", 2),
        ("widgetSetLayoutManager", 2),
        ("boxNew", 2),
        ("boxAppend", 2),
        ("boxSetHomogeneous", 2),
        ("buttonNew", 1),
        ("buttonSetLabel", 2),
        ("buttonNewFromIconName", 1),
        ("buttonSetChild", 2),
        ("labelNew", 1),
        ("labelSetText", 2),
        ("labelSetWrap", 2),
        ("labelSetEllipsize", 2),
        ("labelSetXalign", 2),
        ("labelSetMaxWidthChars", 2),
        ("entryNew", 1),
        ("entrySetText", 2),
        ("entryText", 1),
        ("scrollAreaNew", 1),
        ("scrollAreaSetChild", 2),
        ("scrollAreaSetPolicy", 3),
        ("separatorNew", 1),
        ("overlayNew", 1),
        ("overlaySetChild", 2),
        ("overlayAddOverlay", 2),
        ("drawAreaNew", 2),
        ("drawAreaSetContentSize", 3),
        ("drawAreaQueueDraw", 1),
        ("dragSourceNew", 1),
        ("dragSourceSetText", 2),
        ("dropTargetNew", 1),
        ("dropTargetLastText", 1),
        ("menuModelNew", 1),
        ("menuModelAppendItem", 3),
        ("menuButtonNew", 1),
        ("menuButtonSetMenuModel", 2),
        ("dialogNew", 1),
        ("dialogSetTitle", 2),
        ("dialogSetChild", 2),
        ("dialogPresent", 2),
        ("dialogClose", 1),
        ("fileDialogNew", 1),
        ("fileDialogSelectFile", 1),
        ("imageNewFromFile", 1),
        ("imageSetFile", 2),
        ("imageNewFromResource", 1),
        ("imageSetResource", 2),
        ("imageNewFromIconName", 1),
        ("imageSetPixelSize", 2),
        ("iconThemeAddSearchPath", 1),
        ("listStoreNew", 1),
        ("listStoreAppendText", 2),
        ("listStoreItems", 1),
        ("listViewNew", 1),
        ("listViewSetModel", 2),
        ("treeViewNew", 1),
        ("treeViewSetModel", 2),
        ("gestureClickNew", 1),
        ("gestureClickLastButton", 1),
        ("clipboardDefault", 1),
        ("clipboardSetText", 2),
        ("clipboardText", 1),
        ("actionNew", 1),
        ("actionSetEnabled", 2),
        ("appAddAction", 2),
        ("shortcutNew", 2),
        ("notificationNew", 2),
        ("notificationSetBody", 2),
        ("appSendNotification", 3),
        ("appWithdrawNotification", 2),
        ("layoutManagerNew", 1),
        ("osOpenUri", 2),
        ("osShowInFileManager", 1),
        ("osSetBadgeCount", 2),
        ("osThemePreference", 1),
        ("widgetById", 1),
        ("signalBindBoolProperty", 4),
        ("signalBindCssClass", 4),
        ("signalBindToggleBoolProperty", 3),
        ("signalToggleCssClass", 3),
        ("dialogNew", 1),
        ("dialogSetTitle", 2),
        ("dialogSetChild", 2),
        ("dialogPresent", 2),
        ("dialogClose", 1),
        ("signalBindDialogPresent", 3),
        ("signalBindStackPage", 3),
        ("trayIconNew", 2),
        ("trayIconSetTooltip", 2),
        ("trayIconSetVisible", 2),
        ("trayIconSetMenuItems", 2),
        ("trayNotifyPersonalEmail", 4),
        ("traySetEmailSuggestions", 1),
    ];

    let mut fields = HashMap::new();
    fields.insert("reactiveInit".to_string(), reactive_init_builtin());
    fields.insert("reactiveCommit".to_string(), reactive_commit_builtin());
    fields.insert("derive".to_string(), derive_builtin());
    fields.insert("memo".to_string(), memo_builtin());
    fields.insert("autoBindingsSet".to_string(), auto_bindings_set_builtin());
    fields.insert("autoToMsg".to_string(), auto_to_msg_builtin());
    fields.insert("serializeAttr".to_string(), serialize_attr_builtin());
    fields.insert("serializeSignal".to_string(), serialize_signal_builtin());
    fields.insert("captureBinding".to_string(), capture_binding_builtin());
    for &(name, arity) in stubs {
        fields.insert(name.to_string(), gtk4_stub(name, arity));
    }
    Value::Record(Arc::new(fields))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use super::{collect_auto_bindings_into, ResolvedGtkNode};
    use crate::runtime::builtins::util::builtin;
    use crate::runtime::{build_runtime_base, Value};

    #[test]
    fn auto_bindings_collect_handlers_inside_each_templates() {
        let mut runtime = build_runtime_base();
        let handler = Value::Constructor {
            name: "Save".to_string(),
            args: vec![],
        };
        let handler_handle = runtime.ctx.capture_gtk_binding(handler);
        let template = builtin("template", 1, move |mut args, _| {
            let item = args.remove(0);
            let widget_id = match item {
                Value::Text(text) => text,
                other => panic!("expected template item text, got {other:?}"),
            };
            Ok(Value::Constructor {
                name: "GtkElement".to_string(),
                args: vec![
                    Value::Text("object".to_string()),
                    Value::List(Arc::new(vec![
                        Value::Constructor {
                            name: "GtkIdAttr".to_string(),
                            args: vec![Value::Text(widget_id)],
                        },
                        Value::Constructor {
                            name: "GtkEventProp".to_string(),
                            args: vec![
                                Value::Text("clicked".to_string()),
                                Value::Int(handler_handle),
                            ],
                        },
                    ])),
                    Value::List(Arc::new(vec![])),
                ],
            })
        });
        let node = ResolvedGtkNode::Each {
            items: Value::List(Arc::new(vec![Value::Text("saveBtn".to_string())])),
            template,
            _key: None,
        };
        let mut named_handlers = HashMap::new();
        let mut unique_handlers_by_signal = HashMap::new();

        collect_auto_bindings_into(
            &node,
            &mut runtime,
            &mut named_handlers,
            &mut unique_handlers_by_signal,
        )
        .unwrap_or_else(|error| panic!("collect auto bindings failed: {}", error));

        assert_eq!(
            named_handlers.get(&("saveBtn".to_string(), "clicked".to_string())),
            Some(&"Save".to_string())
        );
        assert_eq!(
            unique_handlers_by_signal.get("clicked"),
            Some(&Some("Save".to_string()))
        );
    }
}
