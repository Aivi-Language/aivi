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

fn signal_builtin() -> Value {
    builtin("gtk4.signal", 1, |mut args, _| {
        let derive = args.remove(0);
        Ok(builtin("gtk4.signal.read", 1, move |mut args, runtime| {
            let model = args.remove(0);
            runtime.apply(derive.clone(), model)
        }))
    })
}

fn computed_builtin() -> Value {
    builtin("gtk4.computed", 2, |mut args, _| {
        let derive = args.remove(1);
        let key = expect_text(args.remove(0), "gtk4.computed key")?;
        Ok(builtin("gtk4.computed.read", 1, move |mut args, runtime| {
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
                    "gtk4.signal.read" | "gtk4.computed.read"
                )
    )
}

fn resolve_reactive_attr_value(value: Value, runtime: &mut crate::runtime::Runtime) -> Result<Value, RuntimeError> {
    let value = runtime.force_value(value)?;
    if !is_reactive_signal(&value) {
        return Ok(value);
    }

    let model = runtime
        .reactive_host
        .as_ref()
        .map(|state| state.current_model.clone())
        .ok_or_else(|| RuntimeError::InvalidArgument {
            context: "gtk4 reactive binding".to_string(),
            reason: "signal values inside gtk sigils require gtkApp or an initialized reactive host".to_string(),
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

fn each_items_builtin() -> Value {
    builtin("gtk4.eachItems", 2, |mut args, runtime| {
        let template = args.remove(1);
        let items = resolve_reactive_attr_value(args.remove(0), runtime)?;
        let items = runtime.force_value(items)?;
        let Value::List(items) = items else {
            return Err(RuntimeError::TypeError {
                context: "gtk4.eachItems".to_string(),
                expected: "List".to_string(),
                got: crate::runtime::format_value(&items),
            });
        };
        let mut out = Vec::with_capacity(items.len());
        for item in items.iter() {
            out.push(runtime.apply(template.clone(), item.clone())?);
        }
        Ok(Value::List(Arc::new(out)))
    })
}

fn serialize_signal_builtin() -> Value {
    builtin("gtk4.serializeSignal", 1, |mut args, _| {
        let value = args.remove(0);
        Ok(Value::Text(match value {
            Value::Text(text) => text,
            Value::Int(value) => value.to_string(),
            Value::Bool(value) => value.to_string(),
            Value::Float(value) => value.to_string(),
            Value::Constructor { name, args } if args.is_empty() => name,
            Value::Constructor { name, args } => {
                let values = args
                    .iter()
                    .map(crate::runtime::format_value)
                    .collect::<Vec<_>>();
                format!("{}({})", name, values.join(","))
            }
            _ => String::new(),
        }))
    })
}

fn collect_auto_bindings_into(
    node: &Value,
    named_handlers: &mut HashMap<(String, String), String>,
    unique_handlers_by_signal: &mut HashMap<String, Option<String>>,
) -> Result<(), RuntimeError> {
    let Value::Constructor { name, args } = node else {
        return Ok(());
    };
    match (name.as_str(), args.as_slice()) {
        ("GtkTextNode", [_]) => Ok(()),
        ("GtkElement", [_, attrs, children]) => {
            let Value::List(attrs) = attrs else {
                return Err(RuntimeError::Message(
                    "gtk4.autoBindingsSet: GtkElement attrs must be a List".to_string(),
                ));
            };
            let Value::List(children) = children else {
                return Err(RuntimeError::Message(
                    "gtk4.autoBindingsSet: GtkElement children must be a List".to_string(),
                ));
            };

            let mut widget_name = String::new();
            let mut signal_handlers = Vec::new();
            for attr in attrs.iter() {
                let Value::Constructor { name, args } = attr else {
                    continue;
                };
                if name != "GtkAttribute" || args.len() != 2 {
                    continue;
                }
                let Some(key) = decode_text(&args[0]) else {
                    continue;
                };
                let Some(value) = decode_text(&args[1]) else {
                    continue;
                };
                if key == "id" {
                    widget_name = value;
                } else if let Some(signal_name) = key.strip_prefix("signal:") {
                    signal_handlers.push((signal_name.to_string(), value));
                }
            }

            for (signal_name, handler) in signal_handlers {
                if !widget_name.is_empty() {
                    named_handlers.insert(
                        (widget_name.clone(), signal_name.clone()),
                        handler.clone(),
                    );
                }
                unique_handlers_by_signal
                    .entry(signal_name)
                    .and_modify(|existing| match existing {
                        Some(existing_handler) if existing_handler == &handler => {}
                        _ => *existing = None,
                    })
                    .or_insert_with(|| Some(handler));
            }

            for child in children.iter() {
                collect_auto_bindings_into(child, named_handlers, unique_handlers_by_signal)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn auto_bindings_set_builtin() -> Value {
    builtin("gtk4.autoBindingsSet", 1, |mut args, _| {
        let node = args.remove(0);
        Ok(Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new(move |runtime| {
                let node = runtime.force_value(node.clone())?;
                let mut named_handlers = HashMap::new();
                let mut unique_handlers_by_signal = HashMap::new();
                collect_auto_bindings_into(
                    &node,
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
            Value::Constructor { name, args } if name == "GtkTick" && args.is_empty() => (
                "tick".to_string(),
                String::new(),
                None,
                None,
            ),
            Value::Constructor { name, args } if name == "GtkUnknownSignal" && args.len() == 5 => (
                decode_text(&args[2]).unwrap_or_default(),
                decode_text(&args[1]).unwrap_or_default(),
                decode_text(&args[3]),
                decode_text(&args[4]).filter(|text| !text.is_empty()).map(Value::Text),
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
    fields.insert("signal".to_string(), signal_builtin());
    fields.insert("computed".to_string(), computed_builtin());
    fields.insert("autoBindingsSet".to_string(), auto_bindings_set_builtin());
    fields.insert("autoToMsg".to_string(), auto_to_msg_builtin());
    fields.insert("serializeAttr".to_string(), serialize_attr_builtin());
    fields.insert("serializeSignal".to_string(), serialize_signal_builtin());
    fields.insert("eachItems".to_string(), each_items_builtin());
    for &(name, arity) in stubs {
        fields.insert(name.to_string(), gtk4_stub(name, arity));
    }
    Value::Record(Arc::new(fields))
}
