use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use super::util::builtin;
use crate::{EffectValue, RuntimeError, Value};

thread_local! {
    static GTK4_STATE: std::cell::RefCell<Gtk4State> = std::cell::RefCell::new(Gtk4State::default());
}

#[derive(Default)]
struct Gtk4State {
    next_id: i64,
    apps: HashMap<i64, String>,
    windows: HashMap<i64, WindowState>,
    widgets: HashSet<i64>,
    boxes: HashMap<i64, BoxState>,
    overlays: HashMap<i64, OverlayState>,
    list_boxes: HashMap<i64, Vec<i64>>,
    buttons: HashMap<i64, String>,
    labels: HashMap<i64, String>,
    entries: HashMap<i64, String>,
    scroll_areas: HashMap<i64, ScrollAreaState>,
    draw_areas: HashMap<i64, DrawAreaState>,
    widget_css: HashMap<i64, Value>,
    app_css: HashMap<i64, Value>,
    tray_icons: HashMap<i64, TrayIconState>,
    drag_sources: HashMap<i64, DragSourceState>,
    drop_targets: HashMap<i64, DropTargetState>,
    menu_models: HashMap<i64, MenuModelState>,
    menu_buttons: HashMap<i64, MenuButtonState>,
    dialogs: HashMap<i64, DialogState>,
    file_dialogs: HashSet<i64>,
    images: HashMap<i64, String>,
    list_stores: HashMap<i64, Vec<String>>,
    list_views: HashMap<i64, i64>,
    tree_views: HashMap<i64, i64>,
    gesture_clicks: HashMap<i64, GestureClickState>,
    clipboards: HashSet<i64>,
    clipboard_text: String,
    actions: HashMap<i64, ActionState>,
    app_actions: HashMap<i64, Vec<i64>>,
    shortcuts: HashMap<i64, ShortcutState>,
    notifications: HashMap<i64, NotificationState>,
    app_notifications: HashMap<i64, HashMap<String, i64>>,
    layout_managers: HashMap<i64, LayoutManagerState>,
    widget_layout_manager: HashMap<i64, i64>,
    badge_count: HashMap<i64, i64>,
    last_opened_uri: Option<String>,
    last_revealed_path: Option<String>,
    theme_preference: String,
    widget_controllers: HashMap<i64, Vec<i64>>,
    widget_shortcuts: HashMap<i64, Vec<i64>>,
    signal_events: VecDeque<SignalEventState>,
    widget_signal_handlers: HashMap<i64, Vec<SignalBindingState>>,
}

#[derive(Clone)]
struct WindowState {
    app_id: i64,
    title: String,
    width: i64,
    height: i64,
    titlebar: Option<i64>,
    child: Option<i64>,
    visible: bool,
}

#[derive(Clone)]
struct BoxState {
    orientation: i64,
    spacing: i64,
    children: Vec<i64>,
}

#[derive(Clone)]
struct ScrollAreaState {
    child: Option<i64>,
}

#[derive(Clone)]
struct OverlayState {
    child: Option<i64>,
    overlays: Vec<i64>,
}

#[derive(Clone)]
struct DrawAreaState {
    width: i64,
    height: i64,
    dirty: bool,
}

#[derive(Clone)]
struct TrayIconState {
    icon_name: String,
    tooltip: String,
    visible: bool,
}

#[derive(Clone)]
struct DragSourceState {
    widget_id: i64,
    text: String,
}

#[derive(Clone)]
struct DropTargetState {
    widget_id: i64,
    last_text: String,
}

#[derive(Clone)]
struct MenuModelState {
    items: Vec<(String, String)>,
}

#[derive(Clone)]
struct MenuButtonState {
    label: String,
    menu_model: Option<i64>,
}

#[derive(Clone)]
struct DialogState {
    app_id: i64,
    title: String,
    child: Option<i64>,
    visible: bool,
}

#[derive(Clone)]
struct GestureClickState {
    widget_id: i64,
    last_button: i64,
}

#[derive(Clone)]
struct ActionState {
    name: String,
    enabled: bool,
}

#[derive(Clone)]
struct ShortcutState {
    trigger: String,
    action_name: String,
}

#[derive(Clone)]
struct NotificationState {
    title: String,
    body: String,
}

#[derive(Clone)]
struct LayoutManagerState {
    kind: String,
}

#[derive(Clone)]
struct SignalBindingState {
    signal: String,
    handler: String,
}

#[derive(Clone)]
struct SignalEventState {
    widget_id: i64,
    signal: String,
    handler: String,
    payload: String,
}

impl Gtk4State {
    fn alloc_id(&mut self) -> i64 {
        self.next_id += 1;
        self.next_id
    }

    fn alloc_widget_id(&mut self) -> i64 {
        let id = self.alloc_id();
        self.widgets.insert(id);
        id
    }

    fn ensure_widget(&self, id: i64, ctx: &str) -> Result<(), RuntimeError> {
        if self.widgets.contains(&id) {
            Ok(())
        } else {
            Err(RuntimeError::Error(Value::Text(format!(
                "gtk4.{ctx} unknown widget id {id}"
            ))))
        }
    }
}

fn effect<F>(f: F) -> Value
where
    F: Fn(&mut crate::Runtime) -> Result<Value, RuntimeError> + Send + Sync + 'static,
{
    Value::Effect(Arc::new(EffectValue::Thunk { func: Arc::new(f) }))
}

fn invalid(name: &str) -> RuntimeError {
    RuntimeError::Message(name.to_string())
}

#[derive(Debug, Clone)]
enum GtkBuilderNode {
    Element {
        tag: String,
        attrs: Vec<(String, String)>,
        children: Vec<GtkBuilderNode>,
    },
    Text(String),
}

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

fn decode_gtk_attr(value: &Value) -> Result<(String, String), RuntimeError> {
    let Value::Constructor { name, args } = value else {
        return Err(invalid("gtk4.buildFromNode expects GtkAttribute values"));
    };
    if name != "GtkAttribute" || args.len() != 2 {
        return Err(invalid("gtk4.buildFromNode expects GtkAttribute values"));
    }
    let key =
        decode_text(&args[0]).ok_or_else(|| invalid("gtk4.buildFromNode invalid attr name"))?;
    let val =
        decode_text(&args[1]).ok_or_else(|| invalid("gtk4.buildFromNode invalid attr value"))?;
    Ok((key, val))
}

fn decode_gtk_node(value: &Value) -> Result<GtkBuilderNode, RuntimeError> {
    let Value::Constructor { name, args } = value else {
        return Err(invalid("gtk4.buildFromNode expects GtkNode"));
    };
    match (name.as_str(), args.len()) {
        ("GtkTextNode", 1) => {
            let text = decode_text(&args[0])
                .ok_or_else(|| invalid("gtk4.buildFromNode invalid GtkTextNode text"))?;
            Ok(GtkBuilderNode::Text(text))
        }
        ("GtkElement", 3) => {
            let tag = decode_text(&args[0])
                .ok_or_else(|| invalid("gtk4.buildFromNode invalid GtkElement tag"))?;
            let Value::List(attrs) = &args[1] else {
                return Err(invalid("gtk4.buildFromNode GtkElement attrs must be List"));
            };
            let Value::List(children) = &args[2] else {
                return Err(invalid(
                    "gtk4.buildFromNode GtkElement children must be List",
                ));
            };
            let attrs = attrs
                .iter()
                .map(decode_gtk_attr)
                .collect::<Result<Vec<_>, _>>()?;
            let children = children
                .iter()
                .map(decode_gtk_node)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(GtkBuilderNode::Element {
                tag,
                attrs,
                children,
            })
        }
        _ => Err(invalid("gtk4.buildFromNode expects GtkNode")),
    }
}

fn parse_i64_text(text: &str) -> Option<i64> {
    text.trim().parse::<i64>().ok()
}

fn parse_usize_text(text: &str) -> Option<usize> {
    text.trim().parse::<usize>().ok()
}

fn parse_bool_text(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_policy_text(text: &str) -> Option<i64> {
    match text.trim().to_ascii_lowercase().as_str() {
        "always" => Some(0),
        "automatic" => Some(1),
        "never" => Some(2),
        "external" => Some(3),
        other => other.parse::<i64>().ok(),
    }
}

fn parse_orientation_text(text: &str) -> i64 {
    let normalized = text.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "vertical" | "1" => 1,
        _ => 0,
    }
}

fn node_attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find_map(|(key, value)| (key == name).then_some(value.as_str()))
}

fn collect_text(children: &[GtkBuilderNode]) -> String {
    let mut out = String::new();
    for child in children {
        if let GtkBuilderNode::Text(text) = child {
            out.push_str(text);
        }
    }
    out.trim().to_string()
}

fn collect_object_properties(
    attrs: &[(String, String)],
    children: &[GtkBuilderNode],
) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (name, value) in attrs {
        if let Some(prop) = name.strip_prefix("prop:") {
            out.insert(prop.to_string(), value.clone());
        }
    }
    for child in children {
        let GtkBuilderNode::Element {
            tag,
            attrs,
            children,
        } = child
        else {
            continue;
        };
        if tag == "property" {
            if let Some(name) = node_attr(attrs, "name") {
                out.insert(name.to_string(), collect_text(children));
            }
            continue;
        }
        if tag == "style" {
            for style_child in children {
                let GtkBuilderNode::Element {
                    tag,
                    attrs,
                    children: _,
                } = style_child
                else {
                    continue;
                };
                if tag == "class" {
                    if let Some(class_name) = node_attr(attrs, "name") {
                        let current = out.remove("css-class").unwrap_or_default();
                        let joined = if current.is_empty() {
                            class_name.to_string()
                        } else {
                            format!("{current} {class_name}")
                        };
                        out.insert("css-class".to_string(), joined);
                    }
                }
            }
        }
    }
    out
}

