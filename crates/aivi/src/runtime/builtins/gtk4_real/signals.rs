    unsafe extern "C" fn scroll_fade_cb(_adj: *mut c_void, data: *mut c_void) {
        let d = &*(data as *const ScrollFadeData);
        let adj = gtk_scrolled_window_get_vadjustment(d.scrolled);
        if adj.is_null() { return; }
        let value     = gtk_adjustment_get_value(adj);
        let upper     = gtk_adjustment_get_upper(adj);
        let page_size = gtk_adjustment_get_page_size(adj);
        let fade_px   = 50.0_f64;
        if !d.top_fade.is_null() {
            let opacity = (value / fade_px).clamp(0.0, 1.0);
            gtk_widget_set_opacity(d.top_fade, opacity);
        }
        if !d.bottom_fade.is_null() {
            let bottom_dist = (upper - page_size - value).max(0.0);
            let opacity = (bottom_dist / fade_px).clamp(0.0, 1.0);
            gtk_widget_set_opacity(d.bottom_fade, opacity);
        }
    }

    unsafe extern "C" fn scroll_fade_destroy(data: *mut c_void, _: *mut c_void) {
        drop(Box::from_raw(data as *mut ScrollFadeData));
    }

    fn wire_scroll_fades(
        scrolled: *mut c_void,
        top_fade: *mut c_void,
        bottom_fade: *mut c_void,
    ) {
        let data = Box::into_raw(Box::new(ScrollFadeData { scrolled, top_fade, bottom_fade }));
        unsafe {
            let adj = gtk_scrolled_window_get_vadjustment(scrolled);
            if adj.is_null() { return; }
            let sig = std::ffi::CString::new("value-changed").unwrap();
            g_signal_connect_data(
                adj,
                sig.as_ptr(),
                scroll_fade_cb as *const c_void,
                data as *mut c_void,
                scroll_fade_destroy as *mut c_void,
                0,
            );
            // set initial opacities
            scroll_fade_cb(adj, data as *mut c_void);
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

    fn parse_constructor_handler(handler: &str) -> (String, String) {
        if let Some(paren_pos) = handler.find('(') {
            let name = handler[..paren_pos].to_string();
            let arg = handler[paren_pos + 1..handler.len().saturating_sub(1)].to_string();
            (name, arg)
        } else {
            (handler.to_string(), String::new())
        }
    }

    fn make_signal_event_value(event: SignalEventState, widget_name: String) -> Value {
        let wid = Value::Int(event.widget_id);
        let name = Value::Text(widget_name);
        let inner = match event.signal.as_str() {
            "clicked" => {
                if event.handler.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    let (cname, arg) = parse_constructor_handler(&event.handler);
                    Value::Constructor {
                        name: "GtkUnknownSignal".to_string(),
                        args: vec![
                            wid,
                            name,
                            Value::Text(cname),
                            Value::Text(arg),
                            Value::Text(String::new()),
                        ],
                    }
                } else {
                    Value::Constructor {
                        name: "GtkClicked".to_string(),
                        args: vec![wid, name],
                    }
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
                Value::Constructor {
                    name: "GtkToggled".to_string(),
                    args: vec![wid, name, Value::Bool(active)],
                }
            }
            "value-changed" => {
                let val = event.payload.parse::<f64>().unwrap_or(0.0);
                Value::Constructor {
                    name: "GtkValueChanged".to_string(),
                    args: vec![wid, name, Value::Float(val)],
                }
            }
            "focus-enter" => Value::Constructor {
                name: "GtkFocusIn".to_string(),
                args: vec![wid, name],
            },
            "focus-leave" => Value::Constructor {
                name: "GtkFocusOut".to_string(),
                args: vec![wid, name],
            },
            _ => Value::Constructor {
                name: "GtkUnknownSignal".to_string(),
                args: vec![
                    wid,
                    name,
                    Value::Text(event.signal),
                    Value::Text(event.handler),
                    Value::Text(event.payload),
                ],
            },
        };
        inner
    }

    unsafe extern "C" fn gtk_signal_callback(instance: *mut c_void, data: *mut c_void) {
        if data.is_null() {
            return;
        }
        let binding = unsafe { &*(data as *const SignalCallbackData) };
        let payload = match binding.payload_kind {
            SignalPayloadKind::None => String::new(),
            SignalPayloadKind::EditableText => {
                let text_ptr = unsafe { gtk_editable_get_text(instance) };
                if text_ptr.is_null() {
                    String::new()
                } else {
                    unsafe { CStr::from_ptr(text_ptr) }
                        .to_string_lossy()
                        .into_owned()
                }
            }
            SignalPayloadKind::ToggleActive => {
                let active = unsafe { gtk_check_button_get_active(instance) };
                if active != 0 { "true" } else { "false" }.to_string()
            }
            SignalPayloadKind::FloatValue => {
                let val = unsafe { gtk_range_get_value(instance) };
                val.to_string()
            }
        };
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let event = SignalEventState {
                widget_id: binding.widget_id,
                signal: binding.signal_name.clone(),
                handler: binding.handler.clone(),
                payload,
            };
            // Broadcast to signalStream receivers (retain only live senders)
            let widget_name = state
                .widget_id_to_name
                .get(&binding.widget_id)
                .cloned()
                .unwrap_or_default();
            let typed_value = make_signal_event_value(event.clone(), widget_name);
            state.signal_senders.retain(|s| s.try_send(typed_value.clone()).is_ok());
            state.signal_events.push_back(event);
            // Apply any registered property bindings for this handler
            if let Some(bindings) = state.signal_bool_bindings.get(&binding.handler) {
                let mutations: Vec<_> = bindings
                    .iter()
                    .map(|b| (b.widget_id, b.property.clone(), b.value))
                    .collect();
                for (wid, prop, val) in mutations {
                    if let Some(&widget) = state.widgets.get(&wid) {
                        let v: c_int = if val { 1 } else { 0 };
                        if let Ok(prop_c) = CString::new(prop.as_str()) {
                            unsafe {
                                g_object_set(widget, prop_c.as_ptr(), v, std::ptr::null::<c_char>());
                            }
                        }
                    }
                }
            }
            // Apply any registered CSS class bindings for this handler
            if let Some(bindings) = state.signal_css_bindings.get(&binding.handler) {
                let mutations: Vec<_> = bindings
                    .iter()
                    .map(|b| (b.widget_id, b.class_name.clone(), b.add))
                    .collect();
                for (wid, class_name, add) in mutations {
                    if let Some(&widget) = state.widgets.get(&wid) {
                        if let Ok(class_c) = CString::new(class_name.as_str()) {
                            unsafe {
                                if add {
                                    gtk_widget_add_css_class(widget, class_c.as_ptr());
                                } else {
                                    gtk_widget_remove_css_class(widget, class_c.as_ptr());
                                }
                            }
                        }
                    }
                }
            }
            // Apply any registered toggle bool property bindings for this handler
            if let Some(bindings) = state.signal_toggle_bool_bindings.get(&binding.handler) {
                let mutations: Vec<_> = bindings
                    .iter()
                    .map(|b| (b.widget_id, b.property.clone()))
                    .collect();
                for (wid, prop) in mutations {
                    if let Some(&widget) = state.widgets.get(&wid) {
                        if let Ok(prop_c) = CString::new(prop.as_str()) {
                            unsafe {
                                let mut current: c_int = 0;
                                g_object_get(widget, prop_c.as_ptr(), &mut current as *mut c_int, std::ptr::null::<c_char>());
                                let toggled: c_int = if current != 0 { 0 } else { 1 };
                                g_object_set(widget, prop_c.as_ptr(), toggled, std::ptr::null::<c_char>());
                            }
                        }
                    }
                }
            }
            // Apply any registered toggle CSS class bindings for this handler
            if let Some(bindings) = state.signal_toggle_css_bindings.get(&binding.handler) {
                let mutations: Vec<_> = bindings
                    .iter()
                    .map(|b| (b.widget_id, b.class_name.clone()))
                    .collect();
                for (wid, class_name) in mutations {
                    if let Some(&widget) = state.widgets.get(&wid) {
                        if let Ok(class_c) = CString::new(class_name.as_str()) {
                            unsafe {
                                if gtk_widget_has_css_class(widget, class_c.as_ptr()) != 0 {
                                    gtk_widget_remove_css_class(widget, class_c.as_ptr());
                                } else {
                                    gtk_widget_add_css_class(widget, class_c.as_ptr());
                                }
                            }
                        }
                    }
                }
            }
            // Apply any registered dialog present bindings
            if let Some(bindings) = state.signal_dialog_bindings.get(&binding.handler) {
                let mutations: Vec<_> = bindings.iter().map(|b| (b.dialog_id, b.parent_id)).collect();
                for (dialog_id, parent_id) in mutations {
                    if let (Some(&dialog), Some(&parent)) = (state.widgets.get(&dialog_id), state.widgets.get(&parent_id)) {
                        call_adw_fn_pp("adw_dialog_present", dialog, parent);
                    }
                }
            }
            // Apply any registered stack page bindings
            if let Some(bindings) = state.signal_stack_page_bindings.get(&binding.handler) {
                let mutations: Vec<_> = bindings.iter().map(|b| (b.stack_id, b.page_name.clone())).collect();
                for (stack_id, page_name) in mutations {
                    if let Some(&stack) = state.widgets.get(&stack_id) {
                        if let Ok(page_c) = CString::new(page_name.as_str()) {
                            unsafe { gtk_stack_set_visible_child_name(stack, page_c.as_ptr()) };
                        }
                    }
                }
            }
        });
    }

    fn signal_payload_kind_for(class_name: &str, signal_name: &str) -> Option<SignalPayloadKind> {
        match (class_name, signal_name) {
            ("GtkButton", "clicked") => Some(SignalPayloadKind::None),
            ("GtkEntry", "changed") | ("GtkEntry", "activate")
            | ("GtkPasswordEntry", "changed") | ("GtkPasswordEntry", "activate")
            | ("AdwEntryRow", "changed") | ("AdwPasswordEntryRow", "changed") => {
                Some(SignalPayloadKind::EditableText)
            }
            ("GtkCheckButton", "toggled") => Some(SignalPayloadKind::ToggleActive),
            ("GtkRange", "value-changed") | ("GtkScale", "value-changed") => {
                Some(SignalPayloadKind::FloatValue)
            }
            _ => None,
        }
    }

    fn connect_widget_signal(
        widget: *mut c_void,
        widget_id: i64,
        class_name: &str,
        binding: &SignalBindingState,
    ) -> Result<c_ulong, RuntimeError> {
        let Some(payload_kind) = signal_payload_kind_for(class_name, &binding.signal) else {
            return Err(RuntimeError::Error(Value::Text(format!(
                "gtk4.buildFromNode unsupported signal `{}` on class `{class_name}`",
                binding.signal
            ))));
        };
        let signal_c = c_text(&binding.signal, "gtk4.buildFromNode invalid signal name")?;
        let callback_data = Box::new(SignalCallbackData {
            widget_id,
            signal_name: binding.signal.clone(),
            handler: binding.handler.clone(),
            payload_kind,
        });
        let callback_ptr = Box::into_raw(callback_data) as *mut c_void;
        let handler_id = unsafe {
            g_signal_connect_data(
                widget,
                signal_c.as_ptr(),
                gtk_signal_callback as *const c_void,
                callback_ptr,
                null_mut(),
                0,
            )
        };
        Ok(handler_id)
    }
