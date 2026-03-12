use crate::runtime::Value;

#[cfg(all(feature = "gtk4-libadwaita", target_os = "linux"))]
mod bridge {
    use std::collections::HashMap;
    use std::sync::atomic::AtomicBool;
    use std::sync::{mpsc, Arc, Mutex};

    use serde_json::{json, Map as JsonMap, Value as JsonValue};

    use super::super::gtk4::{resolve_gtk_node, resolve_reactive_attr_value, ResolvedGtkAttr, ResolvedGtkNode};
    use super::super::util::builtin;
    use crate::runtime::environment::RuntimeContext;
    use crate::runtime::values::{ChannelInner, ChannelRecv};
    use crate::runtime::{
        format_runtime_error, format_value, CancelToken, EffectValue, ReactiveCellKind, Runtime,
        RuntimeError, Value,
    };

    fn effect<F>(f: F) -> Value
    where
        F: Fn(&mut crate::runtime::Runtime) -> Result<Value, RuntimeError> + Send + Sync + 'static,
    {
        Value::Effect(Arc::new(EffectValue::Thunk { func: Arc::new(f) }))
    }

    fn invalid(name: &str) -> RuntimeError {
        RuntimeError::Message(name.to_string())
    }

    fn gtk4_err_to_runtime(e: aivi_gtk4::Gtk4Error) -> RuntimeError {
        RuntimeError::Error(Value::Text(e.message))
    }

