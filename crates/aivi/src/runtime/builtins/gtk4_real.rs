use crate::runtime::Value;

#[cfg(all(feature = "gtk4-libadwaita", target_os = "linux"))]
mod bridge {
    use std::collections::HashMap;
    use std::sync::atomic::AtomicBool;
    use std::sync::{mpsc, Arc, Mutex};

    use super::super::util::builtin;
    use crate::runtime::values::{ChannelInner, ChannelRecv};
    use crate::runtime::{EffectValue, RuntimeError, Value};

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

    fn decode_gtk_attr(value: &Value) -> Result<(String, String), RuntimeError> {
        let Value::Constructor { name, args } = value else {
            return Err(invalid("gtk4.buildFromNode expects GtkAttribute values"));
        };
        if name != "GtkAttribute" || args.len() != 2 {
            return Err(invalid("gtk4.buildFromNode expects GtkAttribute values"));
        }
        let key =
            decode_text(&args[0]).ok_or_else(|| invalid("gtk4.buildFromNode invalid attr name"))?;
        let val = decode_text(&args[1])
            .unwrap_or_else(|| serialize_signal_value(&args[1]));
        Ok((key, val))
    }

    fn decode_gtk_node(value: &Value) -> Result<aivi_gtk4::GtkNode, RuntimeError> {
        let Value::Constructor { name, args } = value else {
            return Err(invalid("gtk4.buildFromNode expects GtkNode"));
        };
        match (name.as_str(), args.len()) {
            ("GtkTextNode", 1) => {
                let text = decode_text(&args[0])
                    .ok_or_else(|| invalid("gtk4.buildFromNode invalid GtkTextNode text"))?;
                Ok(aivi_gtk4::GtkNode::Text(text))
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
                Ok(aivi_gtk4::GtkNode::Element {
                    tag,
                    attrs,
                    children,
                })
            }
            _ => Err(invalid("gtk4.buildFromNode expects GtkNode")),
        }
    }

    fn make_signal_event_value(event: aivi_gtk4::SignalEvent) -> Value {
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
            "focus-enter" => Value::Constructor { name: "GtkFocusIn".to_string(), args: vec![wid, name] },
            "focus-leave" => Value::Constructor { name: "GtkFocusOut".to_string(), args: vec![wid, name] },
            "notify::show-sidebar" => {
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
            Ok(effect(move |_| { aivi_gtk4::init().map_err(gtk4_err_to_runtime)?; Ok(Value::Unit) }))
        }));

        // ── appNew ──
        fields.insert("appNew".to_string(), builtin("gtk4.appNew", 1, |mut args, _| {
            let id = match args.remove(0) { Value::Text(v) => v, _ => return Err(invalid("gtk4.appNew expects Text")) };
            Ok(effect(move |_| { let r = aivi_gtk4::app_new(&id).map_err(gtk4_err_to_runtime)?; Ok(Value::Int(r)) }))
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
            Ok(effect(move |_| {
                let decoded = decode_gtk_node(&node)?;
                let id = aivi_gtk4::build_from_node(&decoded).map_err(gtk4_err_to_runtime)?;
                Ok(Value::Int(id))
            }))
        }));

        fields.insert("buildWithIds".to_string(), builtin("gtk4.buildWithIds", 1, |mut args, _| {
            let node = args.remove(0);
            Ok(effect(move |_| {
                let decoded = decode_gtk_node(&node)?;
                let result = aivi_gtk4::build_with_ids(&decoded).map_err(gtk4_err_to_runtime)?;
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
            Ok(effect(move |_| {
                let decoded = decode_gtk_node(&new_node_val)?;
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

/// Drives one iteration of the GTK/GLib main context from any call site.
#[cfg(all(feature = "gtk4-libadwaita", target_os = "linux"))]
pub(super) fn pump_gtk_events() {
    aivi_gtk4::pump_events();
}

#[cfg(not(all(feature = "gtk4-libadwaita", target_os = "linux")))]
pub(super) fn pump_gtk_events() {}

/// Returns true when a GTK application is active and events need pumping.
#[cfg(all(feature = "gtk4-libadwaita", target_os = "linux"))]
pub(super) fn is_gtk_pump_active() -> bool {
    aivi_gtk4::is_pump_active()
}

#[cfg(not(all(feature = "gtk4-libadwaita", target_os = "linux")))]
pub(super) fn is_gtk_pump_active() -> bool {
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