fn collect_object_signals(
    attrs: &[(String, String)],
    children: &[GtkBuilderNode],
) -> Vec<SignalBindingState> {
    let mut out = Vec::new();
    for (name, value) in attrs {
        if let Some(signal) = name.strip_prefix("signal:") {
            out.push(SignalBindingState {
                signal: signal.to_string(),
                handler: value.clone(),
            });
        }
    }
    for child in children {
        let GtkBuilderNode::Element {
            tag,
            attrs,
            children: _,
        } = child
        else {
            continue;
        };
        if tag != "signal" {
            continue;
        }
        let Some(signal) = node_attr(attrs, "name") else {
            continue;
        };
        let Some(handler) = node_attr(attrs, "handler").or_else(|| node_attr(attrs, "on")) else {
            continue;
        };
        out.push(SignalBindingState {
            signal: signal.to_string(),
            handler: handler.to_string(),
        });
    }
    out
}

fn make_signal_event_value(event: SignalEventState) -> Value {
    let payload = Value::Constructor {
        name: "GtkSignalEvent".to_string(),
        args: vec![
            Value::Int(event.widget_id),
            Value::Text(event.signal),
            Value::Text(event.handler),
            Value::Text(event.payload),
        ],
    };
    Value::Constructor {
        name: "Some".to_string(),
        args: vec![payload],
    }
}

struct ChildSpec<'a> {
    node: &'a GtkBuilderNode,
    child_type: Option<String>,
    position: Option<usize>,
}

fn child_packing_position(children: &[GtkBuilderNode]) -> Option<usize> {
    for child in children {
        let GtkBuilderNode::Element {
            tag,
            attrs: _,
            children,
        } = child
        else {
            continue;
        };
        if tag != "packing" {
            continue;
        }
        for packing_child in children {
            let GtkBuilderNode::Element {
                tag,
                attrs,
                children,
            } = packing_child
            else {
                continue;
            };
            if tag == "property" && node_attr(attrs, "name") == Some("position") {
                return parse_usize_text(&collect_text(children));
            }
        }
    }
    None
}

fn collect_child_objects(children: &[GtkBuilderNode]) -> Vec<ChildSpec<'_>> {
    let mut out = Vec::new();
    for child in children {
        let GtkBuilderNode::Element {
            tag,
            attrs,
            children,
        } = child
        else {
            continue;
        };
        if tag == "child" {
            let child_type = node_attr(attrs, "type").map(str::to_string);
            let position = node_attr(attrs, "position")
                .and_then(parse_usize_text)
                .or_else(|| child_packing_position(children));
            for nested in children {
                if matches!(
                    nested,
                    GtkBuilderNode::Element {
                        tag,
                        attrs: _,
                        children: _
                    } if tag == "object"
                ) {
                    out.push(ChildSpec {
                        node: nested,
                        child_type: child_type.clone(),
                        position,
                    });
                }
            }
        } else if tag == "property" && node_attr(attrs, "name") == Some("child") {
            for nested in children {
                if matches!(
                    nested,
                    GtkBuilderNode::Element {
                        tag,
                        attrs: _,
                        children: _
                    } if tag == "object"
                ) {
                    out.push(ChildSpec {
                        node: nested,
                        child_type: None,
                        position: None,
                    });
                }
            }
        } else if tag == "object" {
            out.push(ChildSpec {
                node: child,
                child_type: None,
                position: None,
            });
        }
    }
    out
}

fn first_object_in_interface(node: &GtkBuilderNode) -> Result<&GtkBuilderNode, RuntimeError> {
    let GtkBuilderNode::Element { tag, children, .. } = node else {
        return Err(invalid("gtk4.buildFromNode expects GtkNode root element"));
    };
    if tag == "object" {
        return Ok(node);
    }
    if tag != "interface" && tag != "template" {
        return Err(invalid(
            "gtk4.buildFromNode root must be <object>, <interface>, or <template>",
        ));
    }
    fn find_first_object(node: &GtkBuilderNode) -> Option<&GtkBuilderNode> {
        let GtkBuilderNode::Element { tag, children, .. } = node else {
            return None;
        };
        if tag == "object" {
            return Some(node);
        }
        for child in children {
            if let Some(found) = find_first_object(child) {
                return Some(found);
            }
        }
        None
    }
    children
        .iter()
        .find_map(find_first_object)
        .ok_or_else(|| invalid("gtk4.buildFromNode root must contain at least one <object>"))
}

fn build_widget_from_node_mock(
    state: &mut Gtk4State,
    node: &GtkBuilderNode,
    id_map: &mut HashMap<String, i64>,
) -> Result<i64, RuntimeError> {
    let GtkBuilderNode::Element {
        tag,
        attrs,
        children,
    } = node
    else {
        return Err(invalid("gtk4.buildFromNode root must be GtkElement"));
    };
    if tag != "object" {
        return Err(invalid("gtk4.buildFromNode root tag must be <object>"));
    }
    if let Some(ref_id) = node_attr(attrs, "idref").or_else(|| node_attr(attrs, "ref")) {
        return id_map.get(ref_id).copied().ok_or_else(|| {
            RuntimeError::Error(Value::Text(format!(
                "gtk4.buildFromNode unresolved object reference id '{ref_id}'"
            )))
        });
    }
    let class = node_attr(attrs, "class")
        .ok_or_else(|| invalid("gtk4.buildFromNode object requires class attribute"))?;
    let props = collect_object_properties(attrs, children);
    let mut signal_bindings = collect_object_signals(attrs, children);
    signal_bindings.retain(|binding| !binding.signal.is_empty() && !binding.handler.is_empty());

    let id = state.alloc_widget_id();
    if let Some(object_id) = node_attr(attrs, "id") {
        id_map.insert(object_id.to_string(), id);
    }
    match class {
        "GtkBox" | "AdwClamp" => {
            let spacing = props
                .get("spacing")
                .and_then(|v| parse_i64_text(v))
                .unwrap_or(0);
            let orientation = props
                .get("orientation")
                .map(|v| parse_orientation_text(v))
                .unwrap_or(0);
            state.boxes.insert(
                id,
                BoxState {
                    orientation,
                    spacing,
                    children: Vec::new(),
                },
            );
        }
        "GtkOverlay" => {
            state.overlays.insert(
                id,
                OverlayState {
                    child: None,
                    overlays: Vec::new(),
                },
            );
        }
        "GtkLabel" => {
            let label = props
                .get("label")
                .or_else(|| props.get("text"))
                .cloned()
                .unwrap_or_default();
            state.labels.insert(id, label);
        }
        "GtkButton" => {
            let label = props.get("label").cloned().unwrap_or_default();
            state.buttons.insert(id, label);
        }
        "GtkEntry" => {
            let text = props.get("text").cloned().unwrap_or_default();
            state.entries.insert(id, text);
        }
        "GtkScrolledWindow" => {
            let _h_policy = props
                .get("hscrollbar-policy")
                .and_then(|v| parse_policy_text(v))
                .unwrap_or(1);
            let _v_policy = props
                .get("vscrollbar-policy")
                .and_then(|v| parse_policy_text(v))
                .unwrap_or(1);
            let _propagate_natural_height = props
                .get("propagate-natural-height")
                .and_then(|v| parse_bool_text(v))
                .unwrap_or(false);
            let _propagate_natural_width = props
                .get("propagate-natural-width")
                .and_then(|v| parse_bool_text(v))
                .unwrap_or(false);
            state
                .scroll_areas
                .insert(id, ScrollAreaState { child: None });
        }
        "GtkDrawingArea" => {
            state.draw_areas.insert(
                id,
                DrawAreaState {
                    width: props
                        .get("width-request")
                        .and_then(|v| parse_i64_text(v))
                        .unwrap_or(0),
                    height: props
                        .get("height-request")
                        .and_then(|v| parse_i64_text(v))
                        .unwrap_or(0),
                    dirty: false,
                },
            );
        }
        "GtkListBox" => {
            state.list_boxes.insert(id, Vec::new());
        }
        "GtkMenuButton" => {
            state.menu_buttons.insert(
                id,
                MenuButtonState {
                    label: props.get("label").cloned().unwrap_or_default(),
                    menu_model: None,
                },
            );
        }
        "GtkGestureClick" => {
            state.gesture_clicks.insert(
                id,
                GestureClickState {
                    widget_id: 0,
                    last_button: props
                        .get("button")
                        .and_then(|v| parse_i64_text(v))
                        .unwrap_or(0),
                },
            );
        }
        "GtkImage" => {
            let src = props
                .get("resource")
                .or_else(|| props.get("file"))
                .or_else(|| props.get("icon-name"))
                .cloned()
                .unwrap_or_default();
            state.images.insert(id, src);
        }
        _ => {}
    }
    if !signal_bindings.is_empty() {
        state.widget_signal_handlers.insert(id, signal_bindings);
    }

    let child_objects = collect_child_objects(children);
    let mut ordered_children = child_objects;
    ordered_children.sort_by_key(|child| child.position.unwrap_or(usize::MAX));
    for child in ordered_children {
        let child_id = build_widget_from_node_mock(state, child.node, id_map)?;
        if child.child_type.as_deref() == Some("controller") {
            state
                .widget_controllers
                .entry(id)
                .or_default()
                .push(child_id);
            if let Some(gesture) = state.gesture_clicks.get_mut(&child_id) {
                gesture.widget_id = id;
            }
            continue;
        }
        if let Some(container) = state.boxes.get_mut(&id) {
            if let Some(position) = child.position {
                let position = position.min(container.children.len());
                container.children.insert(position, child_id);
            } else {
                container.children.push(child_id);
            }
        } else if let Some(list_box) = state.list_boxes.get_mut(&id) {
            list_box.push(child_id);
        } else if let Some(overlay) = state.overlays.get_mut(&id) {
            if child.child_type.as_deref() == Some("overlay") {
                overlay.overlays.push(child_id);
            } else if overlay.child.is_none() {
                overlay.child = Some(child_id);
            } else {
                overlay.overlays.push(child_id);
            }
        } else if let Some(scrolled) = state.scroll_areas.get_mut(&id) {
            if scrolled.child.is_none() {
                scrolled.child = Some(child_id);
            }
        }
    }

    Ok(id)
}