    fn ui_debug_value_type_name(value: &Value) -> &'static str {
        match value {
            Value::Unit => "Unit",
            Value::Bool(_) => "Bool",
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::Text(_) => "Text",
            Value::DateTime(_) => "DateTime",
            Value::Bytes(_) => "Bytes",
            Value::Regex(_) => "Regex",
            Value::BigInt(_) => "BigInt",
            Value::Rational(_) => "Rational",
            Value::Decimal(_) => "Decimal",
            Value::Map(_) => "Map",
            Value::Set(_) => "Set",
            Value::Queue(_) => "Queue",
            Value::Deque(_) => "Deque",
            Value::Heap(_) => "Heap",
            Value::List(_) => "List",
            Value::Tuple(_) => "Tuple",
            Value::Record(_) => "Record",
            Value::Constructor { .. } => "Constructor",
            Value::Builtin(_) | Value::MultiClause(_) => "Function",
            Value::Effect(_) => "Effect",
            Value::Source(_) => "Source",
            Value::Resource(_) => "Resource",
            Value::Thunk(_) => "Thunk",
            Value::Signal(_) => "Signal",
            Value::ChannelSend(_) => "ChannelSend",
            Value::ChannelRecv(_) => "ChannelRecv",
            Value::FileHandle(_) => "FileHandle",
            Value::Listener(_) => "Listener",
            Value::Connection(_) => "Connection",
            Value::Stream(_) => "Stream",
            Value::HttpServer(_) => "HttpServer",
            Value::WebSocket(_) => "WebSocket",
            Value::ImapSession(_) => "ImapSession",
            Value::DbConnection(_) => "DbConnection",
        }
    }

    fn ui_debug_value_json(value: &Value) -> JsonValue {
        let snapshot = crate::runtime::snapshot::value_to_snapshot_json(value).ok();
        json!({
            "type": ui_debug_value_type_name(value),
            "display": format_value(value),
            "snapshot": snapshot,
            "opaque": snapshot.is_none(),
        })
    }

    fn ui_debug_signal_kind_name(kind: &ReactiveCellKind) -> &'static str {
        match kind {
            ReactiveCellKind::Source => "source",
            ReactiveCellKind::Derived { .. } => "derived",
            ReactiveCellKind::DerivedTuple { .. } => "derivedTuple",
        }
    }

    fn ui_debug_signal_dependencies(kind: &ReactiveCellKind) -> Vec<usize> {
        match kind {
            ReactiveCellKind::Source => Vec::new(),
            ReactiveCellKind::Derived { dependencies, .. }
            | ReactiveCellKind::DerivedTuple { dependencies, .. } => dependencies.clone(),
        }
    }

    fn ui_debug_signal_compute_json(kind: &ReactiveCellKind) -> Option<JsonValue> {
        match kind {
            ReactiveCellKind::Source => None,
            ReactiveCellKind::Derived { compute, .. }
            | ReactiveCellKind::DerivedTuple { compute, .. } => Some(ui_debug_value_json(compute)),
        }
    }

    fn ui_debug_batch_state_json(
        graph: &parking_lot::MutexGuard<'_, crate::runtime::ReactiveGraphState>,
    ) -> JsonValue {
        let mut pending = graph
            .pending_notifications
            .iter()
            .copied()
            .collect::<Vec<_>>();
        pending.sort_unstable();
        json!({
            "depth": graph.batch_depth,
            "flushing": graph.flushing,
            "deferredFlush": graph.deferred_flush,
            "flushThreadBound": graph.flush_thread.is_some(),
            "pendingNotificationIds": pending,
        })
    }

    fn ui_debug_signal_summary_json(
        ctx: &RuntimeContext,
        signal_id: usize,
        graph: &parking_lot::MutexGuard<'_, crate::runtime::ReactiveGraphState>,
        include_watchers: bool,
    ) -> Result<JsonValue, aivi_gtk4::Gtk4Error> {
        let signal = graph.signals.get(&signal_id).ok_or_else(|| {
            aivi_gtk4::Gtk4Error::new(format!("gtk ui debug unknown signal id {signal_id}"))
        })?;
        let mut dependencies = ui_debug_signal_dependencies(&signal.kind);
        dependencies.sort_unstable();
        let mut dependents = signal.dependents.iter().copied().collect::<Vec<_>>();
        dependents.sort_unstable();
        let mut watcher_ids = graph
            .watchers_by_signal
            .get(&signal_id)
            .map(|ids| ids.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        watcher_ids.sort_unstable();

        let watchers = if include_watchers {
            watcher_ids
                .iter()
                .filter_map(|watcher_id| {
                    graph.watchers.get(watcher_id).map(|watcher| {
                        let mut widget_ids = ctx.gtk_binding_widgets_for_watcher(*watcher_id);
                        widget_ids.sort_unstable();
                        let watcher_kind = if widget_ids.is_empty() {
                            "runtime"
                        } else {
                            "gtkBinding"
                        };
                        json!({
                            "id": watcher_id,
                            "signalId": watcher.signal_id,
                            "active": true,
                            "lastRevision": watcher.last_revision,
                            "boundWidgetIds": widget_ids,
                            "kind": watcher_kind,
                            "callback": ui_debug_value_json(&watcher.callback),
                        })
                    })
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let mut downstream_widget_ids = watcher_ids
            .iter()
            .flat_map(|watcher_id| ctx.gtk_binding_widgets_for_watcher(*watcher_id))
            .collect::<Vec<_>>();
        downstream_widget_ids.sort_unstable();
        downstream_widget_ids.dedup();

        Ok(json!({
            "id": signal_id,
            "kind": ui_debug_signal_kind_name(&signal.kind),
            "value": ui_debug_value_json(&signal.value),
            "revision": signal.revision,
            "lastChangeSeq": signal.last_change_seq,
            "lastChangeTimestampMs": signal.last_change_timestamp_ms,
            "dirty": signal.dirty,
            "dependencies": dependencies,
            "dependents": dependents,
            "downstreamWidgetIds": downstream_widget_ids,
            "watcherCount": watcher_ids.len(),
            "watcherIds": watcher_ids,
            "watchers": if include_watchers { JsonValue::Array(watchers) } else { JsonValue::Null },
            "compute": ui_debug_signal_compute_json(&signal.kind),
        }))
    }

    pub(super) fn ui_debug_list_signals_json(
        ctx: &RuntimeContext,
    ) -> Result<JsonValue, aivi_gtk4::Gtk4Error> {
        let graph = ctx.reactive_graph.lock();
        let mut signal_ids = graph.signals.keys().copied().collect::<Vec<_>>();
        signal_ids.sort_unstable();
        let mut signals = Vec::with_capacity(signal_ids.len());
        for signal_id in signal_ids {
            signals.push(ui_debug_signal_summary_json(ctx, signal_id, &graph, false)?);
        }
        Ok(json!({
            "protocol": "aivi.gtk.debug.v1",
            "signalCount": graph.signals.len(),
            "watcherCount": graph.watchers.len(),
            "batch": ui_debug_batch_state_json(&graph),
            "signals": signals,
        }))
    }

    fn ui_debug_signal_id_param(params: &JsonMap<String, JsonValue>) -> Result<usize, aivi_gtk4::Gtk4Error> {
        let signal_id = params
            .get("signalId")
            .and_then(JsonValue::as_u64)
            .or_else(|| {
                params
                    .get("signalId")
                    .and_then(JsonValue::as_i64)
                    .and_then(|value| (value >= 0).then_some(value as u64))
            })
            .ok_or_else(|| {
                aivi_gtk4::Gtk4Error::new("gtk ui debug inspectSignal requires signalId (integer)")
            })?;
        usize::try_from(signal_id).map_err(|_| {
            aivi_gtk4::Gtk4Error::new(format!("gtk ui debug signal id {signal_id} is too large"))
        })
    }

    pub(super) fn ui_debug_inspect_signal_json(
        ctx: &RuntimeContext,
        params: &JsonMap<String, JsonValue>,
    ) -> Result<JsonValue, aivi_gtk4::Gtk4Error> {
        let signal_id = ui_debug_signal_id_param(params)?;
        let graph = ctx.reactive_graph.lock();
        let signal = ui_debug_signal_summary_json(ctx, signal_id, &graph, true)?;
        Ok(json!({
            "protocol": "aivi.gtk.debug.v1",
            "signal": signal,
            "signalCount": graph.signals.len(),
            "watcherCount": graph.watchers.len(),
            "batch": ui_debug_batch_state_json(&graph),
        }))
    }

    pub(super) fn ui_debug_explain_signal_json(
        ctx: &RuntimeContext,
        params: &JsonMap<String, JsonValue>,
    ) -> Result<JsonValue, aivi_gtk4::Gtk4Error> {
        let signal_id = ui_debug_signal_id_param(params)?;
        let graph = ctx.reactive_graph.lock();
        let signal = ui_debug_signal_summary_json(ctx, signal_id, &graph, true)?;
        Ok(json!({
            "protocol": "aivi.gtk.debug.v1",
            "signal": signal,
            "explanation": {
                "summary": format!("signal {signal_id} has revision {}, {} watcher(s), and {} downstream GTK widget(s)",
                    signal.get("revision").and_then(JsonValue::as_u64).unwrap_or(0),
                    signal.get("watcherCount").and_then(JsonValue::as_u64).unwrap_or(0),
                    signal.get("downstreamWidgetIds").and_then(JsonValue::as_array).map(|items| items.len()).unwrap_or(0),
                ),
            },
            "signalCount": graph.signals.len(),
            "watcherCount": graph.watchers.len(),
            "batch": ui_debug_batch_state_json(&graph),
        }))
    }

    fn install_ui_debug_request_handler(ctx: Arc<RuntimeContext>) {
        let handler: Arc<aivi_gtk4::UiDebugRequestHandler> = Arc::new(move |method, params| {
            match method {
                "listSignals" => Some(ui_debug_list_signals_json(ctx.as_ref())),
                "inspectSignal" => Some(ui_debug_inspect_signal_json(ctx.as_ref(), params)),
                "explainSignal" => Some(ui_debug_explain_signal_json(ctx.as_ref(), params)),
                _ => None,
            }
        });
        aivi_gtk4::set_ui_debug_request_handler(Some(handler));
    }

    fn install_main_loop_tick_handler(ctx: Arc<RuntimeContext>) {
        let handler: Arc<aivi_gtk4::MainLoopTickHandler> = Arc::new(move || {
            let mut runtime = Runtime::new(ctx.clone(), CancelToken::root());
            runtime
                .reactive_flush_deferred()
                .map_err(|err| aivi_gtk4::Gtk4Error::new(format_runtime_error(err)))
        });
        aivi_gtk4::set_main_loop_tick_handler(Some(handler));
    }

    fn install_gtk_runtime_hooks(ctx: Arc<RuntimeContext>) {
        install_ui_debug_request_handler(ctx.clone());
        install_main_loop_tick_handler(ctx);
    }

    fn serialize_signal_value(val: &Value) -> String {
        match val {
            Value::Text(t) => t.clone(),
            Value::Int(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Constructor { name, args } if args.is_empty() => name.clone(),
            Value::Constructor { name, args } => {
                let arg_strs: Vec<String> = args.iter().map(serialize_signal_value).collect();
                format!("{}({})", name, arg_strs.join(","))
            }
            _ => String::new(),
        }
    }

    fn value_to_static_text(value: &Value, context: &str) -> Result<String, RuntimeError> {
        match value {
            Value::Text(text) => Ok(text.clone()),
            Value::Int(value) => Ok(value.to_string()),
            Value::Float(value) => Ok(value.to_string()),
            Value::Bool(value) => Ok(value.to_string()),
            Value::DateTime(value) => Ok(value.clone()),
            Value::BigInt(value) => Ok(value.to_string()),
            Value::Rational(value) => Ok(value.to_string()),
            Value::Decimal(value) => Ok(value.to_string()),
            Value::Constructor { name, args } if args.is_empty() => Ok(name.clone()),
            Value::Constructor { name, args } => {
                let arg_strs: Vec<String> = args.iter().map(serialize_signal_value).collect();
                Ok(format!("{}({})", name, arg_strs.join(",")))
            }
            other => Err(RuntimeError::TypeError {
                context: context.to_string(),
                expected: "Text-compatible value".to_string(),
                got: crate::runtime::format_value(other),
            }),
        }
    }

    fn resolve_binding_text(
        value: Value,
        runtime: &mut crate::runtime::Runtime,
        context: &str,
    ) -> Result<String, RuntimeError> {
        let value = resolve_reactive_attr_value(value, runtime)?;
        let value = runtime.force_value(value)?;
        value_to_static_text(&value, context)
    }

    fn static_handler_name(value: &Value) -> Option<String> {
        let text = serialize_signal_value(value);
        (!text.is_empty()).then_some(text)
    }

    fn event_handle_run_effect(value: &Value) -> Option<Value> {
        let Value::Record(fields) = value else {
            return None;
        };
        let run = fields.get("run")?.clone();
        matches!(run, Value::Effect(_)).then_some(run)
    }

    #[derive(Clone)]
    enum BindingTextPart {
        Static(String),
        Dynamic(Value),
    }

    #[derive(Clone)]
    struct LivePropertyBinding {
        target: String,
        class_name: String,
        property: String,
        parts: Vec<BindingTextPart>,
        signal: Value,
    }

    #[derive(Clone)]
    struct LiveStructuralBinding {
        target: String,
        node: ResolvedGtkNode,
        signals: Vec<Value>,
    }

    fn alloc_binding_target(next_binding_id: &mut usize) -> String {
        let target = format!("__aivi_binding_{}", *next_binding_id);
        *next_binding_id = next_binding_id.saturating_add(1);
        target
    }

    fn binding_signal_source(
        value: &Value,
        runtime: &mut crate::runtime::Runtime,
    ) -> Result<Option<Value>, RuntimeError> {
        let value = runtime.force_value(value.clone())?;
        Ok(matches!(value, Value::Signal(_)).then_some(value))
    }

    fn resolve_binding_parts_text(
        parts: &[BindingTextPart],
        runtime: &mut crate::runtime::Runtime,
        context: &str,
    ) -> Result<String, RuntimeError> {
        let mut out = String::new();
        for part in parts {
            match part {
                BindingTextPart::Static(text) => out.push_str(text),
                BindingTextPart::Dynamic(value) => {
                    out.push_str(&resolve_binding_text(value.clone(), runtime, context)?);
                }
            }
        }
        Ok(out)
    }

    fn find_static_attr<'a>(attrs: &'a [ResolvedGtkAttr], name: &str) -> Option<&'a str> {
        attrs.iter().find_map(|attr| match attr {
            ResolvedGtkAttr::StaticAttr { name: attr_name, value } if attr_name == name => {
                Some(value.as_str())
            }
            _ => None,
        })
    }

    fn extract_property_child_parts(
        node: &ResolvedGtkNode,
    ) -> Option<(String, Vec<BindingTextPart>)> {
        let ResolvedGtkNode::Element {
            tag,
            attrs,
            children,
        } = node
        else {
            return None;
        };
        if tag != "property" {
            return None;
        }
        let property = find_static_attr(attrs, "name")?.to_string();
        let mut parts = Vec::new();
        for child in children {
            match child {
                ResolvedGtkNode::Text(text) => parts.push(BindingTextPart::Static(text.clone())),
                ResolvedGtkNode::DynamicText(value) => {
                    parts.push(BindingTextPart::Dynamic(value.clone()))
                }
                _ => return None,
            }
        }
        Some((property, parts))
    }

    fn push_live_property_bindings(
        target: &str,
        class_name: &str,
        property: &str,
        parts: &[BindingTextPart],
        runtime: &mut crate::runtime::Runtime,
        bindings: &mut Vec<LivePropertyBinding>,
    ) -> Result<(), RuntimeError> {
        for part in parts {
            let BindingTextPart::Dynamic(value) = part else {
                continue;
            };
            if let Some(signal) = binding_signal_source(value, runtime)? {
                bindings.push(LivePropertyBinding {
                    target: target.to_string(),
                    class_name: class_name.to_string(),
                    property: property.to_string(),
                    parts: parts.to_vec(),
                    signal,
                });
            }
        }
        Ok(())
    }

    fn parse_bool_text(text: &str) -> Option<bool> {
        match text.trim().to_ascii_lowercase().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }

    fn parse_align_text(text: &str) -> Option<i32> {
        match text.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "fill" => Some(0),
            "start" => Some(1),
            "end" => Some(2),
            "center" | "middle" => Some(3),
            other => other.parse::<i32>().ok(),
        }
    }

    fn apply_live_property(
        widget_id: i64,
        class_name: &str,
        property: &str,
        value: &str,
    ) -> Result<(), RuntimeError> {
        if !aivi_gtk4::widget_exists(widget_id).map_err(gtk4_err_to_runtime)? {
            return Ok(());
        }
        match property {
            "open" if class_name.starts_with("Adw") && class_name.ends_with("Dialog") => {
                let open = parse_bool_text(value).ok_or_else(|| RuntimeError::TypeError {
                    context: format!("gtk4 live binding {class_name}.{property}"),
                    expected: "Bool".to_string(),
                    got: value.to_string(),
                })?;
                aivi_gtk4::dialog_set_open(widget_id, open).map_err(gtk4_err_to_runtime)
            }
            "visible" => {
                let visible = parse_bool_text(value).ok_or_else(|| RuntimeError::TypeError {
                    context: format!("gtk4 live binding {class_name}.{property}"),
                    expected: "Bool".to_string(),
                    got: value.to_string(),
                })?;
                if visible {
                    aivi_gtk4::widget_show(widget_id).map_err(gtk4_err_to_runtime)
                } else {
                    aivi_gtk4::widget_hide(widget_id).map_err(gtk4_err_to_runtime)
                }
            }
            "sensitive" => {
                let flag = parse_bool_text(value).ok_or_else(|| RuntimeError::TypeError {
                    context: format!("gtk4 live binding {class_name}.{property}"),
                    expected: "Bool".to_string(),
                    got: value.to_string(),
                })?;
                aivi_gtk4::widget_set_bool_property(widget_id, property, flag)
                    .map_err(gtk4_err_to_runtime)
            }
            "collapsed" | "show-sidebar" if class_name == "AdwOverlaySplitView" => {
                let flag = parse_bool_text(value).ok_or_else(|| RuntimeError::TypeError {
                    context: format!("gtk4 live binding {class_name}.{property}"),
                    expected: "Bool".to_string(),
                    got: value.to_string(),
                })?;
                aivi_gtk4::widget_set_bool_property(widget_id, property, flag)
                    .map_err(gtk4_err_to_runtime)
            }
            "hexpand" => {
                let flag = parse_bool_text(value).ok_or_else(|| RuntimeError::TypeError {
                    context: format!("gtk4 live binding {class_name}.{property}"),
                    expected: "Bool".to_string(),
                    got: value.to_string(),
                })?;
                aivi_gtk4::widget_set_hexpand(widget_id, flag).map_err(gtk4_err_to_runtime)
            }
            "vexpand" => {
                let flag = parse_bool_text(value).ok_or_else(|| RuntimeError::TypeError {
                    context: format!("gtk4 live binding {class_name}.{property}"),
                    expected: "Bool".to_string(),
                    got: value.to_string(),
                })?;
                aivi_gtk4::widget_set_vexpand(widget_id, flag).map_err(gtk4_err_to_runtime)
            }
            "halign" => {
                let align = parse_align_text(value).ok_or_else(|| RuntimeError::TypeError {
                    context: format!("gtk4 live binding {class_name}.{property}"),
                    expected: "GtkAlign".to_string(),
                    got: value.to_string(),
                })?;
                aivi_gtk4::widget_set_halign(widget_id, align).map_err(gtk4_err_to_runtime)
            }
            "valign" => {
                let align = parse_align_text(value).ok_or_else(|| RuntimeError::TypeError {
                    context: format!("gtk4 live binding {class_name}.{property}"),
                    expected: "GtkAlign".to_string(),
                    got: value.to_string(),
                })?;
                aivi_gtk4::widget_set_valign(widget_id, align).map_err(gtk4_err_to_runtime)
            }
            "margin-start" => value
                .parse::<i32>()
                .map_err(|_| RuntimeError::TypeError {
                    context: format!("gtk4 live binding {class_name}.{property}"),
                    expected: "Int".to_string(),
                    got: value.to_string(),
                })
                .and_then(|margin| {
                    aivi_gtk4::widget_set_margin_start(widget_id, margin)
                        .map_err(gtk4_err_to_runtime)
                }),
            "margin-end" => value
                .parse::<i32>()
                .map_err(|_| RuntimeError::TypeError {
                    context: format!("gtk4 live binding {class_name}.{property}"),
                    expected: "Int".to_string(),
                    got: value.to_string(),
                })
                .and_then(|margin| {
                    aivi_gtk4::widget_set_margin_end(widget_id, margin).map_err(gtk4_err_to_runtime)
                }),
            "margin-top" => value
                .parse::<i32>()
                .map_err(|_| RuntimeError::TypeError {
                    context: format!("gtk4 live binding {class_name}.{property}"),
                    expected: "Int".to_string(),
                    got: value.to_string(),
                })
                .and_then(|margin| {
                    aivi_gtk4::widget_set_margin_top(widget_id, margin).map_err(gtk4_err_to_runtime)
                }),
            "margin-bottom" => value
                .parse::<i32>()
                .map_err(|_| RuntimeError::TypeError {
                    context: format!("gtk4 live binding {class_name}.{property}"),
                    expected: "Int".to_string(),
                    got: value.to_string(),
                })
                .and_then(|margin| {
                    aivi_gtk4::widget_set_margin_bottom(widget_id, margin)
                        .map_err(gtk4_err_to_runtime)
                }),
            "tooltip-text" => {
                aivi_gtk4::widget_set_tooltip_text(widget_id, value).map_err(gtk4_err_to_runtime)
            }
            "opacity" => value
                .parse::<f64>()
                .map_err(|_| RuntimeError::TypeError {
                    context: format!("gtk4 live binding {class_name}.{property}"),
                    expected: "Float".to_string(),
                    got: value.to_string(),
                })
                .and_then(|opacity| {
                    aivi_gtk4::widget_set_opacity(widget_id, opacity).map_err(gtk4_err_to_runtime)
                }),
            "label" | "text" if class_name == "GtkLabel" => {
                aivi_gtk4::label_set_text(widget_id, value).map_err(gtk4_err_to_runtime)
            }
            "label" if class_name == "GtkButton" || class_name == "GtkToggleButton" => {
                aivi_gtk4::button_set_label(widget_id, value).map_err(gtk4_err_to_runtime)
            }
            "text"
                if matches!(
                    class_name,
                    "GtkEntry" | "GtkPasswordEntry" | "GtkSearchEntry"
                ) =>
            {
                aivi_gtk4::entry_set_text(widget_id, value).map_err(gtk4_err_to_runtime)
            }
            "title" if matches!(class_name, "GtkWindow" | "AdwWindow" | "AdwApplicationWindow") => {
                aivi_gtk4::window_set_title(widget_id, value).map_err(gtk4_err_to_runtime)
            }
            _ => Ok(()),
        }
    }

    fn install_live_bindings(
        runtime: &mut crate::runtime::Runtime,
        widgets: &HashMap<String, i64>,
        bindings: Vec<LivePropertyBinding>,
    ) -> Result<(), RuntimeError> {
        for binding in bindings {
            let Some(&widget_id) = widgets.get(&binding.target) else {
                continue;
            };
            let class_name = binding.class_name.clone();
            let property = binding.property.clone();
            let parts = binding.parts.clone();
            let callback = builtin("gtk4.liveBinding", 1, move |_args, runtime| {
                let value = resolve_binding_parts_text(
                    &parts,
                    runtime,
                    "gtk4 live property binding",
                )?;
                apply_live_property(widget_id, &class_name, &property, &value)?;
                Ok(Value::Unit)
            });
            let (watcher_id, _) = runtime.reactive_watch_signal_main_thread(binding.signal, callback)?;
            runtime.ctx.push_gtk_binding_watcher(widget_id, watcher_id);
        }
        Ok(())
    }

    fn cleanup_binding_scope(
        runtime: &mut crate::runtime::Runtime,
        widget_id: i64,
    ) -> Result<(), RuntimeError> {
        let scoped_widget_ids = if aivi_gtk4::widget_exists(widget_id).map_err(gtk4_err_to_runtime)? {
            aivi_gtk4::binding_widget_ids(widget_id).map_err(gtk4_err_to_runtime)?
        } else {
            vec![widget_id]
        };
        for scoped_widget_id in scoped_widget_ids {
            for watcher_id in runtime.ctx.take_gtk_binding_watchers(scoped_widget_id) {
                runtime.reactive_dispose_watcher(watcher_id);
            }
        }
        Ok(())
    }

    fn cleanup_binding_widgets(
        runtime: &mut crate::runtime::Runtime,
        widget_ids: &[i64],
    ) -> Result<(), RuntimeError> {
        for &widget_id in widget_ids {
            for watcher_id in runtime.ctx.take_gtk_binding_watchers(widget_id) {
                runtime.reactive_dispose_watcher(watcher_id);
            }
        }
        Ok(())
    }

    fn register_dialog_root_cleanup(
        runtime: &mut crate::runtime::Runtime,
        root_id: i64,
        root_class_name: &str,
        binding_widgets: &HashMap<String, i64>,
    ) -> Result<(), RuntimeError> {
        if !(root_class_name.starts_with("Adw") && root_class_name.ends_with("Dialog")) {
            return Ok(());
        }
        if aivi_gtk4::dialog_root_is_persistent(root_id).map_err(gtk4_err_to_runtime)? {
            return Ok(());
        }
        let mut widget_ids = binding_widgets.values().copied().collect::<Vec<_>>();
        widget_ids.sort_unstable();
        widget_ids.dedup();
        ensure_runtime_handler_dispatcher(runtime.ctx.clone());
        // Share the token with the handler so it can unregister itself after
        // firing (dialog close handlers are one-shot).
        let token_holder: Arc<std::sync::OnceLock<String>> = Arc::new(std::sync::OnceLock::new());
        let token_for_handler = token_holder.clone();
        let handler = builtin("gtk4.dialogRootCleanup", 1, move |_args, runtime| {
            cleanup_binding_widgets(runtime, &widget_ids)?;
            if let Some(token) = token_for_handler.get() {
                runtime.ctx.unregister_gtk_runtime_handler(token);
            }
            Ok(Value::Unit)
        });
        let token = runtime.ctx.register_gtk_runtime_handler(handler);
        let _ = token_holder.set(token.clone());
        aivi_gtk4::signal_bind_cleanup_root(&token, root_id).map_err(gtk4_err_to_runtime)?;
        aivi_gtk4::dialog_root_on_closed(root_id, &token).map_err(gtk4_err_to_runtime)?;
        Ok(())
    }

    fn structural_binding_signals(
        children: &[ResolvedGtkNode],
        runtime: &mut crate::runtime::Runtime,
    ) -> Result<Vec<Value>, RuntimeError> {
        let mut signals = Vec::new();
        for child in children {
            match child {
                ResolvedGtkNode::Each { items, .. } => {
                    if let Some(signal) = binding_signal_source(items, runtime)? {
                        signals.push(signal);
                    }
                }
                ResolvedGtkNode::Show { when, child } => {
                    if !matches!(child.as_ref(), ResolvedGtkNode::Element { .. }) {
                        if let Some(signal) = binding_signal_source(when, runtime)? {
                            signals.push(signal);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(signals)
    }

    fn install_structural_bindings(
        runtime: &mut crate::runtime::Runtime,
        widgets: &HashMap<String, i64>,
        bindings: Vec<LiveStructuralBinding>,
    ) -> Result<(), RuntimeError> {
        for binding in bindings {
            let Some(&widget_id) = widgets.get(&binding.target) else {
                continue;
            };
            let node = binding.node.clone();
            let callback = builtin("gtk4.structuralBinding", 1, move |_args, runtime| {
                if !aivi_gtk4::widget_exists(widget_id).map_err(gtk4_err_to_runtime)? {
                    return Ok(Value::Unit);
                }
                let mut next_binding_id = 1;
                let mut prop_bindings = Vec::new();
                let mut structural_bindings = Vec::new();
                let materialized = materialize_node(
                    &node,
                    runtime,
                    &mut next_binding_id,
                    &mut prop_bindings,
                    &mut structural_bindings,
                )?;
                // NOTE: This disposes ALL watchers for widget_id, including the
                // currently executing structural watcher. This is safe because
                // the callback value was already cloned out of the graph before
                // execution, and reactive_flush handles missing watchers gracefully.
                // The new watchers installed below replace the disposed ones.
                cleanup_binding_scope(runtime, widget_id)?;
                let binding_widgets =
                    aivi_gtk4::reconcile_widget(widget_id, &materialized).map_err(gtk4_err_to_runtime)?;
                install_live_bindings(runtime, &binding_widgets, prop_bindings)?;
                install_structural_bindings(runtime, &binding_widgets, structural_bindings)?;
                Ok(Value::Unit)
            });
            for signal in binding.signals {
                let (watcher_id, _) = runtime.reactive_watch_signal_main_thread(signal, callback.clone())?;
                runtime.ctx.push_gtk_binding_watcher(widget_id, watcher_id);
            }
        }
        Ok(())
    }

    pub(super) fn materialize_with_bindings(
        node: &ResolvedGtkNode,
        runtime: &mut crate::runtime::Runtime,
    ) -> Result<aivi_gtk4::BuildWithBindingsResult, RuntimeError> {
        let mut next_binding_id = 1;
        let mut bindings = Vec::new();
        let mut structural_bindings = Vec::new();
        let node = materialize_node(
            node,
            runtime,
            &mut next_binding_id,
            &mut bindings,
            &mut structural_bindings,
        )?;
        let result = aivi_gtk4::build_with_bindings(&node).map_err(gtk4_err_to_runtime)?;
        install_live_bindings(runtime, &result.binding_widgets, bindings)?;
        install_structural_bindings(runtime, &result.binding_widgets, structural_bindings)?;
        register_dialog_root_cleanup(
            runtime,
            result.root_id,
            &result.root_class_name,
            &result.binding_widgets,
        )?;
        if result.root_class_name.starts_with("Adw") && result.root_class_name.ends_with("Dialog") {
            aivi_gtk4::dialog_sync_open_state(result.root_id).map_err(gtk4_err_to_runtime)?;
        }
        Ok(result)
    }

    pub(super) fn execute_runtime_handler(
        ctx: Arc<RuntimeContext>,
        handler: Value,
        event: aivi_gtk4::SignalEvent,
    ) -> Result<(), RuntimeError> {
        let mut runtime = Runtime::new(ctx, CancelToken::root());
        if let Some(run_effect) = event_handle_run_effect(&handler) {
            runtime.run_effect_value(run_effect)?;
            if let Some(err) = runtime.jit_pending_error.take() {
                return Err(err);
            }
            return Ok(());
        }
        if let Value::Effect(_) = handler {
            runtime.run_effect_value(handler)?;
            if let Some(err) = runtime.jit_pending_error.take() {
                return Err(err);
            }
            return Ok(());
        }
        let result = runtime.apply(handler, make_signal_event_value(event))?;
        let result = runtime.force_value(result)?;
        if let Value::Effect(_) = result {
            runtime.run_effect_value(result)?;
            if let Some(err) = runtime.jit_pending_error.take() {
                return Err(err);
            }
        }
        Ok(())
    }

    fn ensure_runtime_handler_dispatcher(ctx: Arc<RuntimeContext>) {
        if !ctx.mark_gtk_runtime_dispatcher_started() {
            return;
        }
        // Create the signal stream on the current thread (the GTK main thread)
        // so the sender is registered in the correct thread-local GTK_STATE.
        let receiver = match aivi_gtk4::signal_stream() {
            Ok(receiver) => receiver,
            Err(err) => {
                eprintln!(
                    "AIVI GTK runtime handler dispatcher failed to attach: {}",
                    err.message
                );
                return;
            }
        };
        let _ = std::thread::Builder::new()
            .name("aivi-gtk-runtime-handlers".to_string())
            .spawn(move || {
                while let Ok(event) = receiver.recv() {
                    let Some(handler) = ctx.resolve_gtk_runtime_handler(&event.handler) else {
                        continue;
                    };
                    if let Err(err) = execute_runtime_handler(ctx.clone(), handler, event) {
                        eprintln!(
                            "AIVI GTK runtime handler error: {}",
                            format_runtime_error(err)
                        );
                    }
                }
            });
    }

    fn materialize_attr(
        attr: &ResolvedGtkAttr,
        runtime: &mut crate::runtime::Runtime,
        target: &str,
        class_name: &str,
        bindings: &mut Vec<LivePropertyBinding>,
    ) -> Result<(String, String), RuntimeError> {
        match attr {
            ResolvedGtkAttr::StaticAttr { name, value } => Ok((name.clone(), value.clone())),
            ResolvedGtkAttr::BoundAttr { name, value } => Ok((
                name.clone(),
                resolve_binding_text(value.clone(), runtime, "gtk4.buildFromNode bound attr")?,
            )),
            ResolvedGtkAttr::StaticProp { name, value } => {
                Ok((format!("prop:{name}"), value.clone()))
            }
            ResolvedGtkAttr::BoundProp { name, value } => {
                let parts = vec![BindingTextPart::Dynamic(value.clone())];
                push_live_property_bindings(target, class_name, name, &parts, runtime, bindings)?;
                Ok((
                    format!("prop:{name}"),
                    resolve_binding_parts_text(
                        &parts,
                        runtime,
                        "gtk4.buildFromNode bound prop",
                    )?,
                ))
            }
            ResolvedGtkAttr::EventProp { name, handler } => {
                let handler = runtime.force_value(handler.clone())?;
                let handler = if let Some(handler) = static_handler_name(&handler) {
                    handler
                } else {
                    ensure_runtime_handler_dispatcher(runtime.ctx.clone());
                    runtime.ctx.register_gtk_runtime_handler(handler)
                };
                Ok((format!("signal:{name}"), handler))
            }
            ResolvedGtkAttr::Id(value) => Ok(("id".to_string(), value.clone())),
            ResolvedGtkAttr::Ref(value) => Ok(("ref".to_string(), value.clone())),
        }
    }

    fn materialize_children(
        children: &[ResolvedGtkNode],
        runtime: &mut crate::runtime::Runtime,
        next_binding_id: &mut usize,
        bindings: &mut Vec<LivePropertyBinding>,
        structural_bindings: &mut Vec<LiveStructuralBinding>,
    ) -> Result<Vec<aivi_gtk4::GtkNode>, RuntimeError> {
        let mut out = Vec::new();
        for child in children {
            match child {
                ResolvedGtkNode::Show { when, child } => {
                    if let ResolvedGtkNode::Element {
                        tag,
                        attrs,
                        children,
                    } = child.as_ref()
                    {
                        let mut attrs = attrs.clone();
                        attrs.push(ResolvedGtkAttr::BoundProp {
                            name: "visible".to_string(),
                            value: when.clone(),
                        });
                        out.push(materialize_node(
                            &ResolvedGtkNode::Element {
                                tag: tag.clone(),
                                attrs,
                                children: children.clone(),
                            },
                            runtime,
                            next_binding_id,
                            bindings,
                            structural_bindings,
                        )?);
                    } else {
                        let when = resolve_reactive_attr_value(when.clone(), runtime)?;
                        let when = runtime.force_value(when)?;
                        match when {
                            Value::Bool(true) => {
                                out.extend(materialize_children(
                                    std::slice::from_ref(child.as_ref()),
                                    runtime,
                                    next_binding_id,
                                    bindings,
                                    structural_bindings,
                                )?);
                            }
                            Value::Bool(false) => {}
                            other => {
                                return Err(RuntimeError::TypeError {
                                    context: "gtk4.buildFromNode show binding".to_string(),
                                    expected: "Bool".to_string(),
                                    got: crate::runtime::format_value(&other),
                                })
                            }
                        }
                    }
                }
                ResolvedGtkNode::Each { items, template, _key } => {
                    let items = resolve_reactive_attr_value(items.clone(), runtime)?;
                    let items = runtime.force_value(items)?;
                    let Value::List(items) = items else {
                        return Err(RuntimeError::TypeError {
                            context: "gtk4.buildFromNode each items".to_string(),
                            expected: "List".to_string(),
                            got: crate::runtime::format_value(&items),
                        });
                    };
                    for item in items.iter() {
                        let key = if let Some(key_fn) = _key {
                            let key_value = runtime.apply(key_fn.clone(), item.clone())?;
                            let key_value = runtime.force_value(key_value)?;
                            Some(value_to_static_text(&key_value, "gtk4.buildFromNode each key")?)
                        } else {
                            None
                        };
                        let value = runtime.apply(template.clone(), item.clone())?;
                        let value = runtime.force_value(value)?;
                        let resolved = resolve_gtk_node(&value, runtime)?;
                        let mut materialized = materialize_children(
                            std::slice::from_ref(&resolved),
                            runtime,
                            next_binding_id,
                            bindings,
                            structural_bindings,
                        )?;
                        if let Some(key) = key.as_deref() {
                            for child in &mut materialized {
                                if let aivi_gtk4::GtkNode::Element { attrs, .. } = child {
                                    attrs.push(("aivi-key".to_string(), key.to_string()));
                                }
                            }
                        }
                        out.extend(materialized);
                    }
                }
                other => out.push(materialize_node(
                    other,
                    runtime,
                    next_binding_id,
                    bindings,
                    structural_bindings,
                )?),
            }
        }
        Ok(out)
    }

    fn materialize_node(
        node: &ResolvedGtkNode,
        runtime: &mut crate::runtime::Runtime,
        next_binding_id: &mut usize,
        bindings: &mut Vec<LivePropertyBinding>,
        structural_bindings: &mut Vec<LiveStructuralBinding>,
    ) -> Result<aivi_gtk4::GtkNode, RuntimeError> {
        match node {
            ResolvedGtkNode::Text(text) => Ok(aivi_gtk4::GtkNode::Text(text.clone())),
            ResolvedGtkNode::DynamicText(value) => Ok(aivi_gtk4::GtkNode::Text(
                resolve_binding_text(value.clone(), runtime, "gtk4.buildFromNode bound text")?,
            )),
            ResolvedGtkNode::Element {
                tag,
                attrs,
                children,
            } => {
                let class_name = find_static_attr(attrs, "class").unwrap_or(tag.as_str());
                let mut local_bindings = Vec::new();
                let structural_signals = structural_binding_signals(children, runtime)?;
                let target = alloc_binding_target(next_binding_id);
                let mut materialized_attrs = attrs
                    .iter()
                    .map(|attr| materialize_attr(attr, runtime, &target, class_name, &mut local_bindings))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut materialized_children = Vec::new();
                for child in children {
                    if let Some((property, parts)) = extract_property_child_parts(child) {
                        push_live_property_bindings(
                            &target,
                            class_name,
                            &property,
                            &parts,
                            runtime,
                            &mut local_bindings,
                        )?;
                        let value = resolve_binding_parts_text(
                            &parts,
                            runtime,
                            "gtk4.buildFromNode property text",
                        )?;
                        materialized_attrs.push((format!("prop:{property}"), value));
                        continue;
                    }
                    materialized_children.extend(materialize_children(
                        std::slice::from_ref(child),
                        runtime,
                        next_binding_id,
                        bindings,
                        structural_bindings,
                    )?);
                }
                let needs_target = !local_bindings.is_empty() || !structural_signals.is_empty();
                if needs_target {
                    materialized_attrs.push(("aivi-binding-id".to_string(), target.clone()));
                }
                bindings.extend(local_bindings);
                if !structural_signals.is_empty() {
                    structural_bindings.push(LiveStructuralBinding {
                        target,
                        node: node.clone(),
                        signals: structural_signals,
                    });
                }
                Ok(aivi_gtk4::GtkNode::Element {
                    tag: tag.clone(),
                    attrs: materialized_attrs,
                    children: materialized_children,
                })
            }
            ResolvedGtkNode::Show { .. } | ResolvedGtkNode::Each { .. } => Err(invalid(
                "gtk4.buildFromNode root must be a concrete GTK element or text node",
            )),
        }
    }

    pub(super) fn make_signal_event_value(event: aivi_gtk4::SignalEvent) -> Value {
        let wid = Value::Int(event.widget_id);
        let name = Value::Text(event.widget_name);
        match event.signal.as_str() {
            "clicked" => {
                if event.handler.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    let (cname, arg) = parse_constructor_handler(&event.handler);
                    Value::Constructor {
                        name: "GtkUnknownSignal".to_string(),
                        args: vec![wid, name, Value::Text(cname), Value::Text(arg), Value::Text(String::new())],
                    }
                } else {
                    Value::Constructor { name: "GtkClicked".to_string(), args: vec![wid, name] }
                }
            }
            "changed" => Value::Constructor {
                name: "GtkInputChanged".to_string(),
                args: vec![wid, name, Value::Text(event.payload)],
            },
            "activate" => Value::Constructor {
                name: "GtkActivated".to_string(),
                args: vec![wid, name],
            },
            "toggled" => {
                let active = event.payload == "true";
                Value::Constructor { name: "GtkToggled".to_string(), args: vec![wid, name, Value::Bool(active)] }
            }
            "value-changed" => {
                let val = event.payload.parse::<f64>().unwrap_or(0.0);
                Value::Constructor { name: "GtkValueChanged".to_string(), args: vec![wid, name, Value::Float(val)] }
            }
            "key-pressed" => {
                let mut parts = event.payload.splitn(2, '\n');
                let key = parts.next().unwrap_or_default().to_string();
                let detail = parts.next().unwrap_or_default().to_string();
                Value::Constructor {
                    name: "GtkKeyPressed".to_string(),
                    args: vec![wid, name, Value::Text(key), Value::Text(detail)],
                }
            }
            "focus-enter" => Value::Constructor { name: "GtkFocusIn".to_string(), args: vec![wid, name] },
            "focus-leave" => Value::Constructor { name: "GtkFocusOut".to_string(), args: vec![wid, name] },
            "close-request" => Value::Constructor { name: "GtkWindowClosed".to_string(), args: vec![wid, name] },
            "tick" => Value::Constructor { name: "GtkTick".to_string(), args: vec![] },
            signal if signal.starts_with("notify::") => {
                let (cname, _) = parse_constructor_handler(&event.handler);
                Value::Constructor {
                    name: "GtkUnknownSignal".to_string(),
                    args: vec![wid, name, Value::Text(cname), Value::Text(event.payload), Value::Text(String::new())],
                }
            }
            _ => Value::Constructor {
                name: "GtkUnknownSignal".to_string(),
                args: vec![wid, name, Value::Text(event.signal), Value::Text(event.handler), Value::Text(event.payload)],
            },
        }
    }

    fn parse_constructor_handler(handler: &str) -> (String, String) {
        if let Some(paren_pos) = handler.find('(') {
            let name = handler[..paren_pos].to_string();
            let arg = handler[paren_pos + 1..handler.len().saturating_sub(1)].to_string();
            (name, arg)
        } else {
            (handler.to_string(), String::new())
        }
    }

    pub(in super::super) fn build_from_mock(mut fields: HashMap<String, Value>) -> HashMap<String, Value> {
        // ── init ──
        fields.insert("init".to_string(), builtin("gtk4.init", 1, |mut args, _| {
            match args.remove(0) { Value::Unit => {} _ => return Err(invalid("gtk4.init expects Unit")) }
            Ok(effect(move |runtime| {
                install_gtk_runtime_hooks(runtime.ctx.clone());
                aivi_gtk4::init().map_err(gtk4_err_to_runtime)?;
                Ok(Value::Unit)
            }))
        }));

        // ── appNew ──
        fields.insert("appNew".to_string(), builtin("gtk4.appNew", 1, |mut args, _| {
            let id = match args.remove(0) { Value::Text(v) => v, _ => return Err(invalid("gtk4.appNew expects Text")) };
            Ok(effect(move |runtime| {
                install_gtk_runtime_hooks(runtime.ctx.clone());
                let r = aivi_gtk4::app_new(&id).map_err(gtk4_err_to_runtime)?;
                Ok(Value::Int(r))
            }))
        }));

        // ── appRun ──
        fields.insert("appRun".to_string(), builtin("gtk4.appRun", 1, |mut args, _| {
            let app_id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("gtk4.appRun expects Int")) };
            Ok(effect(move |_| { aivi_gtk4::app_run(app_id).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        // ── appSetCss ──
        fields.insert("appSetCss".to_string(), builtin("gtk4.appSetCss", 2, |mut args, _| {
            let css_text = match args.remove(1) {
                Value::Text(v) => v,
                Value::Record(_) => return Ok(effect(|_| Ok(Value::Unit))),
                _ => return Err(invalid("gtk4.appSetCss expects Text or Record")),
            };
            let app_id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("gtk4.appSetCss expects Int")) };
            Ok(effect(move |_| { aivi_gtk4::app_set_css(app_id, &css_text).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        // ── windowNew ──
        fields.insert("windowNew".to_string(), builtin("gtk4.windowNew", 4, |mut args, _| {
            let height = match args.remove(3) { Value::Int(v) => v as i32, _ => return Err(invalid("gtk4.windowNew expects Int height")) };
            let width = match args.remove(2) { Value::Int(v) => v as i32, _ => return Err(invalid("gtk4.windowNew expects Int width")) };
            let title = match args.remove(1) { Value::Text(v) => v, _ => return Err(invalid("gtk4.windowNew expects Text title")) };
            let app_id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("gtk4.windowNew expects Int")) };
            Ok(effect(move |_| { let r = aivi_gtk4::window_new(app_id, &title, width, height).map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));

        macro_rules! bridge_unit_ii {
            ($name:expr, $fn:path) => {
                fields.insert($name.to_string(), builtin(concat!("gtk4.", $name), 2, |mut args, _| {
                    let b = match args.remove(1) { Value::Int(v) => v, _ => return Err(invalid(concat!("gtk4.", $name, " expects Int"))) };
                    let a = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid(concat!("gtk4.", $name, " expects Int"))) };
                    Ok(effect(move |_| { $fn(a, b).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
                }));
            };
        }

        macro_rules! bridge_unit_it {
            ($name:expr, $fn:path) => {
                fields.insert($name.to_string(), builtin(concat!("gtk4.", $name), 2, |mut args, _| {
                    let t = match args.remove(1) { Value::Text(v) => v, _ => return Err(invalid(concat!("gtk4.", $name, " expects Text"))) };
                    let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid(concat!("gtk4.", $name, " expects Int"))) };
                    Ok(effect(move |_| { $fn(id, &t).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
                }));
            };
        }

        macro_rules! bridge_bool_it {
            ($name:expr, $fn:path) => {
                fields.insert($name.to_string(), builtin(concat!("gtk4.", $name), 2, |mut args, _| {
                    let t = match args.remove(1) { Value::Text(v) => v, _ => return Err(invalid(concat!("gtk4.", $name, " expects Text"))) };
                    let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid(concat!("gtk4.", $name, " expects Int"))) };
                    Ok(effect(move |_| { let r = $fn(id, &t).map_err(gtk4_err_to_runtime)?; Ok(Value::Bool(r)) }))
                }));
            };
        }

        macro_rules! bridge_unit_ib {
            ($name:expr, $fn:path) => {
                fields.insert($name.to_string(), builtin(concat!("gtk4.", $name), 2, |mut args, _| {
                    let b = match args.remove(1) { Value::Bool(v) => v, _ => return Err(invalid(concat!("gtk4.", $name, " expects Bool"))) };
                    let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid(concat!("gtk4.", $name, " expects Int"))) };
                    Ok(effect(move |_| { $fn(id, b).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
                }));
            };
        }

        macro_rules! bridge_unit_i {
            ($name:expr, $fn:path) => {
                fields.insert($name.to_string(), builtin(concat!("gtk4.", $name), 1, |mut args, _| {
                    let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid(concat!("gtk4.", $name, " expects Int"))) };
                    Ok(effect(move |_| { $fn(id).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
                }));
            };
        }

        macro_rules! bridge_int_t {
            ($name:expr, $fn:path) => {
                fields.insert($name.to_string(), builtin(concat!("gtk4.", $name), 1, |mut args, _| {
                    let t = match args.remove(0) { Value::Text(v) => v, _ => return Err(invalid(concat!("gtk4.", $name, " expects Text"))) };
                    Ok(effect(move |_| { let r = $fn(&t).map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
                }));
            };
        }

        // ── Window ops ──
        bridge_unit_it!("windowSetTitle", aivi_gtk4::window_set_title);
        bridge_unit_ii!("windowSetTitlebar", aivi_gtk4::window_set_titlebar);
        bridge_unit_ii!("windowSetChild", aivi_gtk4::window_set_child);
        bridge_unit_i!("windowPresent", aivi_gtk4::window_present);
        bridge_unit_i!("windowClose", aivi_gtk4::window_close);
        bridge_unit_ib!("windowSetHideOnClose", aivi_gtk4::window_set_hide_on_close);
        bridge_unit_ib!("windowSetDecorated", aivi_gtk4::window_set_decorated);

        fields.insert("windowOnClose".to_string(), builtin("gtk4.windowOnClose", 2, |mut args, _| {
            let sig = match args.remove(1) { Value::Text(v) => v, _ => return Err(invalid("gtk4.windowOnClose expects Text")) };
            let win_id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("gtk4.windowOnClose expects Int")) };
            Ok(effect(move |_| { aivi_gtk4::window_on_close(win_id, &sig).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        // ── Widget ops ──
        bridge_unit_i!("widgetShow", aivi_gtk4::widget_show);
        bridge_unit_i!("widgetHide", aivi_gtk4::widget_hide);
        bridge_bool_it!("widgetGetBoolProperty", aivi_gtk4::widget_get_bool_property);

        fields.insert("widgetSetBoolProperty".to_string(), builtin("gtk4.widgetSetBoolProperty", 3, |mut args, _| {
            let value = match args.remove(2) { Value::Bool(v) => v, _ => return Err(invalid("gtk4.widgetSetBoolProperty expects Bool")) };
            let prop = match args.remove(1) { Value::Text(v) => v, _ => return Err(invalid("gtk4.widgetSetBoolProperty expects Text")) };
            let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("gtk4.widgetSetBoolProperty expects Int")) };
            Ok(effect(move |_| { aivi_gtk4::widget_set_bool_property(id, &prop, value).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        fields.insert("widgetSetSizeRequest".to_string(), builtin("gtk4.widgetSetSizeRequest", 3, |mut args, _| {
            let h = match args.remove(2) { Value::Int(v) => v as i32, _ => return Err(invalid("expects Int")) };
            let w = match args.remove(1) { Value::Int(v) => v as i32, _ => return Err(invalid("expects Int")) };
            let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { aivi_gtk4::widget_set_size_request(id, w, h).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        bridge_unit_ib!("widgetSetHexpand", aivi_gtk4::widget_set_hexpand);
        bridge_unit_ib!("widgetSetVexpand", aivi_gtk4::widget_set_vexpand);

        fields.insert("widgetSetHalign".to_string(), builtin("gtk4.widgetSetHalign", 2, |mut args, _| {
            let a = match args.remove(1) { Value::Int(v) => v as i32, _ => return Err(invalid("expects Int")) };
            let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { aivi_gtk4::widget_set_halign(id, a).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        fields.insert("widgetSetValign".to_string(), builtin("gtk4.widgetSetValign", 2, |mut args, _| {
            let a = match args.remove(1) { Value::Int(v) => v as i32, _ => return Err(invalid("expects Int")) };
            let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { aivi_gtk4::widget_set_valign(id, a).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        macro_rules! bridge_margin {
            ($name:expr, $fn:path) => {
                fields.insert($name.to_string(), builtin(concat!("gtk4.", $name), 2, |mut args, _| {
                    let m = match args.remove(1) { Value::Int(v) => v as i32, _ => return Err(invalid("expects Int")) };
                    let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
                    Ok(effect(move |_| { $fn(id, m).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
                }));
            };
        }
        bridge_margin!("widgetSetMarginStart", aivi_gtk4::widget_set_margin_start);
        bridge_margin!("widgetSetMarginEnd", aivi_gtk4::widget_set_margin_end);
        bridge_margin!("widgetSetMarginTop", aivi_gtk4::widget_set_margin_top);
        bridge_margin!("widgetSetMarginBottom", aivi_gtk4::widget_set_margin_bottom);

        bridge_unit_it!("widgetAddCssClass", aivi_gtk4::widget_add_css_class);
        bridge_unit_it!("widgetRemoveCssClass", aivi_gtk4::widget_remove_css_class);
        bridge_unit_it!("widgetSetTooltipText", aivi_gtk4::widget_set_tooltip_text);

        fields.insert("widgetSetOpacity".to_string(), builtin("gtk4.widgetSetOpacity", 2, |mut args, _| {
            let opacity = match args.remove(1) { Value::Int(v) => v as f64 / 100.0, _ => return Err(invalid("expects Int")) };
            let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { aivi_gtk4::widget_set_opacity(id, opacity).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        bridge_unit_it!("widgetSetCss", aivi_gtk4::widget_set_css);
        bridge_int_t!("widgetById", aivi_gtk4::widget_by_id);
        bridge_unit_ii!("widgetAddController", aivi_gtk4::widget_add_controller);
        bridge_unit_ii!("widgetAddShortcut", aivi_gtk4::widget_add_shortcut);
        bridge_unit_ii!("widgetSetLayoutManager", aivi_gtk4::widget_set_layout_manager);

        // ── Box ──
        fields.insert("boxNew".to_string(), builtin("gtk4.boxNew", 2, |mut args, _| {
            let spacing = match args.remove(1) { Value::Int(v) => v as i32, _ => return Err(invalid("expects Int")) };
            let ori = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { let r = aivi_gtk4::box_new(ori, spacing).map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));
        bridge_unit_ii!("boxAppend", aivi_gtk4::box_append);
        bridge_unit_ib!("boxSetHomogeneous", aivi_gtk4::box_set_homogeneous);

        // ── Button ──
        bridge_int_t!("buttonNew", aivi_gtk4::button_new);
        bridge_unit_it!("buttonSetLabel", aivi_gtk4::button_set_label);
        bridge_int_t!("buttonNewFromIconName", aivi_gtk4::button_new_from_icon_name);
        bridge_unit_ii!("buttonSetChild", aivi_gtk4::button_set_child);

        // ── Label ──
        bridge_int_t!("labelNew", aivi_gtk4::label_new);
        bridge_unit_it!("labelSetText", aivi_gtk4::label_set_text);
        bridge_unit_ib!("labelSetWrap", aivi_gtk4::label_set_wrap);
        bridge_margin!("labelSetEllipsize", aivi_gtk4::label_set_ellipsize);
        fields.insert("labelSetXalign".to_string(), builtin("gtk4.labelSetXalign", 2, |mut args, _| {
            let x = match args.remove(1) { Value::Int(v) => v as f32 / 100.0, _ => return Err(invalid("expects Int")) };
            let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { aivi_gtk4::label_set_xalign(id, x).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));
        bridge_margin!("labelSetMaxWidthChars", aivi_gtk4::label_set_max_width_chars);

        // ── Entry ──
        fields.insert("entryNew".to_string(), builtin("gtk4.entryNew", 1, |mut args, _| {
            match args.remove(0) { Value::Unit => {} _ => return Err(invalid("expects Unit")) }
            Ok(effect(move |_| { let r = aivi_gtk4::entry_new().map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));
        bridge_unit_it!("entrySetText", aivi_gtk4::entry_set_text);
        fields.insert("entryText".to_string(), builtin("gtk4.entryText", 1, |mut args, _| {
            let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { let r = aivi_gtk4::entry_text(id).map_err(gtk4_err_to_runtime)?; Ok(Value::Text(r)) }))
        }));

        // ── Image ──
        bridge_int_t!("imageNewFromFile", aivi_gtk4::image_new_from_file);
        bridge_unit_it!("imageSetFile", aivi_gtk4::image_set_file);
        bridge_int_t!("imageNewFromResource", aivi_gtk4::image_new_from_resource);
        bridge_unit_it!("imageSetResource", aivi_gtk4::image_set_resource);
        bridge_int_t!("imageNewFromIconName", aivi_gtk4::image_new_from_icon_name);
        bridge_margin!("imageSetPixelSize", aivi_gtk4::image_set_pixel_size);

        // ── Icon theme ──
        fields.insert("iconThemeAddSearchPath".to_string(), builtin("gtk4.iconThemeAddSearchPath", 1, |mut args, _| {
            let path = match args.remove(0) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            Ok(effect(move |_| { aivi_gtk4::icon_theme_add_search_path(&path).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        // ── Scroll, separator, overlay, draw area ──
        fields.insert("scrollAreaNew".to_string(), builtin("gtk4.scrollAreaNew", 1, |mut args, _| {
            match args.remove(0) { Value::Unit => {} _ => return Err(invalid("expects Unit")) }
            Ok(effect(move |_| { let r = aivi_gtk4::scroll_area_new().map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));
        bridge_unit_ii!("scrollAreaSetChild", aivi_gtk4::scroll_area_set_child);
        fields.insert("scrollAreaSetPolicy".to_string(), builtin("gtk4.scrollAreaSetPolicy", 3, |mut args, _| {
            let vp = match args.remove(2) { Value::Int(v) => v as i32, _ => return Err(invalid("expects Int")) };
            let hp = match args.remove(1) { Value::Int(v) => v as i32, _ => return Err(invalid("expects Int")) };
            let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { aivi_gtk4::scroll_area_set_policy(id, hp, vp).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        fields.insert("separatorNew".to_string(), builtin("gtk4.separatorNew", 1, |mut args, _| {
            let ori = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { let r = aivi_gtk4::separator_new(ori).map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));

        fields.insert("overlayNew".to_string(), builtin("gtk4.overlayNew", 1, |mut args, _| {
            match args.remove(0) { Value::Unit => {} _ => return Err(invalid("expects Unit")) }
            Ok(effect(move |_| { let r = aivi_gtk4::overlay_new().map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));
        bridge_unit_ii!("overlaySetChild", aivi_gtk4::overlay_set_child);
        bridge_unit_ii!("overlayAddOverlay", aivi_gtk4::overlay_add_overlay);

        fields.insert("drawAreaNew".to_string(), builtin("gtk4.drawAreaNew", 2, |mut args, _| {
            let h = match args.remove(1) { Value::Int(v) => v as i32, _ => return Err(invalid("expects Int")) };
            let w = match args.remove(0) { Value::Int(v) => v as i32, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { let r = aivi_gtk4::draw_area_new(w, h).map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));
        fields.insert("drawAreaSetContentSize".to_string(), builtin("gtk4.drawAreaSetContentSize", 3, |mut args, _| {
            let h = match args.remove(2) { Value::Int(v) => v as i32, _ => return Err(invalid("expects Int")) };
            let w = match args.remove(1) { Value::Int(v) => v as i32, _ => return Err(invalid("expects Int")) };
            let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { aivi_gtk4::draw_area_set_content_size(id, w, h).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));
        bridge_unit_i!("drawAreaQueueDraw", aivi_gtk4::draw_area_queue_draw);

        // ── Gesture, clipboard ──
        fields.insert("gestureClickNew".to_string(), builtin("gtk4.gestureClickNew", 1, |mut args, _| {
            let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { let r = aivi_gtk4::gesture_click_new(id).map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));
        fields.insert("gestureClickLastButton".to_string(), builtin("gtk4.gestureClickLastButton", 1, |mut args, _| {
            let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { let r = aivi_gtk4::gesture_click_last_button(id).map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));

        fields.insert("clipboardDefault".to_string(), builtin("gtk4.clipboardDefault", 1, |mut args, _| {
            match args.remove(0) { Value::Unit => {} _ => return Err(invalid("expects Unit")) }
            Ok(effect(move |_| { let r = aivi_gtk4::clipboard_default().map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));
        bridge_unit_it!("clipboardSetText", aivi_gtk4::clipboard_set_text);
        fields.insert("clipboardText".to_string(), builtin("gtk4.clipboardText", 1, |mut args, _| {
            let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { let r = aivi_gtk4::clipboard_text(id).map_err(gtk4_err_to_runtime)?; Ok(Value::Text(r)) }))
        }));

        // ── Action, shortcut, notification ──
        bridge_int_t!("actionNew", aivi_gtk4::action_new);
        bridge_unit_ib!("actionSetEnabled", aivi_gtk4::action_set_enabled);
        bridge_unit_ii!("appAddAction", aivi_gtk4::app_add_action);
        fields.insert("shortcutNew".to_string(), builtin("gtk4.shortcutNew", 2, |mut args, _| {
            let action = match args.remove(1) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            let accel = match args.remove(0) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            Ok(effect(move |_| { let r = aivi_gtk4::shortcut_new(&accel, &action).map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));
        fields.insert("notificationNew".to_string(), builtin("gtk4.notificationNew", 2, |mut args, _| {
            let body = match args.remove(1) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            let title = match args.remove(0) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            Ok(effect(move |_| { let r = aivi_gtk4::notification_new(&title, &body).map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));
        bridge_unit_it!("notificationSetBody", aivi_gtk4::notification_set_body);
        fields.insert("appSendNotification".to_string(), builtin("gtk4.appSendNotification", 3, |mut args, _| {
            let nid = match args.remove(2) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            let tag = match args.remove(1) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            let app_id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { aivi_gtk4::app_send_notification(app_id, &tag, nid).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));
        fields.insert("appWithdrawNotification".to_string(), builtin("gtk4.appWithdrawNotification", 2, |mut args, _| {
            let tag = match args.remove(1) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            let app_id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { aivi_gtk4::app_withdraw_notification(app_id, &tag).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));
        bridge_int_t!("layoutManagerNew", aivi_gtk4::layout_manager_new);

        // ── Drag/drop stubs ──
        fields.insert("dragSourceNew".to_string(), builtin("gtk4.dragSourceNew", 1, |mut args, _| {
            let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { let r = aivi_gtk4::drag_source_new(id).map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));
        bridge_unit_it!("dragSourceSetText", aivi_gtk4::drag_source_set_text);
        fields.insert("dropTargetNew".to_string(), builtin("gtk4.dropTargetNew", 1, |mut args, _| {
            let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { let r = aivi_gtk4::drop_target_new(id).map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));
        fields.insert("dropTargetLastText".to_string(), builtin("gtk4.dropTargetLastText", 1, |mut args, _| {
            let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { let r = aivi_gtk4::drop_target_last_text(id).map_err(gtk4_err_to_runtime)?; Ok(Value::Text(r)) }))
        }));

        // ── Menu, dialog ──
        fields.insert("menuModelNew".to_string(), builtin("gtk4.menuModelNew", 1, |mut args, _| {
            match args.remove(0) { Value::Unit => {} _ => return Err(invalid("expects Unit")) }
            Ok(effect(move |_| { let r = aivi_gtk4::menu_model_new().map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));
        fields.insert("menuModelAppendItem".to_string(), builtin("gtk4.menuModelAppendItem", 3, |mut args, _| {
            let action = match args.remove(2) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            let label = match args.remove(1) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { aivi_gtk4::menu_model_append_item(id, &label, &action).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));
        bridge_int_t!("menuButtonNew", aivi_gtk4::menu_button_new);
        bridge_unit_ii!("menuButtonSetMenuModel", aivi_gtk4::menu_button_set_menu_model);

        fields.insert("dialogNew".to_string(), builtin("gtk4.dialogNew", 1, |mut args, _| {
            let _ = args.remove(0);
            Ok(effect(move |_| { let r = aivi_gtk4::dialog_new().map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));
        bridge_unit_it!("dialogSetTitle", aivi_gtk4::dialog_set_title);
        bridge_unit_ii!("dialogSetChild", aivi_gtk4::dialog_set_child);
        bridge_unit_ii!("dialogPresent", aivi_gtk4::dialog_present);
        bridge_unit_i!("dialogClose", aivi_gtk4::dialog_close);
        bridge_unit_ii!("adwDialogPresent", aivi_gtk4::adw_dialog_present);

        // ── File dialog stubs ──
        fields.insert("fileDialogNew".to_string(), builtin("gtk4.fileDialogNew", 1, |mut args, _| {
            match args.remove(0) { Value::Unit => {} _ => return Err(invalid("expects Unit")) }
            Ok(effect(move |_| { let r = aivi_gtk4::file_dialog_new().map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));
        fields.insert("fileDialogSelectFile".to_string(), builtin("gtk4.fileDialogSelectFile", 1, |mut args, _| {
            let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { let r = aivi_gtk4::file_dialog_select_file(id).map_err(gtk4_err_to_runtime)?; Ok(Value::Text(r)) }))
        }));

        // ── List/tree view stubs ──
        fields.insert("listStoreNew".to_string(), builtin("gtk4.listStoreNew", 1, |mut args, _| {
            match args.remove(0) { Value::Unit => {} _ => return Err(invalid("expects Unit")) }
            Ok(effect(move |_| { let r = aivi_gtk4::list_store_new().map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));
        bridge_unit_it!("listStoreAppendText", aivi_gtk4::list_store_append_text);
        fields.insert("listStoreItems".to_string(), builtin("gtk4.listStoreItems", 1, |mut args, _| {
            let id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { let _r = aivi_gtk4::list_store_items(id).map_err(gtk4_err_to_runtime)?; Ok(Value::List(Arc::new(Vec::new()))) }))
        }));
        fields.insert("listViewNew".to_string(), builtin("gtk4.listViewNew", 1, |mut args, _| {
            match args.remove(0) { Value::Unit => {} _ => return Err(invalid("expects Unit")) }
            Ok(effect(move |_| { let r = aivi_gtk4::list_view_new().map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));
        bridge_unit_ii!("listViewSetModel", aivi_gtk4::list_view_set_model);
        fields.insert("treeViewNew".to_string(), builtin("gtk4.treeViewNew", 1, |mut args, _| {
            match args.remove(0) { Value::Unit => {} _ => return Err(invalid("expects Unit")) }
            Ok(effect(move |_| { let r = aivi_gtk4::tree_view_new().map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
        }));
        bridge_unit_ii!("treeViewSetModel", aivi_gtk4::tree_view_set_model);

        // ── OS ──
        fields.insert("osOpenUri".to_string(), builtin("gtk4.osOpenUri", 2, |mut args, _| {
            let uri = match args.remove(1) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            let app_id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { aivi_gtk4::os_open_uri(app_id, &uri).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));
        fields.insert("osShowInFileManager".to_string(), builtin("gtk4.osShowInFileManager", 1, |mut args, _| {
            let path = match args.remove(0) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            Ok(effect(move |_| { aivi_gtk4::os_show_in_file_manager(&path).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));
        fields.insert("osSetBadgeCount".to_string(), builtin("gtk4.osSetBadgeCount", 2, |mut args, _| {
            let count = match args.remove(1) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            let app_id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { aivi_gtk4::os_set_badge_count(app_id, count).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));
        fields.insert("osThemePreference".to_string(), builtin("gtk4.osThemePreference", 1, |mut args, _| {
            match args.remove(0) { Value::Unit => {} _ => return Err(invalid("expects Unit")) }
            Ok(effect(move |_| { let r = aivi_gtk4::os_theme_preference().map_err(gtk4_err_to_runtime)?; Ok(Value::Text(r)) }))
        }));

        // ── Build / reconcile ──
        fields.insert("buildFromNode".to_string(), builtin("gtk4.buildFromNode", 1, |mut args, _| {
            let node = args.remove(0);
            Ok(effect(move |runtime| {
                let node = runtime.force_value(node.clone())?;
                let decoded = resolve_gtk_node(&node, runtime)?;
                let result = materialize_with_bindings(&decoded, runtime)?;
                Ok(Value::Int(result.root_id))
            }))
        }));

        fields.insert("buildWithIds".to_string(), builtin("gtk4.buildWithIds", 1, |mut args, _| {
            let node = args.remove(0);
            Ok(effect(move |runtime| {
                let node = runtime.force_value(node.clone())?;
                let decoded = resolve_gtk_node(&node, runtime)?;
                let result = materialize_with_bindings(&decoded, runtime)?;
                let mut widgets_map = im::HashMap::new();
                for (name, wid) in result.named_widgets {
                    widgets_map.insert(crate::runtime::values::KeyValue::Text(name), Value::Int(wid));
                }
                let mut record = HashMap::new();
                record.insert("root".to_string(), Value::Int(result.root_id));
                record.insert("widgets".to_string(), Value::Map(Arc::new(widgets_map)));
                Ok(Value::Record(Arc::new(record)))
            }))
        }));

        fields.insert("reconcileNode".to_string(), builtin("gtk4.reconcileNode", 2, |mut args, _| {
            let new_node_val = args.remove(1);
            let root_id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |runtime| {
                let node = runtime.force_value(new_node_val.clone())?;
                let decoded = resolve_gtk_node(&node, runtime)?;
                let mut next_binding_id = 1;
                let mut bindings = Vec::new();
                let mut structural_bindings = Vec::new();
                let decoded = materialize_node(
                    &decoded,
                    runtime,
                    &mut next_binding_id,
                    &mut bindings,
                    &mut structural_bindings,
                )?;
                let result_id = aivi_gtk4::reconcile_node(root_id, &decoded).map_err(gtk4_err_to_runtime)?;
                Ok(Value::Int(result_id))
            }))
        }));

        // ── Signal ──
        fields.insert("signalPoll".to_string(), builtin("gtk4.signalPoll", 1, |mut args, _| {
            match args.remove(0) { Value::Unit => {} _ => return Err(invalid("expects Unit")) }
            Ok(effect(move |_| {
                match aivi_gtk4::signal_poll().map_err(gtk4_err_to_runtime)? {
                    Some(event) => Ok(Value::Constructor {
                        name: "Some".to_string(),
                        args: vec![make_signal_event_value(event)],
                    }),
                    None => Ok(Value::Constructor { name: "None".to_string(), args: Vec::new() }),
                }
            }))
        }));

        fields.insert("signalStream".to_string(), builtin("gtk4.signalStream", 1, |mut args, _| {
            match args.remove(0) { Value::Unit => {} _ => return Err(invalid("expects Unit")) }
            Ok(effect(move |_| {
                let receiver = aivi_gtk4::signal_stream().map_err(gtk4_err_to_runtime)?;
                // Convert SignalEvent receiver to Value receiver
                let (value_sender, value_receiver) = mpsc::sync_channel(512);
                std::thread::Builder::new()
                    .name("gtk4-signal-bridge".to_string())
                    .spawn(move || {
                        loop {
                            match receiver.recv() {
                                Ok(event) => {
                                    let value = make_signal_event_value(event);
                                    if value_sender.send(value).is_err() {
                                        eprintln!("[bridge] value_sender.send FAILED, exiting");
                                        break;
                                    }
                                }
                                Err(e) => {
                                    eprintln!("[bridge] receiver.recv error: {:?}, exiting", e);
                                    break;
                                }
                            }
                        }

                    })
                    .ok();
                let inner = Arc::new(ChannelInner {
                    sender: Mutex::new(None),
                    receiver: Mutex::new(value_receiver),
                    closed: AtomicBool::new(false),
                });
                Ok(Value::ChannelRecv(Arc::new(ChannelRecv { inner })))
            }))
        }));


        // dbusServerStart : Unit -> Effect GtkError Unit
        fields.insert("dbusServerStart".to_string(), builtin("gtk4.dbusServerStart", 1, |mut args, _| {
            match args.remove(0) { Value::Unit => {} _ => return Err(invalid("expects Unit")) }
            Ok(effect(move |_| {
                aivi_gtk4::dbus_server_start().map_err(gtk4_err_to_runtime)?;
                Ok(Value::Unit)
            }))
        }));
        fields.insert("signalEmit".to_string(), builtin("gtk4.signalEmit", 4, |mut args, _| {
            let payload = match args.remove(3) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            let handler = match args.remove(2) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            let signal = match args.remove(1) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            let widget_id = match args.remove(0) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { aivi_gtk4::signal_emit(widget_id, &signal, &handler, &payload).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        fields.insert("setInterval".to_string(), builtin("gtk4.setInterval", 1, |mut args, _| {
            let ms = match args.remove(0) { Value::Int(v) => v as u32, _ => return Err(invalid("expects Int")) };
            Ok(effect(move |_| { aivi_gtk4::set_interval(ms).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        // ── Signal bindings ──
        fields.insert("signalBindBoolProperty".to_string(), builtin("gtk4.signalBindBoolProperty", 4, |mut args, _| {
            let value = match args.remove(3) { Value::Bool(v) => v, _ => return Err(invalid("expects Bool")) };
            let prop = match args.remove(2) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            let wid = match args.remove(1) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            let handler = match args.remove(0) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            Ok(effect(move |_| { aivi_gtk4::signal_bind_bool_property(&handler, wid, &prop, value).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        fields.insert("signalBindCssClass".to_string(), builtin("gtk4.signalBindCssClass", 4, |mut args, _| {
            let add = match args.remove(3) { Value::Bool(v) => v, _ => return Err(invalid("expects Bool")) };
            let class = match args.remove(2) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            let wid = match args.remove(1) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            let handler = match args.remove(0) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            Ok(effect(move |_| { aivi_gtk4::signal_bind_css_class(&handler, wid, &class, add).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        fields.insert("signalBindToggleBoolProperty".to_string(), builtin("gtk4.signalBindToggleBoolProperty", 3, |mut args, _| {
            let prop = match args.remove(2) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            let wid = match args.remove(1) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            let handler = match args.remove(0) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            Ok(effect(move |_| { aivi_gtk4::signal_bind_toggle_bool_property(&handler, wid, &prop).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        fields.insert("signalToggleCssClass".to_string(), builtin("gtk4.signalToggleCssClass", 3, |mut args, _| {
            let class = match args.remove(2) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            let wid = match args.remove(1) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            let handler = match args.remove(0) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            Ok(effect(move |_| { aivi_gtk4::signal_toggle_css_class(&handler, wid, &class).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        fields.insert("signalBindDialogPresent".to_string(), builtin("gtk4.signalBindDialogPresent", 3, |mut args, _| {
            let parent_id = match args.remove(2) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            let dialog_id = match args.remove(1) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            let handler = match args.remove(0) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            Ok(effect(move |_| { aivi_gtk4::signal_bind_dialog_present(&handler, dialog_id, parent_id).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        fields.insert("signalBindStackPage".to_string(), builtin("gtk4.signalBindStackPage", 3, |mut args, _| {
            let page = match args.remove(2) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            let stack_id = match args.remove(1) { Value::Int(v) => v, _ => return Err(invalid("expects Int")) };
            let handler = match args.remove(0) { Value::Text(v) => v, _ => return Err(invalid("expects Text")) };
            Ok(effect(move |_| { aivi_gtk4::signal_bind_stack_page(&handler, stack_id, &page).map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        fields.insert("serializeSignal".to_string(), builtin("gtk4.serializeSignal", 1, |mut args, _| {
            let val = args.pop().unwrap();
            Ok(Value::Text(serialize_signal_value(&val)))
        }));

        fields
    }
}

#[cfg(all(test, feature = "gtk4-libadwaita", target_os = "linux"))]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::{Mutex, Once};
    use std::time::Duration;

    use serde_json::json;

    use super::super::gtk4::{ResolvedGtkAttr, ResolvedGtkNode};
    use super::bridge::{
        execute_runtime_handler, make_signal_event_value, materialize_with_bindings,
        ui_debug_inspect_signal_json, ui_debug_list_signals_json,
    };
    use crate::runtime::builtins::builtin;
    use crate::runtime::constructors::core_constructor_ordinals;
    use crate::runtime::environment::{Env, RuntimeContext};
    use crate::runtime::{format_runtime_error, CancelToken, EffectValue, Runtime, Value};

    fn test_ctx() -> Arc<RuntimeContext> {
        Arc::new(RuntimeContext::new_with_constructor_ordinals(
            Env::new(None),
            core_constructor_ordinals(),
        ))
    }

    fn clicked_event() -> aivi_gtk4::SignalEvent {
        aivi_gtk4::SignalEvent {
            widget_id: 42,
            widget_name: "button".to_string(),
            signal: "clicked".to_string(),
            handler: String::new(),
            payload: String::new(),
        }
    }

    fn ensure_gtk() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            aivi_gtk4::init().unwrap_or_else(|err| panic!("init gtk: {}", err.message));
        });
    }

    fn gtk_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GTK_TEST_MUTEX: Mutex<()> = Mutex::new(());
        GTK_TEST_MUTEX
            .lock()
            .expect("GTK test mutex should not be poisoned")
    }

    fn ok_or_panic<T>(result: Result<T, crate::runtime::RuntimeError>, context: &str) -> T {
        match result {
            Ok(value) => value,
            Err(err) => panic!("{context}: {}", format_runtime_error(err)),
        }
    }

    #[test]
    fn ui_debug_signal_snapshot_reports_dependencies_and_watchers() {
        let ctx = test_ctx();
        let mut runtime = Runtime::new(ctx.clone(), CancelToken::root());
        let count = ok_or_panic(runtime.reactive_create_signal(Value::Int(1)), "create signal");
        let mapper = builtin("test.signalMapper", 1, |mut args, _| {
            let value = match args.remove(0) {
                Value::Int(value) => value,
                other => panic!("expected Int, got {:?}", other),
            };
            Ok(Value::Int(value + 1))
        });
        let derived = ok_or_panic(
            runtime.reactive_derive_signal(count.clone(), mapper),
            "derive signal",
        );
        let callback = builtin("test.signalWatcher", 1, |_args, _| Ok(Value::Unit));
        ok_or_panic(
            runtime.reactive_watch_signal(derived.clone(), callback),
            "watch signal",
        );

        let list = ui_debug_list_signals_json(ctx.as_ref()).expect("list signals");
        let signals = list["signals"].as_array().expect("signals array");
        assert_eq!(signals.len(), 2);
        assert_eq!(list["watcherCount"].as_u64(), Some(1));

        let derived_id = match derived {
            Value::Signal(signal) => signal.id,
            other => panic!("expected derived signal, got {:?}", other),
        };
        let detail = ui_debug_inspect_signal_json(ctx.as_ref(), &json!({ "signalId": derived_id }).as_object().cloned().expect("params"))
            .expect("inspect signal");
        let signal = &detail["signal"];
        assert_eq!(signal["id"].as_u64(), Some(derived_id as u64));
        assert_eq!(
            signal["dependencies"].as_array().map(|items| items.len()),
            Some(1)
        );
        assert_eq!(signal["watcherCount"].as_u64(), Some(1));
        assert_eq!(
            signal["compute"]["display"].as_str(),
            Some("<builtin:test.signalMapper>")
        );
        assert_eq!(
            signal["watchers"].as_array().map(|items| items.len()),
            Some(1)
        );
    }

    #[test]
    fn maps_key_pressed_signal_into_typed_event() {
        let value = make_signal_event_value(aivi_gtk4::SignalEvent {
            widget_id: 42,
            widget_name: "game".to_string(),
            signal: "key-pressed".to_string(),
            handler: String::new(),
            payload: "Up\n111".to_string(),
        });
        match value {
            Value::Constructor { name, args } => {
                assert_eq!(name, "GtkKeyPressed");
                assert_eq!(args.len(), 4);
                assert!(matches!(args[0], Value::Int(42)));
                assert!(matches!(args[1], Value::Text(ref text) if text == "game"));
                assert!(matches!(args[2], Value::Text(ref text) if text == "Up"));
                assert!(matches!(args[3], Value::Text(ref text) if text == "111"));
            }
            other => panic!("expected GtkKeyPressed, got {other:?}"),
        }
    }

    #[test]
    fn runtime_handler_callback_updates_shared_signal_graph() {
        let ctx = test_ctx();
        let mut runtime = Runtime::new(ctx.clone(), CancelToken::root());
        let count = ok_or_panic(runtime.reactive_create_signal(Value::Int(0)), "create signal");
        let count_for_handler = count.clone();
        let handler = builtin("test.gtkRuntimeHandler", 1, move |mut args, runtime| {
            let _event = args.remove(0);
            runtime.reactive_set_signal(count_for_handler.clone(), Value::Int(7))
        });

        ok_or_panic(
            execute_runtime_handler(ctx, handler, clicked_event()),
            "run handler",
        );

        match ok_or_panic(runtime.reactive_get_signal(count), "read updated signal") {
            Value::Int(value) => assert_eq!(value, 7),
            other => panic!("expected Int(7), got {other:?}"),
        }
    }

    #[test]
    fn runtime_handler_callback_flushes_live_binding_updates_on_main_thread_tick() {
        let _gtk = gtk_test_guard();
        ensure_gtk();
        let ctx = test_ctx();
        let mut runtime = Runtime::new(ctx.clone(), CancelToken::root());
        let text = ok_or_panic(
            runtime.reactive_create_signal(Value::Text("before".to_string())),
            "create source text",
        );
        let derived = ok_or_panic(
            runtime.reactive_derive_signal(
                text.clone(),
                builtin("test.deriveEntryText", 1, |mut args, _| Ok(args.remove(0))),
            ),
            "derive entry text",
        );
        let node = ResolvedGtkNode::Element {
            tag: "object".to_string(),
            attrs: vec![
                ResolvedGtkAttr::StaticAttr {
                    name: "class".to_string(),
                    value: "GtkEntry".to_string(),
                },
                ResolvedGtkAttr::Id("entry".to_string()),
                ResolvedGtkAttr::BoundProp {
                    name: "text".to_string(),
                    value: derived,
                },
            ],
            children: Vec::new(),
        };

        let result = ok_or_panic(materialize_with_bindings(&node, &mut runtime), "build entry");
        let entry_id = *result
            .named_widgets
            .get("entry")
            .expect("entry widget should be named");
        assert_eq!(
            aivi_gtk4::entry_text(entry_id)
                .unwrap_or_else(|err| panic!("read initial entry text: {}", err.message)),
            "before"
        );

        let text_for_handler = text.clone();
        let handler = builtin("test.gtkRuntimeHandlerDeferred", 1, move |mut args, runtime| {
            let _event = args.remove(0);
            runtime.reactive_set_signal(text_for_handler.clone(), Value::Text("after".to_string()))
        });
        let handler_ctx = ctx.clone();
        std::thread::spawn(move || {
            ok_or_panic(
                execute_runtime_handler(handler_ctx, handler, clicked_event()),
                "run deferred handler",
            );
        })
        .join()
        .expect("runtime handler thread should not panic");

        match ok_or_panic(runtime.reactive_get_signal(text), "read updated source text") {
            Value::Text(value) => assert_eq!(value, "after"),
            other => panic!("expected updated Text, got {other:?}"),
        }
        assert_eq!(
            aivi_gtk4::entry_text(entry_id)
                .unwrap_or_else(|err| panic!("read stale entry text: {}", err.message)),
            "before"
        );

        ok_or_panic(runtime.reactive_flush_deferred(), "flush deferred bindings");
        assert_eq!(
            aivi_gtk4::entry_text(entry_id)
                .unwrap_or_else(|err| panic!("read flushed entry text: {}", err.message)),
            "after"
        );
    }

    #[test]
    fn runtime_event_handle_runs_shared_effect() {
        let ctx = test_ctx();
        let mut runtime = Runtime::new(ctx.clone(), CancelToken::root());
        let count = ok_or_panic(runtime.reactive_create_signal(Value::Int(0)), "create signal");
        let count_for_effect = count.clone();
        let run = Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new(move |runtime| {
                runtime.reactive_set_signal(count_for_effect.clone(), Value::Int(11))?;
                Ok(Value::Unit)
            }),
        }));
        let mut fields = HashMap::new();
        fields.insert("run".to_string(), run);

        ok_or_panic(
            execute_runtime_handler(ctx, Value::Record(Arc::new(fields)), clicked_event()),
            "run event handle",
        );

        match ok_or_panic(runtime.reactive_get_signal(count), "read updated signal") {
            Value::Int(value) => assert_eq!(value, 11),
            other => panic!("expected Int(11), got {other:?}"),
        }
    }

    #[test]
    fn live_bound_entry_text_updates_from_signal_write() {
        let _gtk = gtk_test_guard();
        ensure_gtk();
        let ctx = test_ctx();
        let mut runtime = Runtime::new(ctx, CancelToken::root());
        let text = ok_or_panic(
            runtime.reactive_create_signal(Value::Text("hello".to_string())),
            "create signal",
        );
        let node = ResolvedGtkNode::Element {
            tag: "object".to_string(),
            attrs: vec![
                ResolvedGtkAttr::StaticAttr {
                    name: "class".to_string(),
                    value: "GtkEntry".to_string(),
                },
                ResolvedGtkAttr::Id("entry".to_string()),
                ResolvedGtkAttr::BoundProp {
                    name: "text".to_string(),
                    value: text.clone(),
                },
            ],
            children: Vec::new(),
        };

        let result = ok_or_panic(materialize_with_bindings(&node, &mut runtime), "build entry");
        let entry_id = *result
            .named_widgets
            .get("entry")
            .expect("entry widget should be named");
        assert_eq!(
            aivi_gtk4::entry_text(entry_id)
                .unwrap_or_else(|err| panic!("read entry text: {}", err.message)),
            "hello"
        );

        ok_or_panic(
            runtime.reactive_set_signal(text, Value::Text("world".to_string())),
            "set signal",
        );
        assert_eq!(
            aivi_gtk4::entry_text(entry_id)
                .unwrap_or_else(|err| panic!("read entry text: {}", err.message)),
            "world"
        );
    }

    #[test]
    fn live_property_child_text_updates_from_signal_write() {
        let _gtk = gtk_test_guard();
        ensure_gtk();
        let ctx = test_ctx();
        let mut runtime = Runtime::new(ctx, CancelToken::root());
        let text = ok_or_panic(
            runtime.reactive_create_signal(Value::Text("first".to_string())),
            "create signal",
        );
        let node = ResolvedGtkNode::Element {
            tag: "object".to_string(),
            attrs: vec![
                ResolvedGtkAttr::StaticAttr {
                    name: "class".to_string(),
                    value: "GtkEntry".to_string(),
                },
                ResolvedGtkAttr::Id("entry-property".to_string()),
            ],
            children: vec![ResolvedGtkNode::Element {
                tag: "property".to_string(),
                attrs: vec![ResolvedGtkAttr::StaticAttr {
                    name: "name".to_string(),
                    value: "text".to_string(),
                }],
                children: vec![ResolvedGtkNode::DynamicText(text.clone())],
            }],
        };

        let result = ok_or_panic(
            materialize_with_bindings(&node, &mut runtime),
            "build property entry",
        );
        let entry_id = *result
            .named_widgets
            .get("entry-property")
            .expect("entry widget should be named");
        assert_eq!(
            aivi_gtk4::entry_text(entry_id)
                .unwrap_or_else(|err| panic!("read entry text: {}", err.message)),
            "first"
        );

        ok_or_panic(
            runtime.reactive_set_signal(text, Value::Text("second".to_string())),
            "set signal",
        );
        assert_eq!(
            aivi_gtk4::entry_text(entry_id)
                .unwrap_or_else(|err| panic!("read entry text: {}", err.message)),
            "second"
        );
    }

    #[test]
    fn live_show_binding_toggles_widget_visibility() {
        let _gtk = gtk_test_guard();
        ensure_gtk();
        let ctx = test_ctx();
        let mut runtime = Runtime::new(ctx, CancelToken::root());
        let visible = ok_or_panic(
            runtime.reactive_create_signal(Value::Bool(false)),
            "create signal",
        );
        let node = ResolvedGtkNode::Element {
            tag: "object".to_string(),
            attrs: vec![
                ResolvedGtkAttr::StaticAttr {
                    name: "class".to_string(),
                    value: "GtkBox".to_string(),
                },
                ResolvedGtkAttr::Id("root".to_string()),
            ],
            children: vec![ResolvedGtkNode::Show {
                when: visible.clone(),
                child: Box::new(ResolvedGtkNode::Element {
                    tag: "object".to_string(),
                    attrs: vec![
                        ResolvedGtkAttr::StaticAttr {
                            name: "class".to_string(),
                            value: "GtkEntry".to_string(),
                        },
                        ResolvedGtkAttr::Id("show-entry".to_string()),
                    ],
                    children: Vec::new(),
                }),
            }],
        };

        let result = ok_or_panic(materialize_with_bindings(&node, &mut runtime), "build show");
        let entry_id = *result
            .named_widgets
            .get("show-entry")
            .expect("show entry should be named");
        assert!(
            !aivi_gtk4::widget_get_bool_property(entry_id, "visible")
                .unwrap_or_else(|err| panic!("read visible: {}", err.message))
        );

        ok_or_panic(
            runtime.reactive_set_signal(visible, Value::Bool(true)),
            "set visible",
        );
        assert!(
            aivi_gtk4::widget_get_bool_property(entry_id, "visible")
                .unwrap_or_else(|err| panic!("read visible: {}", err.message))
        );
    }

    #[test]
    fn live_split_view_show_sidebar_updates_from_signal_write() {
        let _gtk = gtk_test_guard();
        ensure_gtk();
        let ctx = test_ctx();
        let mut runtime = Runtime::new(ctx, CancelToken::root());
        let show_sidebar = ok_or_panic(
            runtime.reactive_create_signal(Value::Bool(false)),
            "create split view signal",
        );
        let node = ResolvedGtkNode::Element {
            tag: "object".to_string(),
            attrs: vec![
                ResolvedGtkAttr::StaticAttr {
                    name: "class".to_string(),
                    value: "AdwOverlaySplitView".to_string(),
                },
                ResolvedGtkAttr::Id("split-view".to_string()),
                ResolvedGtkAttr::BoundProp {
                    name: "show-sidebar".to_string(),
                    value: show_sidebar.clone(),
                },
            ],
            children: Vec::new(),
        };

        let result = ok_or_panic(
            materialize_with_bindings(&node, &mut runtime),
            "build split view",
        );
        let split_view_id = *result
            .named_widgets
            .get("split-view")
            .expect("split view widget should be named");
        assert!(
            !aivi_gtk4::widget_get_bool_property(split_view_id, "show-sidebar")
                .unwrap_or_else(|err| panic!("read initial show-sidebar: {}", err.message))
        );

        ok_or_panic(
            runtime.reactive_set_signal(show_sidebar, Value::Bool(true)),
            "set show-sidebar",
        );
        assert!(
            aivi_gtk4::widget_get_bool_property(split_view_id, "show-sidebar")
                .unwrap_or_else(|err| panic!("read updated show-sidebar: {}", err.message))
        );
    }

    #[test]
    fn live_each_binding_reconciles_container_children() {
        let _gtk = gtk_test_guard();
        ensure_gtk();
        let ctx = test_ctx();
        let mut runtime = Runtime::new(ctx, CancelToken::root());
        let items = ok_or_panic(
            runtime.reactive_create_signal(Value::List(Arc::new(vec![Value::Int(1)]))),
            "create items signal",
        );
        let template = builtin("test.eachTemplate", 1, |mut args, _runtime| {
            let item = args.remove(0);
            Ok(Value::Constructor {
                name: "GtkElement".to_string(),
                args: vec![
                    Value::Text("object".to_string()),
                    Value::List(Arc::new(vec![Value::Constructor {
                        name: "GtkStaticAttr".to_string(),
                        args: vec![
                            Value::Text("class".to_string()),
                            Value::Text("GtkEntry".to_string()),
                        ],
                    }])),
                    Value::List(Arc::new(vec![Value::Constructor {
                        name: "GtkElement".to_string(),
                        args: vec![
                            Value::Text("property".to_string()),
                            Value::List(Arc::new(vec![Value::Constructor {
                                name: "GtkStaticAttr".to_string(),
                                args: vec![
                                    Value::Text("name".to_string()),
                                    Value::Text("text".to_string()),
                                ],
                            }])),
                            Value::List(Arc::new(vec![Value::Constructor {
                                name: "GtkTextNode".to_string(),
                                args: vec![item],
                            }])),
                        ],
                    }])),
                ],
            })
        });
        let node = ResolvedGtkNode::Element {
            tag: "object".to_string(),
            attrs: vec![
                ResolvedGtkAttr::StaticAttr {
                    name: "class".to_string(),
                    value: "GtkBox".to_string(),
                },
                ResolvedGtkAttr::Id("root".to_string()),
            ],
            children: vec![ResolvedGtkNode::Each {
                items: items.clone(),
                template,
                _key: None,
            }],
        };

        let result = ok_or_panic(materialize_with_bindings(&node, &mut runtime), "build each");
        let root_id = *result
            .named_widgets
            .get("root")
            .expect("root widget should be named");
        assert_eq!(
            aivi_gtk4::widget_child_count(root_id)
                .unwrap_or_else(|err| panic!("read initial child count: {}", err.message)),
            1
        );

        ok_or_panic(
            runtime.reactive_set_signal(
                items,
                Value::List(Arc::new(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
            ),
            "update items",
        );
        assert_eq!(
            aivi_gtk4::widget_child_count(root_id)
                .unwrap_or_else(|err| panic!("read updated child count: {}", err.message)),
            3
        );
    }

    #[test]
    fn live_each_binding_reinstalls_nested_prop_bindings_after_reconcile() {
        let _gtk = gtk_test_guard();
        ensure_gtk();
        let ctx = test_ctx();
        let mut runtime = Runtime::new(ctx, CancelToken::root());
        let items = ok_or_panic(
            runtime.reactive_create_signal(Value::List(Arc::new(vec![Value::Int(1)]))),
            "create items signal",
        );
        let shared_text = ok_or_panic(
            runtime.reactive_create_signal(Value::Text("before".to_string())),
            "create shared text signal",
        );
        let shared_text_for_template = shared_text.clone();
        let template = builtin("test.eachDynamicTemplate", 1, move |mut args, runtime| {
            let item = args.remove(0);
            let entry_name = match item {
                Value::Int(value) => format!("entry-{value}"),
                other => panic!("expected Int item, got {other:?}"),
            };
            Ok(Value::Constructor {
                name: "GtkElement".to_string(),
                args: vec![
                    Value::Text("object".to_string()),
                    Value::List(Arc::new(vec![
                        Value::Constructor {
                            name: "GtkStaticAttr".to_string(),
                            args: vec![
                                Value::Text("class".to_string()),
                                Value::Text("GtkEntry".to_string()),
                            ],
                        },
                        Value::Constructor {
                            name: "GtkIdAttr".to_string(),
                            args: vec![Value::Text(entry_name)],
                        },
                        Value::Constructor {
                            name: "GtkBoundProp".to_string(),
                            args: vec![
                                Value::Text("text".to_string()),
                                Value::Int(runtime.ctx.capture_gtk_binding(
                                    shared_text_for_template.clone(),
                                )),
                            ],
                        },
                    ])),
                    Value::List(Arc::new(vec![])),
                ],
            })
        });
        let node = ResolvedGtkNode::Element {
            tag: "object".to_string(),
            attrs: vec![
                ResolvedGtkAttr::StaticAttr {
                    name: "class".to_string(),
                    value: "GtkBox".to_string(),
                },
                ResolvedGtkAttr::Id("root-dynamic".to_string()),
            ],
            children: vec![ResolvedGtkNode::Each {
                items: items.clone(),
                template,
                _key: None,
            }],
        };

        let _result = ok_or_panic(
            materialize_with_bindings(&node, &mut runtime),
            "build dynamic each",
        );
        let first_entry_id =
            aivi_gtk4::widget_by_id("entry-1").unwrap_or_else(|err| panic!("lookup entry-1: {}", err.message));
        assert_eq!(
            aivi_gtk4::entry_text(first_entry_id)
                .unwrap_or_else(|err| panic!("read entry-1: {}", err.message)),
            "before"
        );

        ok_or_panic(
            runtime.reactive_set_signal(
                items,
                Value::List(Arc::new(vec![Value::Int(1), Value::Int(2)])),
            ),
            "grow items",
        );
        let second_entry_id =
            aivi_gtk4::widget_by_id("entry-2").unwrap_or_else(|err| panic!("lookup entry-2: {}", err.message));
        assert_eq!(
            aivi_gtk4::entry_text(second_entry_id)
                .unwrap_or_else(|err| panic!("read entry-2: {}", err.message)),
            "before"
        );

        ok_or_panic(
            runtime.reactive_set_signal(shared_text, Value::Text("after".to_string())),
            "update shared text",
        );
        assert_eq!(
            aivi_gtk4::entry_text(first_entry_id)
                .unwrap_or_else(|err| panic!("read updated entry-1: {}", err.message)),
            "after"
        );
        assert_eq!(
            aivi_gtk4::entry_text(second_entry_id)
                .unwrap_or_else(|err| panic!("read updated entry-2: {}", err.message)),
            "after"
        );
    }

    #[test]
    fn live_show_binding_reconciles_non_element_children() {
        let _gtk = gtk_test_guard();
        ensure_gtk();
        let ctx = test_ctx();
        let mut runtime = Runtime::new(ctx, CancelToken::root());
        let visible = ok_or_panic(
            runtime.reactive_create_signal(Value::Bool(false)),
            "create visible signal",
        );
        let items = Value::List(Arc::new(vec![Value::Int(1), Value::Int(2)]));
        let template = builtin("test.showEachTemplate", 1, |mut args, _runtime| {
            let item = args.remove(0);
            Ok(Value::Constructor {
                name: "GtkElement".to_string(),
                args: vec![
                    Value::Text("object".to_string()),
                    Value::List(Arc::new(vec![Value::Constructor {
                        name: "GtkStaticAttr".to_string(),
                        args: vec![
                            Value::Text("class".to_string()),
                            Value::Text("GtkEntry".to_string()),
                        ],
                    }])),
                    Value::List(Arc::new(vec![Value::Constructor {
                        name: "GtkElement".to_string(),
                        args: vec![
                            Value::Text("property".to_string()),
                            Value::List(Arc::new(vec![Value::Constructor {
                                name: "GtkStaticAttr".to_string(),
                                args: vec![
                                    Value::Text("name".to_string()),
                                    Value::Text("text".to_string()),
                                ],
                            }])),
                            Value::List(Arc::new(vec![Value::Constructor {
                                name: "GtkTextNode".to_string(),
                                args: vec![item],
                            }])),
                        ],
                    }])),
                ],
            })
        });
        let node = ResolvedGtkNode::Element {
            tag: "object".to_string(),
            attrs: vec![
                ResolvedGtkAttr::StaticAttr {
                    name: "class".to_string(),
                    value: "GtkBox".to_string(),
                },
                ResolvedGtkAttr::Id("root-show-structural".to_string()),
            ],
            children: vec![ResolvedGtkNode::Show {
                when: visible.clone(),
                child: Box::new(ResolvedGtkNode::Each {
                    items,
                    template,
                    _key: None,
                }),
            }],
        };

        let result = ok_or_panic(
            materialize_with_bindings(&node, &mut runtime),
            "build structural show",
        );
        let root_id = *result
            .named_widgets
            .get("root-show-structural")
            .expect("root show widget should be named");
        assert_eq!(
            aivi_gtk4::widget_child_count(root_id)
                .unwrap_or_else(|err| panic!("read initial show child count: {}", err.message)),
            0
        );

        ok_or_panic(
            runtime.reactive_set_signal(visible, Value::Bool(true)),
            "show structural children",
        );
        assert_eq!(
            aivi_gtk4::widget_child_count(root_id)
                .unwrap_or_else(|err| panic!("read shown child count: {}", err.message)),
            2
        );
    }

    #[test]
    fn live_each_binding_preserves_keyed_widget_identity_on_reorder() {
        let _gtk = gtk_test_guard();
        ensure_gtk();
        let ctx = test_ctx();
        let mut runtime = Runtime::new(ctx, CancelToken::root());
        let items = ok_or_panic(
            runtime.reactive_create_signal(Value::List(Arc::new(vec![Value::Int(1), Value::Int(2)]))),
            "create keyed items signal",
        );
        let template = builtin("test.keyedEachTemplate", 1, |mut args, _runtime| {
            let item = args.remove(0);
            let entry_name = match item {
                Value::Int(value) => format!("keyed-entry-{value}"),
                other => panic!("expected Int item, got {other:?}"),
            };
            Ok(Value::Constructor {
                name: "GtkElement".to_string(),
                args: vec![
                    Value::Text("object".to_string()),
                    Value::List(Arc::new(vec![
                        Value::Constructor {
                            name: "GtkStaticAttr".to_string(),
                            args: vec![
                                Value::Text("class".to_string()),
                                Value::Text("GtkEntry".to_string()),
                            ],
                        },
                        Value::Constructor {
                            name: "GtkIdAttr".to_string(),
                            args: vec![Value::Text(entry_name)],
                        },
                    ])),
                    Value::List(Arc::new(vec![])),
                ],
            })
        });
        let key_fn = builtin("test.keyedEachKey", 1, |mut args, _runtime| {
            let item = args.remove(0);
            match item {
                Value::Int(value) => Ok(Value::Text(value.to_string())),
                other => panic!("expected Int item, got {other:?}"),
            }
        });
        let node = ResolvedGtkNode::Element {
            tag: "object".to_string(),
            attrs: vec![
                ResolvedGtkAttr::StaticAttr {
                    name: "class".to_string(),
                    value: "GtkBox".to_string(),
                },
                ResolvedGtkAttr::Id("root-keyed".to_string()),
            ],
            children: vec![ResolvedGtkNode::Each {
                items: items.clone(),
                template,
                _key: Some(key_fn),
            }],
        };

        let result = ok_or_panic(materialize_with_bindings(&node, &mut runtime), "build keyed each");
        let root_id = *result
            .named_widgets
            .get("root-keyed")
            .expect("root keyed widget should be named");
        let first_entry_id = aivi_gtk4::widget_by_id("keyed-entry-1")
            .unwrap_or_else(|err| panic!("lookup keyed-entry-1: {}", err.message));
        let second_entry_id = aivi_gtk4::widget_by_id("keyed-entry-2")
            .unwrap_or_else(|err| panic!("lookup keyed-entry-2: {}", err.message));

        ok_or_panic(
            runtime.reactive_set_signal(items, Value::List(Arc::new(vec![Value::Int(2), Value::Int(1)]))),
            "reorder keyed items",
        );

        assert_eq!(
            aivi_gtk4::widget_by_id("keyed-entry-1")
                .unwrap_or_else(|err| panic!("lookup reordered keyed-entry-1: {}", err.message)),
            first_entry_id
        );
        assert_eq!(
            aivi_gtk4::widget_by_id("keyed-entry-2")
                .unwrap_or_else(|err| panic!("lookup reordered keyed-entry-2: {}", err.message)),
            second_entry_id
        );
        assert_eq!(
            aivi_gtk4::widget_child_ids(root_id)
                .unwrap_or_else(|err| panic!("read keyed child order: {}", err.message)),
            vec![second_entry_id, first_entry_id]
        );
    }

    #[test]
    fn live_each_binding_cleans_removed_widget_watchers() {
        let _gtk = gtk_test_guard();
        ensure_gtk();
        let ctx = test_ctx();
        let mut runtime = Runtime::new(ctx.clone(), CancelToken::root());
        let items = ok_or_panic(
            runtime.reactive_create_signal(Value::List(Arc::new(vec![Value::Int(1), Value::Int(2)]))),
            "create items signal",
        );
        let shared_text = ok_or_panic(
            runtime.reactive_create_signal(Value::Text("before".to_string())),
            "create shared text signal",
        );
        let shared_text_for_template = shared_text.clone();
        let template = builtin("test.cleanupEachTemplate", 1, move |mut args, runtime| {
            let item = args.remove(0);
            let entry_name = match item {
                Value::Int(value) => format!("cleanup-entry-{value}"),
                other => panic!("expected Int item, got {other:?}"),
            };
            Ok(Value::Constructor {
                name: "GtkElement".to_string(),
                args: vec![
                    Value::Text("object".to_string()),
                    Value::List(Arc::new(vec![
                        Value::Constructor {
                            name: "GtkStaticAttr".to_string(),
                            args: vec![
                                Value::Text("class".to_string()),
                                Value::Text("GtkEntry".to_string()),
                            ],
                        },
                        Value::Constructor {
                            name: "GtkIdAttr".to_string(),
                            args: vec![Value::Text(entry_name)],
                        },
                        Value::Constructor {
                            name: "GtkBoundProp".to_string(),
                            args: vec![
                                Value::Text("text".to_string()),
                                Value::Int(runtime.ctx.capture_gtk_binding(
                                    shared_text_for_template.clone(),
                                )),
                            ],
                        },
                    ])),
                    Value::List(Arc::new(vec![])),
                ],
            })
        });
        let key_fn = builtin("test.cleanupEachKey", 1, |mut args, _runtime| {
            let item = args.remove(0);
            match item {
                Value::Int(value) => Ok(Value::Text(value.to_string())),
                other => panic!("expected Int item, got {other:?}"),
            }
        });
        let node = ResolvedGtkNode::Element {
            tag: "object".to_string(),
            attrs: vec![
                ResolvedGtkAttr::StaticAttr {
                    name: "class".to_string(),
                    value: "GtkBox".to_string(),
                },
                ResolvedGtkAttr::Id("root-cleanup".to_string()),
            ],
            children: vec![ResolvedGtkNode::Each {
                items: items.clone(),
                template,
                _key: Some(key_fn),
            }],
        };

        let _result = ok_or_panic(materialize_with_bindings(&node, &mut runtime), "build cleanup each");
        let removed_entry_id = aivi_gtk4::widget_by_id("cleanup-entry-2")
            .unwrap_or_else(|err| panic!("lookup cleanup-entry-2: {}", err.message));

        ok_or_panic(
            runtime.reactive_set_signal(items, Value::List(Arc::new(vec![Value::Int(1)]))),
            "shrink keyed items",
        );
        assert!(
            !aivi_gtk4::widget_exists(removed_entry_id)
                .unwrap_or_else(|err| panic!("check removed widget exists: {}", err.message))
        );
        assert!(
            ctx.take_gtk_binding_watchers(removed_entry_id).is_empty(),
            "removed widget watchers should be disposed"
        );

        ok_or_panic(
            runtime.reactive_set_signal(shared_text, Value::Text("after".to_string())),
            "update remaining shared text",
        );
        let kept_entry_id = aivi_gtk4::widget_by_id("cleanup-entry-1")
            .unwrap_or_else(|err| panic!("lookup cleanup-entry-1: {}", err.message));
        assert_eq!(
            aivi_gtk4::entry_text(kept_entry_id)
                .unwrap_or_else(|err| panic!("read cleanup-entry-1: {}", err.message)),
            "after"
        );
    }

    #[test]
    fn dialog_close_cleans_up_root_binding_watchers() {
        let _gtk = gtk_test_guard();
        ensure_gtk();
        let ctx = test_ctx();
        let mut runtime = Runtime::new(ctx.clone(), CancelToken::root());
        let shared_text = ok_or_panic(
            runtime.reactive_create_signal(Value::Text("dialog".to_string())),
            "create dialog text signal",
        );
        let node = ResolvedGtkNode::Element {
            tag: "object".to_string(),
            attrs: vec![
                ResolvedGtkAttr::StaticAttr {
                    name: "class".to_string(),
                    value: "AdwPreferencesDialog".to_string(),
                },
                ResolvedGtkAttr::Id("cleanup-dialog".to_string()),
                ResolvedGtkAttr::StaticAttr {
                    name: "title".to_string(),
                    value: "Cleanup".to_string(),
                },
            ],
            children: vec![ResolvedGtkNode::Element {
                tag: "object".to_string(),
                attrs: vec![
                    ResolvedGtkAttr::StaticAttr {
                        name: "class".to_string(),
                        value: "AdwPreferencesPage".to_string(),
                    },
                    ResolvedGtkAttr::StaticAttr {
                        name: "title".to_string(),
                        value: "General".to_string(),
                    },
                ],
                children: vec![ResolvedGtkNode::Element {
                    tag: "object".to_string(),
                    attrs: vec![
                        ResolvedGtkAttr::StaticAttr {
                            name: "class".to_string(),
                            value: "AdwPreferencesGroup".to_string(),
                        },
                        ResolvedGtkAttr::StaticAttr {
                            name: "title".to_string(),
                            value: "State".to_string(),
                        },
                    ],
                    children: vec![ResolvedGtkNode::Element {
                        tag: "object".to_string(),
                        attrs: vec![
                            ResolvedGtkAttr::StaticAttr {
                                name: "class".to_string(),
                                value: "GtkEntry".to_string(),
                            },
                            ResolvedGtkAttr::Id("cleanup-dialog-entry".to_string()),
                            ResolvedGtkAttr::BoundProp {
                                name: "text".to_string(),
                                value: shared_text.clone(),
                            },
                        ],
                        children: Vec::new(),
                    }],
                }],
            }],
        };

        let result = ok_or_panic(
            materialize_with_bindings(&node, &mut runtime),
            "build cleanup dialog",
        );
        let entry_id = *result
            .named_widgets
            .get("cleanup-dialog-entry")
            .expect("cleanup dialog entry should be named");
        assert_eq!(
            ui_debug_list_signals_json(ctx.as_ref())
                .expect("list dialog signals")["watcherCount"]
                .as_u64(),
            Some(1)
        );

        let app = aivi_gtk4::app_new("com.aivi.dialog.cleanup.test")
            .unwrap_or_else(|err| panic!("create app: {}", err.message));
        let win = aivi_gtk4::window_new(app, "Host", 480, 320)
            .unwrap_or_else(|err| panic!("create host window: {}", err.message));
        aivi_gtk4::window_present(win)
            .unwrap_or_else(|err| panic!("present host window: {}", err.message));
        aivi_gtk4::adw_dialog_present(result.root_id, win)
            .unwrap_or_else(|err| panic!("present cleanup dialog: {}", err.message));
        super::pump_gtk_events();

        aivi_gtk4::adw_dialog_close(result.root_id)
            .unwrap_or_else(|err| panic!("close cleanup dialog: {}", err.message));
        for _ in 0..50 {
            super::pump_gtk_events();
            std::thread::sleep(Duration::from_millis(10));
            if ui_debug_list_signals_json(ctx.as_ref())
                .expect("list signals after dialog close")["watcherCount"]
                .as_u64()
                == Some(0)
            {
                break;
            }
        }

        assert_eq!(
            ui_debug_list_signals_json(ctx.as_ref())
                .expect("final dialog signals")["watcherCount"]
                .as_u64(),
            Some(0)
        );
        assert!(
            ctx.take_gtk_binding_watchers(entry_id).is_empty(),
            "dialog entry watchers should be disposed on close"
        );
        assert!(
            !aivi_gtk4::widget_exists(result.root_id)
                .unwrap_or_else(|err| panic!("check cleanup dialog root: {}", err.message))
        );
        assert!(
            !aivi_gtk4::widget_exists(entry_id)
                .unwrap_or_else(|err| panic!("check cleanup dialog entry: {}", err.message))
        );
    }

    #[test]
    fn dialog_open_binding_reopens_without_cleanup() {
        let _gtk = gtk_test_guard();
        ensure_gtk();
        let ctx = test_ctx();
        let mut runtime = Runtime::new(ctx.clone(), CancelToken::root());
        let dialog_open = ok_or_panic(
            runtime.reactive_create_signal(Value::Bool(false)),
            "create dialog open signal",
        );
        let shared_text = ok_or_panic(
            runtime.reactive_create_signal(Value::Text("dialog".to_string())),
            "create dialog text signal",
        );

        let app = aivi_gtk4::app_new("com.aivi.dialog.open.binding.test")
            .unwrap_or_else(|err| panic!("create app: {}", err.message));
        let win = aivi_gtk4::window_new(app, "Host", 480, 320)
            .unwrap_or_else(|err| panic!("create host window: {}", err.message));
        aivi_gtk4::window_present(win)
            .unwrap_or_else(|err| panic!("present host window: {}", err.message));

        let node = ResolvedGtkNode::Element {
            tag: "object".to_string(),
            attrs: vec![
                ResolvedGtkAttr::StaticAttr {
                    name: "class".to_string(),
                    value: "AdwPreferencesDialog".to_string(),
                },
                ResolvedGtkAttr::Id("persistent-dialog".to_string()),
                ResolvedGtkAttr::StaticProp {
                    name: "present-for".to_string(),
                    value: win.to_string(),
                },
                ResolvedGtkAttr::BoundProp {
                    name: "open".to_string(),
                    value: dialog_open.clone(),
                },
            ],
            children: vec![ResolvedGtkNode::Element {
                tag: "object".to_string(),
                attrs: vec![ResolvedGtkAttr::StaticAttr {
                    name: "class".to_string(),
                    value: "AdwPreferencesPage".to_string(),
                }],
                children: vec![ResolvedGtkNode::Element {
                    tag: "object".to_string(),
                    attrs: vec![ResolvedGtkAttr::StaticAttr {
                        name: "class".to_string(),
                        value: "AdwPreferencesGroup".to_string(),
                    }],
                    children: vec![ResolvedGtkNode::Element {
                        tag: "object".to_string(),
                        attrs: vec![
                            ResolvedGtkAttr::StaticAttr {
                                name: "class".to_string(),
                                value: "GtkEntry".to_string(),
                            },
                            ResolvedGtkAttr::Id("persistent-dialog-entry".to_string()),
                            ResolvedGtkAttr::BoundProp {
                                name: "text".to_string(),
                                value: shared_text.clone(),
                            },
                        ],
                        children: Vec::new(),
                    }],
                }],
            }],
        };

        let result = ok_or_panic(
            materialize_with_bindings(&node, &mut runtime),
            "build persistent dialog",
        );
        let entry_id = *result
            .named_widgets
            .get("persistent-dialog-entry")
            .expect("persistent dialog entry should be named");
        assert_eq!(
            ui_debug_list_signals_json(ctx.as_ref())
                .expect("list persistent dialog signals")["watcherCount"]
                .as_u64(),
            Some(2)
        );

        ok_or_panic(
            runtime.reactive_set_signal(dialog_open.clone(), Value::Bool(true)),
            "open persistent dialog",
        );
        for _ in 0..60 {
            super::pump_gtk_events();
            std::thread::sleep(Duration::from_millis(10));
        }

        ok_or_panic(
            runtime.reactive_set_signal(dialog_open.clone(), Value::Bool(false)),
            "close persistent dialog via open binding",
        );
        for _ in 0..60 {
            super::pump_gtk_events();
            std::thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(
            ui_debug_list_signals_json(ctx.as_ref())
                .expect("list persistent dialog signals after close")["watcherCount"]
                .as_u64(),
            Some(2)
        );
        assert!(
            aivi_gtk4::widget_exists(result.root_id)
                .unwrap_or_else(|err| panic!("check persistent dialog root: {}", err.message))
        );
        assert!(
            aivi_gtk4::widget_exists(entry_id)
                .unwrap_or_else(|err| panic!("check persistent dialog entry: {}", err.message))
        );

        ok_or_panic(
            runtime.reactive_set_signal(dialog_open.clone(), Value::Bool(true)),
            "reopen persistent dialog",
        );
        for _ in 0..20 {
            super::pump_gtk_events();
            std::thread::sleep(Duration::from_millis(10));
        }
        ok_or_panic(
            runtime.reactive_set_signal(dialog_open, Value::Bool(false)),
            "close reopened persistent dialog",
        );
        for _ in 0..20 {
            super::pump_gtk_events();
            std::thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(
            ui_debug_list_signals_json(ctx.as_ref())
                .expect("list persistent dialog signals after reopen")["watcherCount"]
                .as_u64(),
            Some(2)
        );
        ok_or_panic(
            runtime.reactive_set_signal(shared_text, Value::Text("after".to_string())),
            "update persistent dialog text",
        );
        assert_eq!(
            aivi_gtk4::entry_text(entry_id)
                .unwrap_or_else(|err| panic!("read persistent dialog entry: {}", err.message)),
            "after"
        );
    }
}

/// Drives one iteration of the GTK/GLib main context from any call site.
#[cfg(all(feature = "gtk4-libadwaita", target_os = "linux"))]
pub(super) fn pump_gtk_events() {
    aivi_gtk4::pump_events();
}

#[cfg(not(all(feature = "gtk4-libadwaita", target_os = "linux")))]
pub(super) fn pump_gtk_events() {}

/// Returns true when a GTK application is active and events need pumping.
#[cfg(all(feature = "gtk4-libadwaita", target_os = "linux"))]
pub(crate) fn is_gtk_pump_active() -> bool {
    aivi_gtk4::is_pump_active()
}

#[cfg(not(all(feature = "gtk4-libadwaita", target_os = "linux")))]
pub(crate) fn is_gtk_pump_active() -> bool {
    false
}

pub(super) fn build_gtk4_record_real(build_mock: fn() -> Value) -> Option<Value> {
    #[cfg(all(feature = "gtk4-libadwaita", target_os = "linux"))]
    {
        let Value::Record(existing) = build_mock() else {
            return None;
        };
        let fields = bridge::build_from_mock((*existing).clone());
        return Some(Value::Record(std::sync::Arc::new(fields)));
    }

    #[allow(unreachable_code)]
    {
        let _ = build_mock;
        None
    }
}