pub(super) fn build_gtk4_record() -> Value {
    if let Some(real) = super::gtk4_real::build_gtk4_record_real(build_gtk4_record_mock) {
        return real;
    }
    build_gtk4_record_mock()
}

fn build_gtk4_record_mock() -> Value {
    let mut fields = HashMap::new();

    fields.insert(
        "init".to_string(),
        builtin("gtk4.init", 1, |_, _| Ok(effect(|_| Ok(Value::Unit)))),
    );

    fields.insert(
        "appNew".to_string(),
        builtin("gtk4.appNew", 1, |mut args, _| {
            let app_id = match args.remove(0) {
                Value::Text(text) => text,
                _ => return Err(invalid("gtk4.appNew expects Text application id")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_id();
                    state.apps.insert(id, app_id.clone());
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "windowNew".to_string(),
        builtin("gtk4.windowNew", 4, |mut args, _| {
            let height = match args.remove(3) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.windowNew expects Int height")),
            };
            let width = match args.remove(2) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.windowNew expects Int width")),
            };
            let title = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.windowNew expects Text title")),
            };
            let app_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.windowNew expects Int app id")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    if !state.apps.contains_key(&app_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.windowNew unknown app id {app_id}"
                        ))));
                    }
                    let id = state.alloc_id();
                    state.windows.insert(
                        id,
                        WindowState {
                            app_id,
                            title: title.clone(),
                            width,
                            height,
                            titlebar: None,
                            child: None,
                            visible: false,
                        },
                    );
                    Ok(id)
                })?;
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "windowSetTitle".to_string(),
        builtin("gtk4.windowSetTitle", 2, |mut args, _| {
            let title = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.windowSetTitle expects Text title")),
            };
            let window_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.windowSetTitle expects Int window id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(window) = state.windows.get_mut(&window_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.windowSetTitle unknown window id {window_id}"
                        ))));
                    };
                    window.title = title.clone();
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "windowSetTitlebar".to_string(),
        builtin("gtk4.windowSetTitlebar", 2, |mut args, _| {
            let titlebar_id = match args.remove(1) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.windowSetTitlebar expects Int titlebar id")),
            };
            let window_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.windowSetTitlebar expects Int window id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    state.ensure_widget(titlebar_id, "windowSetTitlebar")?;
                    let Some(window) = state.windows.get_mut(&window_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.windowSetTitlebar unknown window id {window_id}"
                        ))));
                    };
                    window.titlebar = Some(titlebar_id);
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "windowPresent".to_string(),
        builtin("gtk4.windowPresent", 1, |mut args, _| {
            let window_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.windowPresent expects Int window id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let state = state.borrow();
                    let Some(window) = state.windows.get(&window_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.windowPresent unknown window id {window_id}"
                        ))));
                    };
                    let _ = (
                        &window.title,
                        window.width,
                        window.height,
                        window.titlebar,
                        window.app_id,
                        window.visible,
                    );
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "windowSetChild".to_string(),
        builtin("gtk4.windowSetChild", 2, |mut args, _| {
            let child_id = match args.remove(1) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.windowSetChild expects Int child id")),
            };
            let window_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.windowSetChild expects Int window id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    state.ensure_widget(child_id, "windowSetChild")?;
                    let Some(window) = state.windows.get_mut(&window_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.windowSetChild unknown window id {window_id}"
                        ))));
                    };
                    window.child = Some(child_id);
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "appRun".to_string(),
        builtin("gtk4.appRun", 1, |mut args, _| {
            let app_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.appRun expects Int app id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let state = state.borrow();
                    if !state.apps.contains_key(&app_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.appRun unknown app id {app_id}"
                        ))));
                    }
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "widgetShow".to_string(),
        builtin("gtk4.widgetShow", 1, |mut args, _| {
            let widget_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.widgetShow expects Int widget id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let state = state.borrow();
                    state.ensure_widget(widget_id, "widgetShow")?;
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "widgetHide".to_string(),
        builtin("gtk4.widgetHide", 1, |mut args, _| {
            let widget_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.widgetHide expects Int widget id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let state = state.borrow();
                    state.ensure_widget(widget_id, "widgetHide")?;
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "boxNew".to_string(),
        builtin("gtk4.boxNew", 2, |mut args, _| {
            let spacing = match args.remove(1) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.boxNew expects Int spacing")),
            };
            let orientation = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.boxNew expects Int orientation")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_widget_id();
                    state.boxes.insert(
                        id,
                        BoxState {
                            orientation,
                            spacing,
                            children: Vec::new(),
                        },
                    );
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "boxAppend".to_string(),
        builtin("gtk4.boxAppend", 2, |mut args, _| {
            let child_id = match args.remove(1) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.boxAppend expects Int child id")),
            };
            let box_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.boxAppend expects Int box id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    state.ensure_widget(child_id, "boxAppend")?;
                    let Some(container) = state.boxes.get_mut(&box_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.boxAppend unknown box id {box_id}"
                        ))));
                    };
                    let _ = (container.orientation, container.spacing);
                    container.children.push(child_id);
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "buttonNew".to_string(),
        builtin("gtk4.buttonNew", 1, |mut args, _| {
            let label = match args.remove(0) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.buttonNew expects Text label")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_widget_id();
                    state.buttons.insert(id, label.clone());
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "buttonSetLabel".to_string(),
        builtin("gtk4.buttonSetLabel", 2, |mut args, _| {
            let label = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.buttonSetLabel expects Text label")),
            };
            let button_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.buttonSetLabel expects Int button id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(button) = state.buttons.get_mut(&button_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.buttonSetLabel unknown button id {button_id}"
                        ))));
                    };
                    *button = label.clone();
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "labelNew".to_string(),
        builtin("gtk4.labelNew", 1, |mut args, _| {
            let text = match args.remove(0) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.labelNew expects Text")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_widget_id();
                    state.labels.insert(id, text.clone());
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "labelSetText".to_string(),
        builtin("gtk4.labelSetText", 2, |mut args, _| {
            let text = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.labelSetText expects Text")),
            };
            let label_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.labelSetText expects Int label id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(label) = state.labels.get_mut(&label_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.labelSetText unknown label id {label_id}"
                        ))));
                    };
                    *label = text.clone();
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "entryNew".to_string(),
        builtin("gtk4.entryNew", 1, |mut args, _| {
            match args.remove(0) {
                Value::Unit => {}
                _ => return Err(invalid("gtk4.entryNew expects Unit")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_widget_id();
                    state.entries.insert(id, String::new());
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "entrySetText".to_string(),
        builtin("gtk4.entrySetText", 2, |mut args, _| {
            let text = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.entrySetText expects Text")),
            };
            let entry_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.entrySetText expects Int entry id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(entry) = state.entries.get_mut(&entry_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.entrySetText unknown entry id {entry_id}"
                        ))));
                    };
                    *entry = text.clone();
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "entryText".to_string(),
        builtin("gtk4.entryText", 1, |mut args, _| {
            let entry_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.entryText expects Int entry id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let state = state.borrow();
                    let Some(entry) = state.entries.get(&entry_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.entryText unknown entry id {entry_id}"
                        ))));
                    };
                    Ok(Value::Text(entry.clone()))
                })
            }))
        }),
    );

    fields.insert(
        "scrollAreaNew".to_string(),
        builtin("gtk4.scrollAreaNew", 1, |mut args, _| {
            match args.remove(0) {
                Value::Unit => {}
                _ => return Err(invalid("gtk4.scrollAreaNew expects Unit")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_widget_id();
                    state
                        .scroll_areas
                        .insert(id, ScrollAreaState { child: None });
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "scrollAreaSetChild".to_string(),
        builtin("gtk4.scrollAreaSetChild", 2, |mut args, _| {
            let child_id = match args.remove(1) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.scrollAreaSetChild expects Int child id")),
            };
            let scroll_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => {
                    return Err(invalid(
                        "gtk4.scrollAreaSetChild expects Int scroll area id",
                    ))
                }
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    state.ensure_widget(child_id, "scrollAreaSetChild")?;
                    let Some(scroll) = state.scroll_areas.get_mut(&scroll_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.scrollAreaSetChild unknown scroll area id {scroll_id}"
                        ))));
                    };
                    scroll.child = Some(child_id);
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "drawAreaNew".to_string(),
        builtin("gtk4.drawAreaNew", 2, |mut args, _| {
            let height = match args.remove(1) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.drawAreaNew expects Int height")),
            };
            let width = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.drawAreaNew expects Int width")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_widget_id();
                    state.draw_areas.insert(
                        id,
                        DrawAreaState {
                            width,
                            height,
                            dirty: false,
                        },
                    );
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "drawAreaSetContentSize".to_string(),
        builtin("gtk4.drawAreaSetContentSize", 3, |mut args, _| {
            let height = match args.remove(2) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.drawAreaSetContentSize expects Int height")),
            };
            let width = match args.remove(1) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.drawAreaSetContentSize expects Int width")),
            };
            let draw_area_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => {
                    return Err(invalid(
                        "gtk4.drawAreaSetContentSize expects Int draw area id",
                    ))
                }
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(draw_area) = state.draw_areas.get_mut(&draw_area_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.drawAreaSetContentSize unknown draw area id {draw_area_id}"
                        ))));
                    };
                    draw_area.width = width;
                    draw_area.height = height;
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "drawAreaQueueDraw".to_string(),
        builtin("gtk4.drawAreaQueueDraw", 1, |mut args, _| {
            let draw_area_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.drawAreaQueueDraw expects Int draw area id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(draw_area) = state.draw_areas.get_mut(&draw_area_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.drawAreaQueueDraw unknown draw area id {draw_area_id}"
                        ))));
                    };
                    let _ = (draw_area.width, draw_area.height);
                    draw_area.dirty = true;
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "widgetSetCss".to_string(),
        builtin("gtk4.widgetSetCss", 2, |mut args, _| {
            let css = match args.remove(1) {
                Value::Record(v) => Value::Record(v),
                _ => return Err(invalid("gtk4.widgetSetCss expects Record css style")),
            };
            let widget_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.widgetSetCss expects Int widget id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    state.ensure_widget(widget_id, "widgetSetCss")?;
                    state.widget_css.insert(widget_id, css.clone());
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "appSetCss".to_string(),
        builtin("gtk4.appSetCss", 2, |mut args, _| {
            let css = match args.remove(1) {
                Value::Record(v) => Value::Record(v),
                Value::Text(v) => Value::Text(v),
                _ => return Err(invalid("gtk4.appSetCss expects Text css or Record")),
            };
            let app_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.appSetCss expects Int app id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    if !state.apps.contains_key(&app_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.appSetCss unknown app id {app_id}"
                        ))));
                    }
                    state.app_css.insert(app_id, css.clone());
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "iconThemeAddSearchPath".to_string(),
        builtin("gtk4.iconThemeAddSearchPath", 1, |mut args, _| {
            let _path = match args.remove(0) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.iconThemeAddSearchPath expects Text path")),
            };
            Ok(effect(move |_| Ok(Value::Unit)))
        }),
    );

    fields.insert(
        "trayIconNew".to_string(),
        builtin("gtk4.trayIconNew", 2, |mut args, _| {
            let tooltip = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.trayIconNew expects Text tooltip")),
            };
            let icon_name = match args.remove(0) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.trayIconNew expects Text icon name")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_id();
                    state.tray_icons.insert(
                        id,
                        TrayIconState {
                            icon_name: icon_name.clone(),
                            tooltip: tooltip.clone(),
                            visible: true,
                        },
                    );
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "trayIconSetTooltip".to_string(),
        builtin("gtk4.trayIconSetTooltip", 2, |mut args, _| {
            let tooltip = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.trayIconSetTooltip expects Text tooltip")),
            };
            let tray_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.trayIconSetTooltip expects Int tray icon id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(tray) = state.tray_icons.get_mut(&tray_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.trayIconSetTooltip unknown tray icon id {tray_id}"
                        ))));
                    };
                    let _ = &tray.icon_name;
                    tray.tooltip = tooltip.clone();
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "trayIconSetVisible".to_string(),
        builtin("gtk4.trayIconSetVisible", 2, |mut args, _| {
            let visible = match args.remove(1) {
                Value::Bool(v) => v,
                _ => return Err(invalid("gtk4.trayIconSetVisible expects Bool visible")),
            };
            let tray_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.trayIconSetVisible expects Int tray icon id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(tray) = state.tray_icons.get_mut(&tray_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.trayIconSetVisible unknown tray icon id {tray_id}"
                        ))));
                    };
                    tray.visible = visible;
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "dragSourceNew".to_string(),
        builtin("gtk4.dragSourceNew", 1, |mut args, _| {
            let widget_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.dragSourceNew expects Int widget id")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| -> Result<i64, RuntimeError> {
                    let mut state = state.borrow_mut();
                    state.ensure_widget(widget_id, "dragSourceNew")?;
                    let id = state.alloc_id();
                    state.drag_sources.insert(
                        id,
                        DragSourceState {
                            widget_id,
                            text: String::new(),
                        },
                    );
                    Ok(id)
                })?;
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "dragSourceSetText".to_string(),
        builtin("gtk4.dragSourceSetText", 2, |mut args, _| {
            let text = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.dragSourceSetText expects Text")),
            };
            let source_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.dragSourceSetText expects Int source id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(source) = state.drag_sources.get_mut(&source_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.dragSourceSetText unknown source id {source_id}"
                        ))));
                    };
                    let _ = source.widget_id;
                    source.text = text.clone();
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "dropTargetNew".to_string(),
        builtin("gtk4.dropTargetNew", 1, |mut args, _| {
            let widget_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.dropTargetNew expects Int widget id")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| -> Result<i64, RuntimeError> {
                    let mut state = state.borrow_mut();
                    state.ensure_widget(widget_id, "dropTargetNew")?;
                    let id = state.alloc_id();
                    state.drop_targets.insert(
                        id,
                        DropTargetState {
                            widget_id,
                            last_text: String::new(),
                        },
                    );
                    Ok(id)
                })?;
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "dropTargetLastText".to_string(),
        builtin("gtk4.dropTargetLastText", 1, |mut args, _| {
            let target_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.dropTargetLastText expects Int target id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let state = state.borrow();
                    let Some(target) = state.drop_targets.get(&target_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.dropTargetLastText unknown target id {target_id}"
                        ))));
                    };
                    let _ = target.widget_id;
                    Ok(Value::Text(target.last_text.clone()))
                })
            }))
        }),
    );

    fields.insert(
        "menuModelNew".to_string(),
        builtin("gtk4.menuModelNew", 1, |mut args, _| {
            match args.remove(0) {
                Value::Unit => {}
                _ => return Err(invalid("gtk4.menuModelNew expects Unit")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_id();
                    state
                        .menu_models
                        .insert(id, MenuModelState { items: Vec::new() });
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "menuModelAppendItem".to_string(),
        builtin("gtk4.menuModelAppendItem", 3, |mut args, _| {
            let target = match args.remove(2) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.menuModelAppendItem expects Text target")),
            };
            let label = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.menuModelAppendItem expects Text label")),
            };
            let menu_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => {
                    return Err(invalid(
                        "gtk4.menuModelAppendItem expects Int menu model id",
                    ))
                }
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(model) = state.menu_models.get_mut(&menu_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.menuModelAppendItem unknown menu model id {menu_id}"
                        ))));
                    };
                    model.items.push((label.clone(), target.clone()));
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "menuButtonNew".to_string(),
        builtin("gtk4.menuButtonNew", 1, |mut args, _| {
            let label = match args.remove(0) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.menuButtonNew expects Text label")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_widget_id();
                    state.menu_buttons.insert(
                        id,
                        MenuButtonState {
                            label: label.clone(),
                            menu_model: None,
                        },
                    );
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "menuButtonSetMenuModel".to_string(),
        builtin("gtk4.menuButtonSetMenuModel", 2, |mut args, _| {
            let menu_id = match args.remove(1) {
                Value::Int(v) => v,
                _ => {
                    return Err(invalid(
                        "gtk4.menuButtonSetMenuModel expects Int menu model id",
                    ))
                }
            };
            let button_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => {
                    return Err(invalid(
                        "gtk4.menuButtonSetMenuModel expects Int menu button id",
                    ))
                }
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    if !state.menu_models.contains_key(&menu_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.menuButtonSetMenuModel unknown menu model id {menu_id}"
                        ))));
                    }
                    let Some(button) = state.menu_buttons.get_mut(&button_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.menuButtonSetMenuModel unknown menu button id {button_id}"
                        ))));
                    };
                    let _ = &button.label;
                    button.menu_model = Some(menu_id);
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "dialogNew".to_string(),
        builtin("gtk4.dialogNew", 1, |mut args, _| {
            let app_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.dialogNew expects Int app id")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| -> Result<i64, RuntimeError> {
                    let mut state = state.borrow_mut();
                    if !state.apps.contains_key(&app_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.dialogNew unknown app id {app_id}"
                        ))));
                    }
                    let id = state.alloc_id();
                    state.dialogs.insert(
                        id,
                        DialogState {
                            app_id,
                            title: String::new(),
                            child: None,
                            visible: false,
                        },
                    );
                    Ok(id)
                })?;
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "dialogSetTitle".to_string(),
        builtin("gtk4.dialogSetTitle", 2, |mut args, _| {
            let title = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.dialogSetTitle expects Text title")),
            };
            let dialog_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.dialogSetTitle expects Int dialog id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(dialog) = state.dialogs.get_mut(&dialog_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.dialogSetTitle unknown dialog id {dialog_id}"
                        ))));
                    };
                    dialog.title = title.clone();
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "dialogSetChild".to_string(),
        builtin("gtk4.dialogSetChild", 2, |mut args, _| {
            let child_id = match args.remove(1) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.dialogSetChild expects Int child id")),
            };
            let dialog_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.dialogSetChild expects Int dialog id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    state.ensure_widget(child_id, "dialogSetChild")?;
                    let Some(dialog) = state.dialogs.get_mut(&dialog_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.dialogSetChild unknown dialog id {dialog_id}"
                        ))));
                    };
                    dialog.child = Some(child_id);
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "dialogPresent".to_string(),
        builtin("gtk4.dialogPresent", 1, |mut args, _| {
            let dialog_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.dialogPresent expects Int dialog id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(dialog) = state.dialogs.get_mut(&dialog_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.dialogPresent unknown dialog id {dialog_id}"
                        ))));
                    };
                    let _ = dialog.app_id;
                    dialog.visible = true;
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "dialogClose".to_string(),
        builtin("gtk4.dialogClose", 1, |mut args, _| {
            let dialog_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.dialogClose expects Int dialog id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(dialog) = state.dialogs.get_mut(&dialog_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.dialogClose unknown dialog id {dialog_id}"
                        ))));
                    };
                    dialog.visible = false;
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "fileDialogNew".to_string(),
        builtin("gtk4.fileDialogNew", 1, |mut args, _| {
            match args.remove(0) {
                Value::Unit => {}
                _ => return Err(invalid("gtk4.fileDialogNew expects Unit")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_id();
                    state.file_dialogs.insert(id);
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "fileDialogSelectFile".to_string(),
        builtin("gtk4.fileDialogSelectFile", 1, |mut args, _| {
            let dialog_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => {
                    return Err(invalid(
                        "gtk4.fileDialogSelectFile expects Int file dialog id",
                    ))
                }
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let state = state.borrow();
                    if !state.file_dialogs.contains(&dialog_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.fileDialogSelectFile unknown file dialog id {dialog_id}"
                        ))));
                    }
                    Ok(Value::Text(String::new()))
                })
            }))
        }),
    );

    fields.insert(
        "imageNewFromFile".to_string(),
        builtin("gtk4.imageNewFromFile", 1, |mut args, _| {
            let path = match args.remove(0) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.imageNewFromFile expects Text path")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_widget_id();
                    state.images.insert(id, path.clone());
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "imageNewFromResource".to_string(),
        builtin("gtk4.imageNewFromResource", 1, |mut args, _| {
            let resource_path = match args.remove(0) {
                Value::Text(v) => v,
                _ => {
                    return Err(invalid(
                        "gtk4.imageNewFromResource expects Text resource path",
                    ))
                }
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_widget_id();
                    state.images.insert(id, resource_path.clone());
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "imageSetFile".to_string(),
        builtin("gtk4.imageSetFile", 2, |mut args, _| {
            let path = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.imageSetFile expects Text path")),
            };
            let image_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.imageSetFile expects Int image id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(image) = state.images.get_mut(&image_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.imageSetFile unknown image id {image_id}"
                        ))));
                    };
                    *image = path.clone();
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "imageSetResource".to_string(),
        builtin("gtk4.imageSetResource", 2, |mut args, _| {
            let resource_path = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.imageSetResource expects Text resource path")),
            };
            let image_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.imageSetResource expects Int image id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(image) = state.images.get_mut(&image_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.imageSetResource unknown image id {image_id}"
                        ))));
                    };
                    *image = resource_path.clone();
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "listStoreNew".to_string(),
        builtin("gtk4.listStoreNew", 1, |mut args, _| {
            match args.remove(0) {
                Value::Unit => {}
                _ => return Err(invalid("gtk4.listStoreNew expects Unit")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_id();
                    state.list_stores.insert(id, Vec::new());
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "listStoreAppendText".to_string(),
        builtin("gtk4.listStoreAppendText", 2, |mut args, _| {
            let text = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.listStoreAppendText expects Text item")),
            };
            let store_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.listStoreAppendText expects Int store id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(store) = state.list_stores.get_mut(&store_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.listStoreAppendText unknown store id {store_id}"
                        ))));
                    };
                    store.push(text.clone());
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "listStoreItems".to_string(),
        builtin("gtk4.listStoreItems", 1, |mut args, _| {
            let store_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.listStoreItems expects Int store id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let state = state.borrow();
                    let Some(store) = state.list_stores.get(&store_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.listStoreItems unknown store id {store_id}"
                        ))));
                    };
                    Ok(Value::List(Arc::new(
                        store.iter().cloned().map(Value::Text).collect(),
                    )))
                })
            }))
        }),
    );

    fields.insert(
        "listViewNew".to_string(),
        builtin("gtk4.listViewNew", 1, |mut args, _| {
            match args.remove(0) {
                Value::Unit => {}
                _ => return Err(invalid("gtk4.listViewNew expects Unit")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_widget_id();
                    state.list_views.insert(id, 0);
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "listViewSetModel".to_string(),
        builtin("gtk4.listViewSetModel", 2, |mut args, _| {
            let store_id = match args.remove(1) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.listViewSetModel expects Int store id")),
            };
            let view_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.listViewSetModel expects Int list view id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    if !state.list_stores.contains_key(&store_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.listViewSetModel unknown store id {store_id}"
                        ))));
                    }
                    let Some(view) = state.list_views.get_mut(&view_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.listViewSetModel unknown list view id {view_id}"
                        ))));
                    };
                    *view = store_id;
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "treeViewNew".to_string(),
        builtin("gtk4.treeViewNew", 1, |mut args, _| {
            match args.remove(0) {
                Value::Unit => {}
                _ => return Err(invalid("gtk4.treeViewNew expects Unit")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_widget_id();
                    state.tree_views.insert(id, 0);
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "treeViewSetModel".to_string(),
        builtin("gtk4.treeViewSetModel", 2, |mut args, _| {
            let store_id = match args.remove(1) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.treeViewSetModel expects Int store id")),
            };
            let view_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.treeViewSetModel expects Int tree view id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    if !state.list_stores.contains_key(&store_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.treeViewSetModel unknown store id {store_id}"
                        ))));
                    }
                    let Some(view) = state.tree_views.get_mut(&view_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.treeViewSetModel unknown tree view id {view_id}"
                        ))));
                    };
                    *view = store_id;
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "gestureClickNew".to_string(),
        builtin("gtk4.gestureClickNew", 1, |mut args, _| {
            let widget_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.gestureClickNew expects Int widget id")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| -> Result<i64, RuntimeError> {
                    let mut state = state.borrow_mut();
                    state.ensure_widget(widget_id, "gestureClickNew")?;
                    let id = state.alloc_id();
                    state.gesture_clicks.insert(
                        id,
                        GestureClickState {
                            widget_id,
                            last_button: 0,
                        },
                    );
                    Ok(id)
                })?;
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "gestureClickLastButton".to_string(),
        builtin("gtk4.gestureClickLastButton", 1, |mut args, _| {
            let gesture_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => {
                    return Err(invalid(
                        "gtk4.gestureClickLastButton expects Int gesture id",
                    ))
                }
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let state = state.borrow();
                    let Some(gesture) = state.gesture_clicks.get(&gesture_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.gestureClickLastButton unknown gesture id {gesture_id}"
                        ))));
                    };
                    let _ = gesture.widget_id;
                    Ok(Value::Int(gesture.last_button))
                })
            }))
        }),
    );

    fields.insert(
        "widgetAddController".to_string(),
        builtin("gtk4.widgetAddController", 2, |mut args, _| {
            let controller_id = match args.remove(1) {
                Value::Int(v) => v,
                _ => {
                    return Err(invalid(
                        "gtk4.widgetAddController expects Int controller id",
                    ))
                }
            };
            let widget_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.widgetAddController expects Int widget id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    state.ensure_widget(widget_id, "widgetAddController")?;
                    if !state.gesture_clicks.contains_key(&controller_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.widgetAddController unknown controller id {controller_id}"
                        ))));
                    }
                    state
                        .widget_controllers
                        .entry(widget_id)
                        .or_default()
                        .push(controller_id);
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "clipboardDefault".to_string(),
        builtin("gtk4.clipboardDefault", 1, |mut args, _| {
            match args.remove(0) {
                Value::Unit => {}
                _ => return Err(invalid("gtk4.clipboardDefault expects Unit")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = 1;
                    state.clipboards.insert(id);
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "clipboardSetText".to_string(),
        builtin("gtk4.clipboardSetText", 2, |mut args, _| {
            let text = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.clipboardSetText expects Text")),
            };
            let clipboard_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.clipboardSetText expects Int clipboard id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    if !state.clipboards.contains(&clipboard_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.clipboardSetText unknown clipboard id {clipboard_id}"
                        ))));
                    }
                    state.clipboard_text = text.clone();
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "clipboardText".to_string(),
        builtin("gtk4.clipboardText", 1, |mut args, _| {
            let clipboard_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.clipboardText expects Int clipboard id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let state = state.borrow();
                    if !state.clipboards.contains(&clipboard_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.clipboardText unknown clipboard id {clipboard_id}"
                        ))));
                    }
                    Ok(Value::Text(state.clipboard_text.clone()))
                })
            }))
        }),
    );

    fields.insert(
        "actionNew".to_string(),
        builtin("gtk4.actionNew", 1, |mut args, _| {
            let name = match args.remove(0) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.actionNew expects Text name")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_id();
                    state.actions.insert(
                        id,
                        ActionState {
                            name: name.clone(),
                            enabled: true,
                        },
                    );
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "actionSetEnabled".to_string(),
        builtin("gtk4.actionSetEnabled", 2, |mut args, _| {
            let enabled = match args.remove(1) {
                Value::Bool(v) => v,
                _ => return Err(invalid("gtk4.actionSetEnabled expects Bool enabled")),
            };
            let action_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.actionSetEnabled expects Int action id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(action) = state.actions.get_mut(&action_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.actionSetEnabled unknown action id {action_id}"
                        ))));
                    };
                    let _ = &action.name;
                    action.enabled = enabled;
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "appAddAction".to_string(),
        builtin("gtk4.appAddAction", 2, |mut args, _| {
            let action_id = match args.remove(1) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.appAddAction expects Int action id")),
            };
            let app_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.appAddAction expects Int app id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    if !state.apps.contains_key(&app_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.appAddAction unknown app id {app_id}"
                        ))));
                    }
                    if !state.actions.contains_key(&action_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.appAddAction unknown action id {action_id}"
                        ))));
                    }
                    state.app_actions.entry(app_id).or_default().push(action_id);
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "shortcutNew".to_string(),
        builtin("gtk4.shortcutNew", 2, |mut args, _| {
            let action_name = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.shortcutNew expects Text action name")),
            };
            let trigger = match args.remove(0) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.shortcutNew expects Text trigger")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_id();
                    state.shortcuts.insert(
                        id,
                        ShortcutState {
                            trigger: trigger.clone(),
                            action_name: action_name.clone(),
                        },
                    );
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "widgetAddShortcut".to_string(),
        builtin("gtk4.widgetAddShortcut", 2, |mut args, _| {
            let shortcut_id = match args.remove(1) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.widgetAddShortcut expects Int shortcut id")),
            };
            let widget_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.widgetAddShortcut expects Int widget id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    state.ensure_widget(widget_id, "widgetAddShortcut")?;
                    let Some(shortcut) = state.shortcuts.get(&shortcut_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.widgetAddShortcut unknown shortcut id {shortcut_id}"
                        ))));
                    };
                    let _ = (&shortcut.trigger, &shortcut.action_name);
                    state
                        .widget_shortcuts
                        .entry(widget_id)
                        .or_default()
                        .push(shortcut_id);
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "notificationNew".to_string(),
        builtin("gtk4.notificationNew", 2, |mut args, _| {
            let body = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.notificationNew expects Text body")),
            };
            let title = match args.remove(0) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.notificationNew expects Text title")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_id();
                    state.notifications.insert(
                        id,
                        NotificationState {
                            title: title.clone(),
                            body: body.clone(),
                        },
                    );
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "notificationSetBody".to_string(),
        builtin("gtk4.notificationSetBody", 2, |mut args, _| {
            let body = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.notificationSetBody expects Text body")),
            };
            let notification_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => {
                    return Err(invalid(
                        "gtk4.notificationSetBody expects Int notification id",
                    ))
                }
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(notification) = state.notifications.get_mut(&notification_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.notificationSetBody unknown notification id {notification_id}"
                        ))));
                    };
                    let _ = &notification.title;
                    notification.body = body.clone();
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "appSendNotification".to_string(),
        builtin("gtk4.appSendNotification", 3, |mut args, _| {
            let notification_id = match args.remove(2) {
                Value::Int(v) => v,
                _ => {
                    return Err(invalid(
                        "gtk4.appSendNotification expects Int notification id",
                    ))
                }
            };
            let notif_key = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.appSendNotification expects Text key")),
            };
            let app_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.appSendNotification expects Int app id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    if !state.apps.contains_key(&app_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.appSendNotification unknown app id {app_id}"
                        ))));
                    }
                    if !state.notifications.contains_key(&notification_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.appSendNotification unknown notification id {notification_id}"
                        ))));
                    }
                    state
                        .app_notifications
                        .entry(app_id)
                        .or_default()
                        .insert(notif_key.clone(), notification_id);
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "appWithdrawNotification".to_string(),
        builtin("gtk4.appWithdrawNotification", 2, |mut args, _| {
            let notif_key = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.appWithdrawNotification expects Text key")),
            };
            let app_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.appWithdrawNotification expects Int app id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(map) = state.app_notifications.get_mut(&app_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.appWithdrawNotification unknown app id {app_id}"
                        ))));
                    };
                    map.remove(&notif_key);
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "layoutManagerNew".to_string(),
        builtin("gtk4.layoutManagerNew", 1, |mut args, _| {
            let kind = match args.remove(0) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.layoutManagerNew expects Text kind")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_id();
                    state
                        .layout_managers
                        .insert(id, LayoutManagerState { kind: kind.clone() });
                    id
                });
                Ok(Value::Int(id))
            }))
        }),
    );

    fields.insert(
        "widgetSetLayoutManager".to_string(),
        builtin("gtk4.widgetSetLayoutManager", 2, |mut args, _| {
            let layout_id = match args.remove(1) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.widgetSetLayoutManager expects Int layout id")),
            };
            let widget_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.widgetSetLayoutManager expects Int widget id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    state.ensure_widget(widget_id, "widgetSetLayoutManager")?;
                    let Some(layout) = state.layout_managers.get(&layout_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.widgetSetLayoutManager unknown layout manager id {layout_id}"
                        ))));
                    };
                    let _ = &layout.kind;
                    state.widget_layout_manager.insert(widget_id, layout_id);
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "buildFromNode".to_string(),
        builtin("gtk4.buildFromNode", 1, |mut args, _| {
            let node = args.remove(0);
            Ok(effect(move |_| {
                let decoded = decode_gtk_node(&node)?;
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let mut id_map = HashMap::new();
                    let root = first_object_in_interface(&decoded)?;
                    let id = build_widget_from_node_mock(&mut state, root, &mut id_map)?;
                    Ok(Value::Int(id))
                })
            }))
        }),
    );

    fields.insert(
        "signalPoll".to_string(),
        builtin("gtk4.signalPoll", 1, |mut args, _| {
            match args.remove(0) {
                Value::Unit => {}
                _ => return Err(invalid("gtk4.signalPoll expects Unit")),
            }
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    if let Some(event) = state.signal_events.pop_front() {
                        Ok(make_signal_event_value(event))
                    } else {
                        Ok(Value::Constructor {
                            name: "None".to_string(),
                            args: Vec::new(),
                        })
                    }
                })
            }))
        }),
    );

    fields.insert(
        "signalEmit".to_string(),
        builtin("gtk4.signalEmit", 4, |mut args, _| {
            let payload = match args.remove(3) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.signalEmit expects Text payload")),
            };
            let handler = match args.remove(2) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.signalEmit expects Text handler")),
            };
            let signal = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.signalEmit expects Text signal")),
            };
            let widget_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.signalEmit expects Int widget id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    state.ensure_widget(widget_id, "signalEmit")?;
                    state.signal_events.push_back(SignalEventState {
                        widget_id,
                        signal: signal.clone(),
                        handler: handler.clone(),
                        payload: payload.clone(),
                    });
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "osOpenUri".to_string(),
        builtin("gtk4.osOpenUri", 2, |mut args, _| {
            let uri = match args.remove(1) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.osOpenUri expects Text uri")),
            };
            let app_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.osOpenUri expects Int app id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    if !state.apps.contains_key(&app_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.osOpenUri unknown app id {app_id}"
                        ))));
                    }
                    state.last_opened_uri = Some(uri.clone());
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "osShowInFileManager".to_string(),
        builtin("gtk4.osShowInFileManager", 1, |mut args, _| {
            let path = match args.remove(0) {
                Value::Text(v) => v,
                _ => return Err(invalid("gtk4.osShowInFileManager expects Text path")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    state.last_revealed_path = Some(path.clone());
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "osSetBadgeCount".to_string(),
        builtin("gtk4.osSetBadgeCount", 2, |mut args, _| {
            let count = match args.remove(1) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.osSetBadgeCount expects Int count")),
            };
            let app_id = match args.remove(0) {
                Value::Int(v) => v,
                _ => return Err(invalid("gtk4.osSetBadgeCount expects Int app id")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    if !state.apps.contains_key(&app_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.osSetBadgeCount unknown app id {app_id}"
                        ))));
                    }
                    state.badge_count.insert(app_id, count);
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    fields.insert(
        "osThemePreference".to_string(),
        builtin("gtk4.osThemePreference", 1, |mut args, _| {
            match args.remove(0) {
                Value::Unit => {}
                _ => return Err(invalid("gtk4.osThemePreference expects Unit")),
            };
            Ok(effect(move |_| {
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    if state.theme_preference.is_empty() {
                        state.theme_preference = "system".to_string();
                    }
                    Ok(Value::Text(state.theme_preference.clone()))
                })
            }))
        }),
    );

    Value::Record(Arc::new(fields))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{build_gtk4_record_mock, Gtk4State, GTK4_STATE};
    use crate::{Runtime, RuntimeError, Value};

    fn gtk4_field(record: &Value, name: &str) -> Value {
        let Value::Record(fields) = record else {
            panic!("gtk4 builtin must be a record")
        };
        fields
            .get(name)
            .unwrap_or_else(|| panic!("missing gtk4 field: {name}"))
            .clone()
    }

    fn attr(name: &str, value: &str) -> Value {
        Value::Constructor {
            name: "GtkAttribute".to_string(),
            args: vec![
                Value::Text(name.to_string()),
                Value::Text(value.to_string()),
            ],
        }
    }

    fn text_node(text: &str) -> Value {
        Value::Constructor {
            name: "GtkTextNode".to_string(),
            args: vec![Value::Text(text.to_string())],
        }
    }

    fn element(tag: &str, attrs: Vec<Value>, children: Vec<Value>) -> Value {
        Value::Constructor {
            name: "GtkElement".to_string(),
            args: vec![
                Value::Text(tag.to_string()),
                Value::List(Arc::new(attrs)),
                Value::List(Arc::new(children)),
            ],
        }
    }

    #[test]
    fn image_resource_apis_store_and_update_resource_path() {
        GTK4_STATE.with(|state| *state.borrow_mut() = Gtk4State::default());
        let mut runtime = Runtime::default();
        let gtk4 = build_gtk4_record_mock();

        let new_from_resource = gtk4_field(&gtk4, "imageNewFromResource");
        let create_effect = runtime
            .call(
                new_from_resource,
                vec![Value::Text(
                    "/com/example/YourApp/icons/lucide/home.svg".to_string(),
                )],
            )
            .expect("create effect");
        let image_id = match runtime.run_effect_value(create_effect).expect("run effect") {
            Value::Int(id) => id,
            _ => panic!("expected image id"),
        };

        let set_resource = gtk4_field(&gtk4, "imageSetResource");
        let set_effect = runtime
            .call(
                set_resource,
                vec![
                    Value::Int(image_id),
                    Value::Text("/com/example/YourApp/icons/lucide/search.svg".to_string()),
                ],
            )
            .expect("set effect");
        runtime
            .run_effect_value(set_effect)
            .expect("run set effect");

        GTK4_STATE.with(|state| {
            let state = state.borrow();
            assert_eq!(
                state.images.get(&image_id),
                Some(&"/com/example/YourApp/icons/lucide/search.svg".to_string())
            );
        });
    }

    #[test]
    fn image_new_from_resource_requires_text_path() {
        GTK4_STATE.with(|state| *state.borrow_mut() = Gtk4State::default());
        let mut runtime = Runtime::default();
        let gtk4 = build_gtk4_record_mock();
        let new_from_resource = gtk4_field(&gtk4, "imageNewFromResource");

        let err = match runtime.call(new_from_resource, vec![Value::Int(1)]) {
            Ok(_) => panic!("should reject non-text path"),
            Err(err) => err,
        };
        assert!(matches!(
            err,
            RuntimeError::Message(msg)
            if msg == "gtk4.imageNewFromResource expects Text resource path"
        ));
    }

    #[test]
    fn build_from_node_creates_box_and_child_label() {
        GTK4_STATE.with(|state| *state.borrow_mut() = Gtk4State::default());
        let mut runtime = Runtime::default();
        let gtk4 = build_gtk4_record_mock();
        let build_from_node = gtk4_field(&gtk4, "buildFromNode");

        let node = Value::Constructor {
            name: "GtkElement".to_string(),
            args: vec![
                Value::Text("object".to_string()),
                Value::List(Arc::new(vec![
                    Value::Constructor {
                        name: "GtkAttribute".to_string(),
                        args: vec![
                            Value::Text("class".to_string()),
                            Value::Text("GtkBox".to_string()),
                        ],
                    },
                    Value::Constructor {
                        name: "GtkAttribute".to_string(),
                        args: vec![
                            Value::Text("prop:spacing".to_string()),
                            Value::Text("24".to_string()),
                        ],
                    },
                ])),
                Value::List(Arc::new(vec![Value::Constructor {
                    name: "GtkElement".to_string(),
                    args: vec![
                        Value::Text("child".to_string()),
                        Value::List(Arc::new(vec![])),
                        Value::List(Arc::new(vec![Value::Constructor {
                            name: "GtkElement".to_string(),
                            args: vec![
                                Value::Text("object".to_string()),
                                Value::List(Arc::new(vec![Value::Constructor {
                                    name: "GtkAttribute".to_string(),
                                    args: vec![
                                        Value::Text("class".to_string()),
                                        Value::Text("GtkLabel".to_string()),
                                    ],
                                }])),
                                Value::List(Arc::new(vec![Value::Constructor {
                                    name: "GtkElement".to_string(),
                                    args: vec![
                                        Value::Text("property".to_string()),
                                        Value::List(Arc::new(vec![Value::Constructor {
                                            name: "GtkAttribute".to_string(),
                                            args: vec![
                                                Value::Text("name".to_string()),
                                                Value::Text("label".to_string()),
                                            ],
                                        }])),
                                        Value::List(Arc::new(vec![Value::Constructor {
                                            name: "GtkTextNode".to_string(),
                                            args: vec![Value::Text("Hello".to_string())],
                                        }])),
                                    ],
                                }])),
                            ],
                        }])),
                    ],
                }])),
            ],
        };

        let effect = runtime
            .call(build_from_node, vec![node])
            .expect("buildFromNode should return effect");
        let root_id = match runtime
            .run_effect_value(effect)
            .expect("buildFromNode effect should succeed")
        {
            Value::Int(id) => id,
            _ => panic!("expected Int root id"),
        };

        GTK4_STATE.with(|state| {
            let state = state.borrow();
            let root_box = state.boxes.get(&root_id).expect("root should be a GtkBox");
            assert_eq!(root_box.spacing, 24);
            assert_eq!(root_box.children.len(), 1);
            let child_id = root_box.children[0];
            assert_eq!(state.labels.get(&child_id), Some(&"Hello".to_string()));
        });
    }

    #[test]
    fn build_from_node_accepts_interface_root() {
        GTK4_STATE.with(|state| *state.borrow_mut() = Gtk4State::default());
        let mut runtime = Runtime::default();
        let gtk4 = build_gtk4_record_mock();
        let build_from_node = gtk4_field(&gtk4, "buildFromNode");

        let node = Value::Constructor {
            name: "GtkElement".to_string(),
            args: vec![
                Value::Text("interface".to_string()),
                Value::List(Arc::new(vec![])),
                Value::List(Arc::new(vec![
                    Value::Constructor {
                        name: "GtkElement".to_string(),
                        args: vec![
                            Value::Text("requires".to_string()),
                            Value::List(Arc::new(vec![
                                Value::Constructor {
                                    name: "GtkAttribute".to_string(),
                                    args: vec![
                                        Value::Text("lib".to_string()),
                                        Value::Text("gtk".to_string()),
                                    ],
                                },
                                Value::Constructor {
                                    name: "GtkAttribute".to_string(),
                                    args: vec![
                                        Value::Text("version".to_string()),
                                        Value::Text("4.0".to_string()),
                                    ],
                                },
                            ])),
                            Value::List(Arc::new(vec![])),
                        ],
                    },
                    Value::Constructor {
                        name: "GtkElement".to_string(),
                        args: vec![
                            Value::Text("object".to_string()),
                            Value::List(Arc::new(vec![Value::Constructor {
                                name: "GtkAttribute".to_string(),
                                args: vec![
                                    Value::Text("class".to_string()),
                                    Value::Text("GtkLabel".to_string()),
                                ],
                            }])),
                            Value::List(Arc::new(vec![Value::Constructor {
                                name: "GtkElement".to_string(),
                                args: vec![
                                    Value::Text("property".to_string()),
                                    Value::List(Arc::new(vec![Value::Constructor {
                                        name: "GtkAttribute".to_string(),
                                        args: vec![
                                            Value::Text("name".to_string()),
                                            Value::Text("label".to_string()),
                                        ],
                                    }])),
                                    Value::List(Arc::new(vec![Value::Constructor {
                                        name: "GtkTextNode".to_string(),
                                        args: vec![Value::Text("FromInterface".to_string())],
                                    }])),
                                ],
                            }])),
                        ],
                    },
                ])),
            ],
        };

        let effect = runtime
            .call(build_from_node, vec![node])
            .expect("buildFromNode should return effect");
        let root_id = match runtime
            .run_effect_value(effect)
            .expect("buildFromNode effect should succeed")
        {
            Value::Int(id) => id,
            _ => panic!("expected Int root id"),
        };

        GTK4_STATE.with(|state| {
            let state = state.borrow();
            assert_eq!(
                state.labels.get(&root_id),
                Some(&"FromInterface".to_string())
            );
        });
    }

    #[test]
    fn build_from_node_resolves_ref_children() {
        GTK4_STATE.with(|state| *state.borrow_mut() = Gtk4State::default());
        let mut runtime = Runtime::default();
        let gtk4 = build_gtk4_record_mock();
        let build_from_node = gtk4_field(&gtk4, "buildFromNode");

        let label_object = Value::Constructor {
            name: "GtkElement".to_string(),
            args: vec![
                Value::Text("object".to_string()),
                Value::List(Arc::new(vec![
                    Value::Constructor {
                        name: "GtkAttribute".to_string(),
                        args: vec![
                            Value::Text("class".to_string()),
                            Value::Text("GtkLabel".to_string()),
                        ],
                    },
                    Value::Constructor {
                        name: "GtkAttribute".to_string(),
                        args: vec![
                            Value::Text("id".to_string()),
                            Value::Text("sharedLabel".to_string()),
                        ],
                    },
                ])),
                Value::List(Arc::new(vec![Value::Constructor {
                    name: "GtkElement".to_string(),
                    args: vec![
                        Value::Text("property".to_string()),
                        Value::List(Arc::new(vec![Value::Constructor {
                            name: "GtkAttribute".to_string(),
                            args: vec![
                                Value::Text("name".to_string()),
                                Value::Text("label".to_string()),
                            ],
                        }])),
                        Value::List(Arc::new(vec![Value::Constructor {
                            name: "GtkTextNode".to_string(),
                            args: vec![Value::Text("Shared".to_string())],
                        }])),
                    ],
                }])),
            ],
        };

        let ref_object = Value::Constructor {
            name: "GtkElement".to_string(),
            args: vec![
                Value::Text("object".to_string()),
                Value::List(Arc::new(vec![Value::Constructor {
                    name: "GtkAttribute".to_string(),
                    args: vec![
                        Value::Text("ref".to_string()),
                        Value::Text("sharedLabel".to_string()),
                    ],
                }])),
                Value::List(Arc::new(vec![])),
            ],
        };

        let node = Value::Constructor {
            name: "GtkElement".to_string(),
            args: vec![
                Value::Text("object".to_string()),
                Value::List(Arc::new(vec![Value::Constructor {
                    name: "GtkAttribute".to_string(),
                    args: vec![
                        Value::Text("class".to_string()),
                        Value::Text("GtkBox".to_string()),
                    ],
                }])),
                Value::List(Arc::new(vec![
                    Value::Constructor {
                        name: "GtkElement".to_string(),
                        args: vec![
                            Value::Text("child".to_string()),
                            Value::List(Arc::new(vec![])),
                            Value::List(Arc::new(vec![label_object])),
                        ],
                    },
                    Value::Constructor {
                        name: "GtkElement".to_string(),
                        args: vec![
                            Value::Text("child".to_string()),
                            Value::List(Arc::new(vec![])),
                            Value::List(Arc::new(vec![ref_object])),
                        ],
                    },
                ])),
            ],
        };

        let effect = runtime
            .call(build_from_node, vec![node])
            .expect("buildFromNode should return effect");
        let root_id = match runtime
            .run_effect_value(effect)
            .expect("buildFromNode effect should succeed")
        {
            Value::Int(id) => id,
            _ => panic!("expected Int root id"),
        };

        GTK4_STATE.with(|state| {
            let state = state.borrow();
            let root_box = state.boxes.get(&root_id).expect("root should be GtkBox");
            assert_eq!(root_box.children.len(), 2);
            assert_eq!(root_box.children[0], root_box.children[1]);
            assert_eq!(
                state.labels.get(&root_box.children[0]),
                Some(&"Shared".to_string())
            );
        });
    }

    #[test]
    fn build_from_node_accepts_template_root() {
        GTK4_STATE.with(|state| *state.borrow_mut() = Gtk4State::default());
        let mut runtime = Runtime::default();
        let gtk4 = build_gtk4_record_mock();
        let build_from_node = gtk4_field(&gtk4, "buildFromNode");

        let node = element(
            "template",
            vec![attr("class", "Card"), attr("parent", "GtkBox")],
            vec![element(
                "child",
                vec![],
                vec![element(
                    "object",
                    vec![attr("class", "GtkLabel")],
                    vec![element(
                        "property",
                        vec![attr("name", "label")],
                        vec![text_node("FromTemplate")],
                    )],
                )],
            )],
        );

        let effect = runtime
            .call(build_from_node, vec![node])
            .expect("buildFromNode should return effect");
        let root_id = match runtime
            .run_effect_value(effect)
            .expect("buildFromNode effect should succeed")
        {
            Value::Int(id) => id,
            _ => panic!("expected Int root id"),
        };

        GTK4_STATE.with(|state| {
            let state = state.borrow();
            assert_eq!(
                state.labels.get(&root_id),
                Some(&"FromTemplate".to_string())
            );
        });
    }

    #[test]
    fn build_from_node_applies_child_type_and_position() {
        GTK4_STATE.with(|state| *state.borrow_mut() = Gtk4State::default());
        let mut runtime = Runtime::default();
        let gtk4 = build_gtk4_record_mock();
        let build_from_node = gtk4_field(&gtk4, "buildFromNode");

        let node = element(
            "object",
            vec![attr("class", "GtkBox")],
            vec![
                element(
                    "child",
                    vec![attr("position", "1")],
                    vec![element(
                        "object",
                        vec![attr("class", "GtkLabel"), attr("id", "later")],
                        vec![element(
                            "property",
                            vec![attr("name", "label")],
                            vec![text_node("Later")],
                        )],
                    )],
                ),
                element(
                    "child",
                    vec![attr("position", "0")],
                    vec![element(
                        "object",
                        vec![attr("class", "GtkLabel"), attr("id", "first")],
                        vec![element(
                            "property",
                            vec![attr("name", "label")],
                            vec![text_node("First")],
                        )],
                    )],
                ),
                element(
                    "child",
                    vec![attr("type", "controller")],
                    vec![element(
                        "object",
                        vec![attr("class", "GtkGestureClick")],
                        vec![],
                    )],
                ),
            ],
        );

        let effect = runtime
            .call(build_from_node, vec![node])
            .expect("buildFromNode should return effect");
        let root_id = match runtime
            .run_effect_value(effect)
            .expect("buildFromNode effect should succeed")
        {
            Value::Int(id) => id,
            _ => panic!("expected Int root id"),
        };

        GTK4_STATE.with(|state| {
            let state = state.borrow();
            let root_box = state.boxes.get(&root_id).expect("root should be GtkBox");
            assert_eq!(root_box.children.len(), 2);
            assert_eq!(
                state.labels.get(&root_box.children[0]),
                Some(&"First".to_string())
            );
            assert_eq!(
                state.labels.get(&root_box.children[1]),
                Some(&"Later".to_string())
            );
            let controllers = state
                .widget_controllers
                .get(&root_id)
                .expect("box should have controller child");
            assert_eq!(controllers.len(), 1);
            let gesture = state
                .gesture_clicks
                .get(&controllers[0])
                .expect("controller should be gesture click");
            assert_eq!(gesture.widget_id, root_id);
        });
    }

    #[test]
    fn build_from_node_supports_drawing_area_and_scrolled_properties() {
        GTK4_STATE.with(|state| *state.borrow_mut() = Gtk4State::default());
        let mut runtime = Runtime::default();
        let gtk4 = build_gtk4_record_mock();
        let build_from_node = gtk4_field(&gtk4, "buildFromNode");

        let node = element(
            "object",
            vec![attr("class", "GtkScrolledWindow")],
            vec![
                element(
                    "property",
                    vec![attr("name", "hscrollbar-policy")],
                    vec![text_node("never")],
                ),
                element(
                    "property",
                    vec![attr("name", "vscrollbar-policy")],
                    vec![text_node("always")],
                ),
                element(
                    "property",
                    vec![attr("name", "propagate-natural-height")],
                    vec![text_node("true")],
                ),
                element(
                    "child",
                    vec![],
                    vec![element(
                        "object",
                        vec![
                            attr("class", "GtkDrawingArea"),
                            attr("prop:width-request", "640"),
                            attr("prop:height-request", "320"),
                        ],
                        vec![],
                    )],
                ),
            ],
        );

        let effect = runtime
            .call(build_from_node, vec![node])
            .expect("buildFromNode should return effect");
        let root_id = match runtime
            .run_effect_value(effect)
            .expect("buildFromNode effect should succeed")
        {
            Value::Int(id) => id,
            _ => panic!("expected Int root id"),
        };

        GTK4_STATE.with(|state| {
            let state = state.borrow();
            let scrolled = state
                .scroll_areas
                .get(&root_id)
                .expect("root should be scrolled window");
            let child_id = scrolled.child.expect("scrolled window child should exist");
            let drawing = state
                .draw_areas
                .get(&child_id)
                .expect("child should be drawing area");
            assert_eq!(drawing.width, 640);
            assert_eq!(drawing.height, 320);
        });
    }

    #[test]
    fn build_from_node_collects_signal_bindings() {
        GTK4_STATE.with(|state| *state.borrow_mut() = Gtk4State::default());
        let mut runtime = Runtime::default();
        let gtk4 = build_gtk4_record_mock();
        let build_from_node = gtk4_field(&gtk4, "buildFromNode");

        let node = element(
            "object",
            vec![
                attr("class", "GtkButton"),
                attr("signal:clicked", "Msg.Save"),
            ],
            vec![],
        );
        let effect = runtime
            .call(build_from_node, vec![node])
            .expect("buildFromNode should return effect");
        let root_id = match runtime
            .run_effect_value(effect)
            .expect("buildFromNode effect should succeed")
        {
            Value::Int(id) => id,
            _ => panic!("expected Int root id"),
        };

        GTK4_STATE.with(|state| {
            let state = state.borrow();
            let handlers = state
                .widget_signal_handlers
                .get(&root_id)
                .expect("signal bindings should be collected");
            assert_eq!(handlers.len(), 1);
            assert_eq!(handlers[0].signal, "clicked");
            assert_eq!(handlers[0].handler, "Msg.Save");
        });
    }

    #[test]
    fn signal_emit_and_poll_roundtrip() {
        GTK4_STATE.with(|state| *state.borrow_mut() = Gtk4State::default());
        let mut runtime = Runtime::default();
        let gtk4 = build_gtk4_record_mock();
        let button_new = gtk4_field(&gtk4, "buttonNew");
        let signal_emit = gtk4_field(&gtk4, "signalEmit");
        let signal_poll = gtk4_field(&gtk4, "signalPoll");

        let create_effect = runtime
            .call(button_new, vec![Value::Text("Save".to_string())])
            .expect("buttonNew should return effect");
        let widget_id = match runtime
            .run_effect_value(create_effect)
            .expect("buttonNew effect should succeed")
        {
            Value::Int(id) => id,
            _ => panic!("expected widget id"),
        };

        let emit_effect = runtime
            .call(
                signal_emit,
                vec![
                    Value::Int(widget_id),
                    Value::Text("clicked".to_string()),
                    Value::Text("Msg.Save".to_string()),
                    Value::Text("".to_string()),
                ],
            )
            .expect("signalEmit should return effect");
        runtime
            .run_effect_value(emit_effect)
            .expect("signalEmit effect should succeed");

        let poll_effect = runtime
            .call(signal_poll.clone(), vec![Value::Unit])
            .expect("signalPoll should return effect");
        let first = runtime
            .run_effect_value(poll_effect)
            .expect("signalPoll effect should succeed");
        let Value::Constructor { name, args } = first else {
            panic!("expected Option constructor");
        };
        assert_eq!(name, "Some");
        let Value::Constructor {
            name: evt_name,
            args: evt_args,
        } = &args[0]
        else {
            panic!("expected GtkSignalEvent payload");
        };
        assert_eq!(evt_name, "GtkSignalEvent");
        assert_eq!(evt_args.len(), 4);
        match &evt_args[0] {
            Value::Int(id) => assert_eq!(*id, widget_id),
            _ => panic!("expected widget id in event payload"),
        }
        match &evt_args[1] {
            Value::Text(text) => assert_eq!(text, "clicked"),
            _ => panic!("expected signal name in event payload"),
        }
        match &evt_args[2] {
            Value::Text(text) => assert_eq!(text, "Msg.Save"),
            _ => panic!("expected handler token in event payload"),
        }

        let poll_again_effect = runtime
            .call(signal_poll, vec![Value::Unit])
            .expect("signalPoll should return effect");
        let second = runtime
            .run_effect_value(poll_again_effect)
            .expect("signalPoll effect should succeed");
        let Value::Constructor { name, args } = second else {
            panic!("expected Option constructor");
        };
        assert_eq!(name, "None");
        assert!(args.is_empty(), "None must not carry payload");
    }
}
