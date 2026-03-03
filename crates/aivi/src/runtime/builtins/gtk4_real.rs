use crate::runtime::Value;

#[cfg(all(feature = "gtk4-libadwaita", target_os = "linux"))]
mod linux {
    use std::cell::RefCell;
    use std::collections::{HashMap, VecDeque};
    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_int, c_uint, c_ulong, c_void};
    use std::ptr::null_mut;
    use std::sync::atomic::AtomicBool;
    use std::sync::{mpsc, Arc, Mutex, OnceLock};

    use super::super::util::builtin;
    use crate::runtime::values::{ChannelInner, ChannelRecv};
    use crate::runtime::{EffectValue, RuntimeError, Value};

    #[link(name = "gtk-4")]
    unsafe extern "C" {
        fn gtk_init();
        fn gtk_application_new(application_id: *const c_char, flags: c_int) -> *mut c_void;
        fn gtk_window_set_title(window: *mut c_void, title: *const c_char);
        fn gtk_window_set_default_size(window: *mut c_void, width: c_int, height: c_int);
        fn gtk_window_set_titlebar(window: *mut c_void, titlebar: *mut c_void);
        fn gtk_window_new() -> *mut c_void;
        fn gtk_window_set_child(window: *mut c_void, child: *mut c_void);
        fn gtk_window_set_modal(window: *mut c_void, modal: c_int);
        fn gtk_window_set_transient_for(window: *mut c_void, parent: *mut c_void);
        fn gtk_window_present(window: *mut c_void);
        fn gtk_window_close(window: *mut c_void);
        fn gtk_window_set_hide_on_close(window: *mut c_void, setting: c_int);
        fn gtk_window_set_decorated(window: *mut c_void, setting: c_int);

        fn gtk_widget_set_visible(widget: *mut c_void, visible: c_int);
        fn gtk_widget_set_sensitive(widget: *mut c_void, sensitive: c_int);
        fn gtk_widget_set_size_request(widget: *mut c_void, width: c_int, height: c_int);
        fn gtk_widget_set_hexpand(widget: *mut c_void, expand: c_int);
        fn gtk_widget_set_vexpand(widget: *mut c_void, expand: c_int);
        fn gtk_widget_set_halign(widget: *mut c_void, align: c_int);
        fn gtk_widget_set_valign(widget: *mut c_void, align: c_int);
        fn gtk_widget_set_margin_start(widget: *mut c_void, margin: c_int);
        fn gtk_widget_set_margin_end(widget: *mut c_void, margin: c_int);
        fn gtk_widget_set_margin_top(widget: *mut c_void, margin: c_int);
        fn gtk_widget_set_margin_bottom(widget: *mut c_void, margin: c_int);
        fn gtk_widget_add_css_class(widget: *mut c_void, css_class: *const c_char);
        fn gtk_widget_remove_css_class(widget: *mut c_void, css_class: *const c_char);
        fn gtk_widget_has_css_class(widget: *mut c_void, css_class: *const c_char) -> c_int;
        fn gtk_widget_set_tooltip_text(widget: *mut c_void, text: *const c_char);
        fn gtk_widget_set_name(widget: *mut c_void, name: *const c_char);
        fn gtk_widget_queue_draw(widget: *mut c_void);
        fn gtk_widget_set_opacity(widget: *mut c_void, opacity: f64);
        fn gtk_widget_unparent(widget: *mut c_void);
        // Sets the AT-SPI accessible label (GTK_ACCESSIBLE_PROPERTY_LABEL = 5)
        fn gtk_accessible_update_property(accessible: *mut c_void, first_property: c_int, ...);

        fn gtk_box_new(orientation: c_int, spacing: c_int) -> *mut c_void;
        fn gtk_box_append(container: *mut c_void, child: *mut c_void);
        fn gtk_box_remove(container: *mut c_void, child: *mut c_void);
        fn gtk_box_insert_child_after(
            container: *mut c_void,
            child: *mut c_void,
            sibling: *mut c_void,
        );
        fn gtk_box_set_homogeneous(boxw: *mut c_void, homogeneous: c_int);
        fn gtk_header_bar_new() -> *mut c_void;
        fn gtk_header_bar_pack_start(header_bar: *mut c_void, child: *mut c_void);
        fn gtk_header_bar_pack_end(header_bar: *mut c_void, child: *mut c_void);
        fn gtk_header_bar_set_title_widget(header_bar: *mut c_void, title_widget: *mut c_void);
        fn gtk_header_bar_set_show_title_buttons(header_bar: *mut c_void, setting: c_int);
        fn gtk_header_bar_set_decoration_layout(
            header_bar: *mut c_void,
            layout: *const c_char,
        );
        fn gtk_list_box_new() -> *mut c_void;
        fn gtk_list_box_append(list_box: *mut c_void, child: *mut c_void);
        fn gtk_list_box_remove(list_box: *mut c_void, child: *mut c_void);

        fn gtk_drawing_area_new() -> *mut c_void;

        fn gtk_button_new_with_label(label: *const c_char) -> *mut c_void;
        fn gtk_button_set_label(button: *mut c_void, label: *const c_char);
        fn gtk_button_new_from_icon_name(icon_name: *const c_char) -> *mut c_void;

        fn gtk_label_new(text: *const c_char) -> *mut c_void;
        fn gtk_label_set_text(label: *mut c_void, text: *const c_char);
        fn gtk_label_set_wrap(label: *mut c_void, wrap: c_int);
        fn gtk_label_set_ellipsize(label: *mut c_void, mode: c_int);
        fn gtk_label_set_xalign(label: *mut c_void, xalign: f32);
        fn gtk_label_set_max_width_chars(label: *mut c_void, n_chars: c_int);

        fn gtk_entry_new() -> *mut c_void;
        fn gtk_password_entry_new() -> *mut c_void;
        fn gtk_password_entry_set_show_peek_icon(
            entry: *mut c_void,
            show_peek_icon: c_int,
        );
        fn gtk_editable_set_text(editable: *mut c_void, text: *const c_char);
        fn gtk_editable_get_text(editable: *mut c_void) -> *const c_char;
        fn gtk_check_button_get_active(check_button: *mut c_void) -> c_int;
        fn gtk_range_get_value(range: *mut c_void) -> f64;

        fn gtk_text_view_new() -> *mut c_void;
        fn gtk_text_view_set_wrap_mode(text_view: *mut c_void, wrap_mode: c_int);
        fn gtk_text_view_set_top_margin(text_view: *mut c_void, top_margin: c_int);
        fn gtk_text_view_set_bottom_margin(text_view: *mut c_void, bottom_margin: c_int);
        fn gtk_text_view_set_left_margin(text_view: *mut c_void, left_margin: c_int);
        fn gtk_text_view_set_right_margin(text_view: *mut c_void, right_margin: c_int);
        fn gtk_text_view_set_editable(text_view: *mut c_void, setting: c_int);
        fn gtk_text_view_set_cursor_visible(text_view: *mut c_void, setting: c_int);

        fn gtk_image_new_from_file(filename: *const c_char) -> *mut c_void;
        fn gtk_image_set_from_file(image: *mut c_void, filename: *const c_char);
        fn gtk_image_new_from_resource(resource_path: *const c_char) -> *mut c_void;
        fn gtk_image_set_from_resource(image: *mut c_void, resource_path: *const c_char);
        fn gtk_image_new_from_icon_name(icon_name: *const c_char) -> *mut c_void;
        fn gtk_image_set_pixel_size(image: *mut c_void, pixel_size: c_int);

        fn gtk_scrolled_window_new() -> *mut c_void;
        fn gtk_scrolled_window_set_child(scrolled: *mut c_void, child: *mut c_void);
        fn gtk_scrolled_window_set_policy(
            scrolled: *mut c_void,
            hscrollbar_policy: c_int,
            vscrollbar_policy: c_int,
        );
        fn gtk_scrolled_window_set_propagate_natural_height(
            scrolled: *mut c_void,
            propagate: c_int,
        );
        fn gtk_scrolled_window_set_propagate_natural_width(scrolled: *mut c_void, propagate: c_int);
        fn gtk_scrolled_window_get_vadjustment(scrolled: *mut c_void) -> *mut c_void;

        fn gtk_adjustment_get_value(adjustment: *mut c_void) -> f64;
        fn gtk_adjustment_get_upper(adjustment: *mut c_void) -> f64;
        fn gtk_adjustment_get_page_size(adjustment: *mut c_void) -> f64;

        fn gtk_separator_new(orientation: c_int) -> *mut c_void;

        fn gtk_overlay_new() -> *mut c_void;
        fn gtk_overlay_set_child(overlay: *mut c_void, child: *mut c_void);
        fn gtk_overlay_add_overlay(overlay: *mut c_void, widget: *mut c_void);
        fn gtk_overlay_remove_overlay(overlay: *mut c_void, widget: *mut c_void);

        fn gtk_css_provider_new() -> *mut c_void;
        fn gtk_css_provider_load_from_string(provider: *mut c_void, css: *const c_char);
        fn gtk_style_context_add_provider_for_display(
            display: *mut c_void,
            provider: *mut c_void,
            priority: u32,
        );

        fn gtk_gesture_click_new() -> *mut c_void;
        fn gtk_widget_add_controller(widget: *mut c_void, controller: *mut c_void);

        fn gtk_icon_theme_get_for_display(display: *mut c_void) -> *mut c_void;
        fn gtk_icon_theme_add_search_path(icon_theme: *mut c_void, path: *const c_char);
        fn gtk_button_set_child(button: *mut c_void, child: *mut c_void);

        fn gtk_stack_new() -> *mut c_void;
        fn gtk_stack_add_named(stack: *mut c_void, child: *mut c_void, name: *const c_char);
        fn gtk_stack_set_visible_child_name(stack: *mut c_void, name: *const c_char);

        fn gtk_menu_button_new() -> *mut c_void;

        fn gtk_revealer_new() -> *mut c_void;
        fn gtk_revealer_set_child(revealer: *mut c_void, child: *mut c_void);
        fn gtk_revealer_set_reveal_child(revealer: *mut c_void, reveal_child: c_int);
        fn gtk_revealer_set_transition_type(revealer: *mut c_void, transition: c_int);
        fn gtk_revealer_set_transition_duration(revealer: *mut c_void, duration: c_uint);

        fn gdk_display_get_default() -> *mut c_void;
    }

    #[link(name = "gio-2.0")]
    unsafe extern "C" {
        fn g_application_run(
            application: *mut c_void,
            argc: c_int,
            argv: *mut *mut c_char,
        ) -> c_int;
        fn g_resource_load(filename: *const c_char, error: *mut *mut c_void) -> *mut c_void;
        fn g_resources_register(resource: *mut c_void);
    }

    #[link(name = "glib-2.0")]
    unsafe extern "C" {
        fn g_main_context_default() -> *mut c_void;
        fn g_main_context_pending(context: *mut c_void) -> c_int;
        fn g_main_context_iteration(context: *mut c_void, may_block: c_int) -> c_int;
    }

    #[link(name = "gobject-2.0")]
    unsafe extern "C" {
        fn g_type_from_name(name: *const c_char) -> usize;
        fn g_object_new(object_type: usize, first_property_name: *const c_char, ...) -> *mut c_void;
        fn g_object_ref_sink(object: *mut c_void) -> *mut c_void;
        fn g_object_set(object: *mut c_void, first_property_name: *const c_char, ...);
        fn g_object_get(object: *mut c_void, first_property_name: *const c_char, ...);
        fn g_signal_connect_data(
            instance: *mut c_void,
            detailed_signal: *const c_char,
            c_handler: *const c_void,
            data: *mut c_void,
            destroy_data: *mut c_void,
            connect_flags: c_int,
        ) -> c_ulong;
        fn g_signal_handler_disconnect(instance: *mut c_void, handler_id: c_ulong);
    }

    unsafe extern "C" fn activate_noop(_app: *mut c_void, _data: *mut c_void) {}

    #[link(name = "dl")]
    unsafe extern "C" {
        fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlclose(handle: *mut c_void) -> c_int;
    }

    thread_local! {
        static GTK_STATE: RefCell<RealGtkState> = RefCell::new(RealGtkState::default());
        // Set to true once gtk_init() has been called so channel.recv can pump the GTK event loop.
        static GTK_PUMP_ACTIVE: RefCell<bool> = const { RefCell::new(false) };
    }

    // Cross-thread queue for tray icon actions (SNI runs in a separate tokio thread).
    static PENDING_TRAY_ACTIONS: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
    fn pending_tray_actions() -> &'static Mutex<VecDeque<String>> {
        PENDING_TRAY_ACTIONS.get_or_init(|| Mutex::new(VecDeque::new()))
    }

    // Flag set when menu items change; tokio thread polls and emits LayoutUpdated.
    static PENDING_LAYOUT_UPDATE: OnceLock<Mutex<bool>> = OnceLock::new();
    fn pending_layout_update() -> &'static Mutex<bool> {
        PENDING_LAYOUT_UPDATE.get_or_init(|| Mutex::new(false))
    }

    include!("gtk4_real/types.rs");
    include!("gtk4_real/helpers.rs");
    include!("gtk4_real/sni_tray.rs");
    include!("gtk4_real/signals.rs");
    include!("gtk4_real/widget_builder.rs");
    include!("gtk4_real/reconciler.rs");

    pub(super) fn pump_gtk_events() {
        GTK_PUMP_ACTIVE.with(|active| {
            if *active.borrow() {
                unsafe {
                    let ctx = g_main_context_default();
                    // Drain all pending events (not just one) to avoid input lag.
                    while g_main_context_pending(ctx) != 0 {
                        g_main_context_iteration(ctx, 0);
                    }
                }
                // Drain cross-thread tray actions into the signal stream.
                let actions: Vec<String> = pending_tray_actions()
                    .lock()
                    .map(|mut q| q.drain(..).collect())
                    .unwrap_or_default();
                for raw_action in actions {
                    // Format is "action_name:x:y" (coords optional)
                    let (action_name, coords) = raw_action
                        .split_once(':')
                        .map(|(a, rest)| (a.to_string(), rest.to_string()))
                        .unwrap_or_else(|| (raw_action.clone(), String::new()));
                    let event = SignalEventState {
                        widget_id: 0,
                        signal: "mailfox.tray.action".to_string(),
                        handler: action_name,
                        payload: coords,
                    };
                    let typed_value = make_signal_event_value(event.clone(), String::new());
                    GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        state.signal_senders.retain(|s| s.try_send(typed_value.clone()).is_ok());
                        state.signal_events.push_back(event);
                    });
                }
            }
        });
    }

    pub(super) fn build_from_mock(mut fields: HashMap<String, Value>) -> HashMap<String, Value> {
        fields.insert(
            "init".to_string(),
            builtin("gtk4.init", 1, |mut args, _| {
                match args.remove(0) {
                    Value::Unit => {}
                    _ => return Err(invalid("gtk4.init expects Unit")),
                }
                Ok(effect(|_| {
                    unsafe { gtk_init() };
                    try_adw_init();
                    GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        if !state.resources_registered {
                            maybe_register_gresource_bundle()?;
                            state.resources_registered = true;
                        }
                        Ok::<(), RuntimeError>(())
                    })?;
                    Ok(Value::Unit)
                }))
            }),
        );

        fields.insert(
            "appNew".to_string(),
            builtin("gtk4.appNew", 1, |mut args, _| {
                let app_id = match args.remove(0) {
                    Value::Text(text) => text,
                    _ => return Err(invalid("gtk4.appNew expects Text application id")),
                };
                Ok(effect(move |_| {
                    let app_id_c = c_text(&app_id, "gtk4.appNew invalid application id")?;
                    // Ensure GTK + libadwaita are initialized before creating the app
                    unsafe { gtk_init() };
                    try_adw_init();
                    let raw = unsafe { gtk_application_new(app_id_c.as_ptr(), 0) };
                    if raw.is_null() {
                        return Err(RuntimeError::Error(Value::Text(
                            "gtk4.appNew failed to create GTK application".to_string(),
                        )));
                    }
                    // Keep a strong owned reference while tracked in GTK_STATE so later
                    // window creation never observes a dropped/invalid application handle.
                    unsafe { g_object_ref_sink(raw) };
                    // Connect a no-op activate handler so GTK does not warn
                    let sig = CString::new("activate").unwrap();
                    unsafe {
                        g_signal_connect_data(
                            raw,
                            sig.as_ptr(),
                            activate_noop as *const c_void,
                            null_mut(),
                            null_mut(),
                            0,
                        );
                    }
                    let id = GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let id = state.alloc_id();
                        state.apps.insert(id, raw);
                        id
                    });
                    // GTK is now initialised; channel.recv can safely pump the event loop.
                    GTK_PUMP_ACTIVE.with(|active| *active.borrow_mut() = true);
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
                    let width_i32 = as_i32(width, "gtk4.windowNew width out of range")?;
                    let height_i32 = as_i32(height, "gtk4.windowNew height out of range")?;
                    let title_c = c_text(&title, "gtk4.windowNew invalid title")?;
                    let id = GTK_STATE.with(|state| -> Result<i64, RuntimeError> {
                        let mut state = state.borrow_mut();
                        let _ = state.apps.get(&app_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.windowNew unknown app id {app_id}"
                            )))
                        })?;
                        let window = unsafe { gtk_window_new() };
                        if window.is_null() {
                            return Err(RuntimeError::Error(Value::Text(
                                "gtk4.windowNew failed to create window".to_string(),
                            )));
                        }
                        unsafe {
                            gtk_window_set_title(window, title_c.as_ptr());
                            gtk_window_set_default_size(window, width_i32, height_i32);
                        }
                        let id = state.alloc_id();
                        state.windows.insert(id, window);
                        state.widgets.insert(id, window);
                        apply_pending_display_customizations(&mut state)?;
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
                    let title_c = c_text(&title, "gtk4.windowSetTitle invalid title")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let window = state.windows.get(&window_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.windowSetTitle unknown window id {window_id}"
                            )))
                        })?;
                        unsafe { gtk_window_set_title(window, title_c.as_ptr()) };
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
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let window = state.windows.get(&window_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.windowSetTitlebar unknown window id {window_id}"
                            )))
                        })?;
                        let titlebar = widget_ptr(&state, titlebar_id, "windowSetTitlebar")?;
                        unsafe { gtk_window_set_titlebar(window, titlebar) };
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
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let window = state.windows.get(&window_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.windowSetChild unknown window id {window_id}"
                            )))
                        })?;
                        let child = widget_ptr(&state, child_id, "windowSetChild")?;
                        unsafe { gtk_window_set_child(window, child) };
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
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let window = state.windows.get(&window_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.windowPresent unknown window id {window_id}"
                            )))
                        })?;
                        unsafe { gtk_window_present(window) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "windowClose".to_string(),
            builtin("gtk4.windowClose", 1, |mut args, _| {
                let window_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.windowClose expects Int window id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let window = state.windows.get(&window_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.windowClose unknown window id {window_id}"
                            )))
                        })?;
                        unsafe { gtk_window_close(window) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "windowSetDecorated".to_string(),
            builtin("gtk4.windowSetDecorated", 2, |mut args, _| {
                let decorated = match args.remove(1) {
                    Value::Bool(v) => v,
                    _ => return Err(invalid("gtk4.windowSetDecorated expects Bool")),
                };
                let window_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.windowSetDecorated expects Int window id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let window = *state.windows.get(&window_id).ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.windowSetDecorated unknown window id {window_id}"
                            )))
                        })?;
                        unsafe {
                            gtk_window_set_decorated(window, if decorated { 1 } else { 0 });
                        }
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "windowSetHideOnClose".to_string(),
            builtin("gtk4.windowSetHideOnClose", 2, |mut args, _| {
                let hide = match args.remove(1) {
                    Value::Bool(v) => v,
                    _ => return Err(invalid("gtk4.windowSetHideOnClose expects Bool")),
                };
                let window_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.windowSetHideOnClose expects Int window id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let window = state.windows.get(&window_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.windowSetHideOnClose unknown window id {window_id}"
                            )))
                        })?;
                        unsafe { gtk_window_set_hide_on_close(window, if hide { 1 } else { 0 }) };
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
                    let app = GTK_STATE.with(|state| {
                        let state = state.borrow();
                        state.apps.get(&app_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.appRun unknown app id {app_id}"
                            )))
                        })
                    })?;
                    // Run outside the borrow so signal callbacks can borrow_mut
                    unsafe {
                        let _ = g_application_run(app, 0, null_mut());
                    }
                    Ok(Value::Unit)
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
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let widget = widget_ptr(&state, widget_id, "widgetShow")?;
                        unsafe { gtk_widget_set_visible(widget, 1) };
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
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let widget = widget_ptr(&state, widget_id, "widgetHide")?;
                        unsafe { gtk_widget_set_visible(widget, 0) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "widgetSetBoolProperty".to_string(),
            builtin("gtk4.widgetSetBoolProperty", 3, |mut args, _| {
                let value = match args.remove(2) {
                    Value::Bool(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetBoolProperty expects Bool value")),
                };
                let prop_name = match args.remove(1) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetBoolProperty expects Text property name")),
                };
                let widget_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetBoolProperty expects Int widget id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let widget = widget_ptr(&state, widget_id, "widgetSetBoolProperty")?;
                        let prop_c = c_text(&prop_name, "gtk4.widgetSetBoolProperty invalid property name")?;
                        let v: c_int = if value { 1 } else { 0 };
                        unsafe { g_object_set(widget, prop_c.as_ptr(), v, std::ptr::null::<c_char>()) };
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
                    let spacing_i32 = as_i32(spacing, "gtk4.boxNew spacing out of range")?;
                    let orientation_i32: i32 = if orientation == 1 { 1 } else { 0 };
                    let id = GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let raw = unsafe { gtk_box_new(orientation_i32, spacing_i32) };
                        let id = state.alloc_id();
                        state.boxes.insert(id, raw);
                        state.widgets.insert(id, raw);
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
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let container = state.boxes.get(&box_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.boxAppend unknown box id {box_id}"
                            )))
                        })?;
                        let child = widget_ptr(&state, child_id, "boxAppend")?;
                        unsafe { gtk_box_append(container, child) };
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
                    let width_i32 = as_i32(width, "gtk4.drawAreaNew width out of range")?;
                    let height_i32 = as_i32(height, "gtk4.drawAreaNew height out of range")?;
                    let id = GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let raw = unsafe { gtk_drawing_area_new() };
                        unsafe { gtk_widget_set_size_request(raw, width_i32, height_i32) };
                        let id = state.alloc_id();
                        state.draw_areas.insert(id, raw);
                        state.widgets.insert(id, raw);
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
                    let width_i32 =
                        as_i32(width, "gtk4.drawAreaSetContentSize width out of range")?;
                    let height_i32 =
                        as_i32(height, "gtk4.drawAreaSetContentSize height out of range")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let draw =
                            state
                                .draw_areas
                                .get(&draw_area_id)
                                .copied()
                                .ok_or_else(|| {
                                    RuntimeError::Error(Value::Text(format!(
                                "gtk4.drawAreaSetContentSize unknown draw area id {draw_area_id}"
                            )))
                                })?;
                        unsafe { gtk_widget_set_size_request(draw, width_i32, height_i32) };
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
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let draw =
                            state
                                .draw_areas
                                .get(&draw_area_id)
                                .copied()
                                .ok_or_else(|| {
                                    RuntimeError::Error(Value::Text(format!(
                                "gtk4.drawAreaQueueDraw unknown draw area id {draw_area_id}"
                            )))
                                })?;
                        unsafe { gtk_widget_queue_draw(draw) };
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
                    let id = GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let _widget = widget_ptr(&state, widget_id, "gestureClickNew")?;
                        let raw = unsafe { gtk_gesture_click_new() };
                        let id = state.alloc_id();
                        state.gesture_clicks.insert(
                            id,
                            GestureClickState {
                                widget_id,
                                raw,
                                last_button: 0,
                            },
                        );
                        Ok::<i64, RuntimeError>(id)
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
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let gesture = state.gesture_clicks.get(&gesture_id).ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.gestureClickLastButton unknown gesture id {gesture_id}"
                            )))
                        })?;
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
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let widget = widget_ptr(&state, widget_id, "widgetAddController")?;
                        let gesture =
                            state.gesture_clicks.get(&controller_id).ok_or_else(|| {
                                RuntimeError::Error(Value::Text(format!(
                                "gtk4.widgetAddController unknown controller id {controller_id}"
                            )))
                            })?;
                        unsafe { gtk_widget_add_controller(widget, gesture.raw) };
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
                    let label_c = c_text(&label, "gtk4.buttonNew invalid label")?;
                    let id = GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let raw = unsafe { gtk_button_new_with_label(label_c.as_ptr()) };
                        let id = state.alloc_id();
                        state.buttons.insert(id, raw);
                        state.widgets.insert(id, raw);
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
                    let label_c = c_text(&label, "gtk4.buttonSetLabel invalid label")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let button = state.buttons.get(&button_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.buttonSetLabel unknown button id {button_id}"
                            )))
                        })?;
                        unsafe { gtk_button_set_label(button, label_c.as_ptr()) };
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
                    let text_c = c_text(&text, "gtk4.labelNew invalid text")?;
                    let id = GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let raw = unsafe { gtk_label_new(text_c.as_ptr()) };
                        let id = state.alloc_id();
                        state.labels.insert(id, raw);
                        state.widgets.insert(id, raw);
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
                    let text_c = c_text(&text, "gtk4.labelSetText invalid text")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let label = state.labels.get(&label_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.labelSetText unknown label id {label_id}"
                            )))
                        })?;
                        unsafe { gtk_label_set_text(label, text_c.as_ptr()) };
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
                }
                Ok(effect(move |_| {
                    let id = GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let raw = unsafe { gtk_entry_new() };
                        let id = state.alloc_id();
                        state.entries.insert(id, raw);
                        state.widgets.insert(id, raw);
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
                    let text_c = c_text(&text, "gtk4.entrySetText invalid text")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let entry = state.entries.get(&entry_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.entrySetText unknown entry id {entry_id}"
                            )))
                        })?;
                        unsafe { gtk_editable_set_text(entry, text_c.as_ptr()) };
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
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let entry = state.entries.get(&entry_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.entryText unknown entry id {entry_id}"
                            )))
                        })?;
                        let text_ptr = unsafe { gtk_editable_get_text(entry) };
                        if text_ptr.is_null() {
                            return Ok(Value::Text(String::new()));
                        }
                        let text = unsafe { CStr::from_ptr(text_ptr) }
                            .to_string_lossy()
                            .into_owned();
                        Ok(Value::Text(text))
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
                    let path_c = c_text(&path, "gtk4.imageNewFromFile invalid path")?;
                    let id = GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let raw = unsafe { gtk_image_new_from_file(path_c.as_ptr()) };
                        if raw.is_null() {
                            return Err(RuntimeError::Error(Value::Text(
                                "gtk4.imageNewFromFile failed to create image".to_string(),
                            )));
                        }
                        let id = state.alloc_id();
                        state.images.insert(id, raw);
                        state.widgets.insert(id, raw);
                        Ok::<i64, RuntimeError>(id)
                    })?;
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
                    let path_c = c_text(&path, "gtk4.imageSetFile invalid path")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let image = state.images.get(&image_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.imageSetFile unknown image id {image_id}"
                            )))
                        })?;
                        unsafe { gtk_image_set_from_file(image, path_c.as_ptr()) };
                        Ok(Value::Unit)
                    })
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
                    let resource_c = c_text(
                        &resource_path,
                        "gtk4.imageNewFromResource invalid resource path",
                    )?;
                    let id = GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let raw = unsafe { gtk_image_new_from_resource(resource_c.as_ptr()) };
                        if raw.is_null() {
                            return Err(RuntimeError::Error(Value::Text(
                                "gtk4.imageNewFromResource failed to create image".to_string(),
                            )));
                        }
                        let id = state.alloc_id();
                        state.images.insert(id, raw);
                        state.widgets.insert(id, raw);
                        Ok::<i64, RuntimeError>(id)
                    })?;
                    Ok(Value::Int(id))
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
                    let resource_c = c_text(
                        &resource_path,
                        "gtk4.imageSetResource invalid resource path",
                    )?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let image = state.images.get(&image_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.imageSetResource unknown image id {image_id}"
                            )))
                        })?;
                        unsafe { gtk_image_set_from_resource(image, resource_c.as_ptr()) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        // ── widget layout primitives ──────────────────────────────────

        fields.insert(
            "widgetSetSizeRequest".to_string(),
            builtin("gtk4.widgetSetSizeRequest", 3, |mut args, _| {
                let height = match args.remove(2) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetSizeRequest expects Int height")),
                };
                let width = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetSizeRequest expects Int width")),
                };
                let widget_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetSizeRequest expects Int widget id")),
                };
                Ok(effect(move |_| {
                    let w = as_i32(width, "gtk4.widgetSetSizeRequest width")?;
                    let h = as_i32(height, "gtk4.widgetSetSizeRequest height")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let widget = widget_ptr(&state, widget_id, "widgetSetSizeRequest")?;
                        unsafe { gtk_widget_set_size_request(widget, w, h) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "widgetSetHexpand".to_string(),
            builtin("gtk4.widgetSetHexpand", 2, |mut args, _| {
                let expand = match args.remove(1) {
                    Value::Bool(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetHexpand expects Bool")),
                };
                let widget_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetHexpand expects Int widget id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let widget = widget_ptr(&state, widget_id, "widgetSetHexpand")?;
                        unsafe { gtk_widget_set_hexpand(widget, if expand { 1 } else { 0 }) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "widgetSetVexpand".to_string(),
            builtin("gtk4.widgetSetVexpand", 2, |mut args, _| {
                let expand = match args.remove(1) {
                    Value::Bool(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetVexpand expects Bool")),
                };
                let widget_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetVexpand expects Int widget id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let widget = widget_ptr(&state, widget_id, "widgetSetVexpand")?;
                        unsafe { gtk_widget_set_vexpand(widget, if expand { 1 } else { 0 }) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        // GtkAlign: FILL=0, START=1, END=2, CENTER=3, BASELINE_FILL=4, BASELINE_CENTER=5
        fields.insert(
            "widgetSetHalign".to_string(),
            builtin("gtk4.widgetSetHalign", 2, |mut args, _| {
                let align = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetHalign expects Int align")),
                };
                let widget_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetHalign expects Int widget id")),
                };
                Ok(effect(move |_| {
                    let a = as_i32(align, "gtk4.widgetSetHalign align")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let widget = widget_ptr(&state, widget_id, "widgetSetHalign")?;
                        unsafe { gtk_widget_set_halign(widget, a) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "widgetSetValign".to_string(),
            builtin("gtk4.widgetSetValign", 2, |mut args, _| {
                let align = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetValign expects Int align")),
                };
                let widget_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetValign expects Int widget id")),
                };
                Ok(effect(move |_| {
                    let a = as_i32(align, "gtk4.widgetSetValign align")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let widget = widget_ptr(&state, widget_id, "widgetSetValign")?;
                        unsafe { gtk_widget_set_valign(widget, a) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "widgetSetMarginStart".to_string(),
            builtin("gtk4.widgetSetMarginStart", 2, |mut args, _| {
                let margin = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetMarginStart expects Int margin")),
                };
                let widget_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetMarginStart expects Int widget id")),
                };
                Ok(effect(move |_| {
                    let m = as_i32(margin, "gtk4.widgetSetMarginStart margin")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let widget = widget_ptr(&state, widget_id, "widgetSetMarginStart")?;
                        unsafe { gtk_widget_set_margin_start(widget, m) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "widgetSetMarginEnd".to_string(),
            builtin("gtk4.widgetSetMarginEnd", 2, |mut args, _| {
                let margin = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetMarginEnd expects Int margin")),
                };
                let widget_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetMarginEnd expects Int widget id")),
                };
                Ok(effect(move |_| {
                    let m = as_i32(margin, "gtk4.widgetSetMarginEnd margin")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let widget = widget_ptr(&state, widget_id, "widgetSetMarginEnd")?;
                        unsafe { gtk_widget_set_margin_end(widget, m) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "widgetSetMarginTop".to_string(),
            builtin("gtk4.widgetSetMarginTop", 2, |mut args, _| {
                let margin = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetMarginTop expects Int margin")),
                };
                let widget_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetMarginTop expects Int widget id")),
                };
                Ok(effect(move |_| {
                    let m = as_i32(margin, "gtk4.widgetSetMarginTop margin")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let widget = widget_ptr(&state, widget_id, "widgetSetMarginTop")?;
                        unsafe { gtk_widget_set_margin_top(widget, m) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "widgetSetMarginBottom".to_string(),
            builtin("gtk4.widgetSetMarginBottom", 2, |mut args, _| {
                let margin = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetMarginBottom expects Int margin")),
                };
                let widget_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetMarginBottom expects Int widget id")),
                };
                Ok(effect(move |_| {
                    let m = as_i32(margin, "gtk4.widgetSetMarginBottom margin")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let widget = widget_ptr(&state, widget_id, "widgetSetMarginBottom")?;
                        unsafe { gtk_widget_set_margin_bottom(widget, m) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "widgetAddCssClass".to_string(),
            builtin("gtk4.widgetAddCssClass", 2, |mut args, _| {
                let class = match args.remove(1) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.widgetAddCssClass expects Text class")),
                };
                let widget_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetAddCssClass expects Int widget id")),
                };
                Ok(effect(move |_| {
                    let class_c = c_text(&class, "gtk4.widgetAddCssClass invalid class")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let widget = widget_ptr(&state, widget_id, "widgetAddCssClass")?;
                        unsafe { gtk_widget_add_css_class(widget, class_c.as_ptr()) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "widgetRemoveCssClass".to_string(),
            builtin("gtk4.widgetRemoveCssClass", 2, |mut args, _| {
                let class = match args.remove(1) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.widgetRemoveCssClass expects Text class")),
                };
                let widget_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetRemoveCssClass expects Int widget id")),
                };
                Ok(effect(move |_| {
                    let class_c = c_text(&class, "gtk4.widgetRemoveCssClass invalid class")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let widget = widget_ptr(&state, widget_id, "widgetRemoveCssClass")?;
                        unsafe { gtk_widget_remove_css_class(widget, class_c.as_ptr()) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "widgetSetTooltipText".to_string(),
            builtin("gtk4.widgetSetTooltipText", 2, |mut args, _| {
                let text = match args.remove(1) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetTooltipText expects Text")),
                };
                let widget_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetTooltipText expects Int widget id")),
                };
                Ok(effect(move |_| {
                    let text_c = c_text(&text, "gtk4.widgetSetTooltipText invalid text")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let widget = widget_ptr(&state, widget_id, "widgetSetTooltipText")?;
                        unsafe { gtk_widget_set_tooltip_text(widget, text_c.as_ptr()) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "widgetSetOpacity".to_string(),
            builtin("gtk4.widgetSetOpacity", 2, |mut args, _| {
                let opacity = match args.remove(1) {
                    Value::Int(v) => v as f64 / 100.0,
                    _ => return Err(invalid("gtk4.widgetSetOpacity expects Int (0-100)")),
                };
                let widget_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.widgetSetOpacity expects Int widget id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let widget = widget_ptr(&state, widget_id, "widgetSetOpacity")?;
                        unsafe { gtk_widget_set_opacity(widget, opacity) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        // ── box extras ────────────────────────────────────────────────

        fields.insert(
            "boxSetHomogeneous".to_string(),
            builtin("gtk4.boxSetHomogeneous", 2, |mut args, _| {
                let homogeneous = match args.remove(1) {
                    Value::Bool(v) => v,
                    _ => return Err(invalid("gtk4.boxSetHomogeneous expects Bool")),
                };
                let box_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.boxSetHomogeneous expects Int box id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let boxw = state.boxes.get(&box_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.boxSetHomogeneous unknown box id {box_id}"
                            )))
                        })?;
                        unsafe { gtk_box_set_homogeneous(boxw, if homogeneous { 1 } else { 0 }) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        // ── label extras ──────────────────────────────────────────────

        fields.insert(
            "labelSetWrap".to_string(),
            builtin("gtk4.labelSetWrap", 2, |mut args, _| {
                let wrap = match args.remove(1) {
                    Value::Bool(v) => v,
                    _ => return Err(invalid("gtk4.labelSetWrap expects Bool")),
                };
                let label_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.labelSetWrap expects Int label id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let label = state.labels.get(&label_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.labelSetWrap unknown label id {label_id}"
                            )))
                        })?;
                        unsafe { gtk_label_set_wrap(label, if wrap { 1 } else { 0 }) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        // PangoEllipsizeMode: NONE=0, START=1, MIDDLE=2, END=3
        fields.insert(
            "labelSetEllipsize".to_string(),
            builtin("gtk4.labelSetEllipsize", 2, |mut args, _| {
                let mode = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.labelSetEllipsize expects Int mode")),
                };
                let label_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.labelSetEllipsize expects Int label id")),
                };
                Ok(effect(move |_| {
                    let m = as_i32(mode, "gtk4.labelSetEllipsize mode")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let label = state.labels.get(&label_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.labelSetEllipsize unknown label id {label_id}"
                            )))
                        })?;
                        unsafe { gtk_label_set_ellipsize(label, m) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "labelSetXalign".to_string(),
            builtin("gtk4.labelSetXalign", 2, |mut args, _| {
                let xalign = match args.remove(1) {
                    Value::Int(v) => v as f32 / 100.0,
                    _ => return Err(invalid("gtk4.labelSetXalign expects Int (0-100)")),
                };
                let label_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.labelSetXalign expects Int label id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let label = state.labels.get(&label_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.labelSetXalign unknown label id {label_id}"
                            )))
                        })?;
                        unsafe { gtk_label_set_xalign(label, xalign) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "labelSetMaxWidthChars".to_string(),
            builtin("gtk4.labelSetMaxWidthChars", 2, |mut args, _| {
                let n = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.labelSetMaxWidthChars expects Int n")),
                };
                let label_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.labelSetMaxWidthChars expects Int label id")),
                };
                Ok(effect(move |_| {
                    let n_i32 = as_i32(n, "gtk4.labelSetMaxWidthChars n")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let label = state.labels.get(&label_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.labelSetMaxWidthChars unknown label id {label_id}"
                            )))
                        })?;
                        unsafe { gtk_label_set_max_width_chars(label, n_i32) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        // ── button icon ───────────────────────────────────────────────

        fields.insert(
            "buttonNewFromIconName".to_string(),
            builtin("gtk4.buttonNewFromIconName", 1, |mut args, _| {
                let icon_name = match args.remove(0) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.buttonNewFromIconName expects Text icon name")),
                };
                Ok(effect(move |_| {
                    let icon_c =
                        c_text(&icon_name, "gtk4.buttonNewFromIconName invalid icon name")?;
                    let id = GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let raw = unsafe { gtk_button_new_from_icon_name(icon_c.as_ptr()) };
                        let id = state.alloc_id();
                        state.buttons.insert(id, raw);
                        state.widgets.insert(id, raw);
                        id
                    });
                    Ok(Value::Int(id))
                }))
            }),
        );

        // ── image from icon name ──────────────────────────────────────

        fields.insert(
            "imageNewFromIconName".to_string(),
            builtin("gtk4.imageNewFromIconName", 1, |mut args, _| {
                let icon_name = match args.remove(0) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.imageNewFromIconName expects Text icon name")),
                };
                Ok(effect(move |_| {
                    let icon_c = c_text(&icon_name, "gtk4.imageNewFromIconName invalid icon name")?;
                    let id = GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let raw = unsafe { gtk_image_new_from_icon_name(icon_c.as_ptr()) };
                        let id = state.alloc_id();
                        state.images.insert(id, raw);
                        state.widgets.insert(id, raw);
                        id
                    });
                    Ok(Value::Int(id))
                }))
            }),
        );

        fields.insert(
            "imageSetPixelSize".to_string(),
            builtin("gtk4.imageSetPixelSize", 2, |mut args, _| {
                let size = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.imageSetPixelSize expects Int size")),
                };
                let image_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.imageSetPixelSize expects Int image id")),
                };
                Ok(effect(move |_| {
                    let s = as_i32(size, "gtk4.imageSetPixelSize size")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let image = state.images.get(&image_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.imageSetPixelSize unknown image id {image_id}"
                            )))
                        })?;
                        unsafe { gtk_image_set_pixel_size(image, s) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        // ── icon theme search path (real) ─────────────────────────────

        fields.insert(
            "iconThemeAddSearchPath".to_string(),
            builtin("gtk4.iconThemeAddSearchPath", 1, |mut args, _| {
                let path = match args.remove(0) {
                    Value::Text(text) => text,
                    _ => return Err(invalid("gtk4.iconThemeAddSearchPath expects Text path")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        state.pending_icon_search_paths.push(path.clone());
                        if !state.windows.is_empty() {
                            apply_pending_display_customizations(&mut state)?;
                        }
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        // ── button set child (real) ───────────────────────────────────

        fields.insert(
            "buttonSetChild".to_string(),
            builtin("gtk4.buttonSetChild", 2, |mut args, _| {
                let child_id = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.buttonSetChild expects Int child id")),
                };
                let button_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.buttonSetChild expects Int button id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let button = state.buttons.get(&button_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.buttonSetChild unknown button id {button_id}"
                            )))
                        })?;
                        let child = state.widgets.get(&child_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.buttonSetChild unknown child id {child_id}"
                            )))
                        })?;
                        unsafe { gtk_button_set_child(button, child) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        // ── scrolled window (real) ────────────────────────────────────

        fields.insert(
            "scrollAreaNew".to_string(),
            builtin("gtk4.scrollAreaNew", 1, |mut args, _| {
                match args.remove(0) {
                    Value::Unit => {}
                    _ => return Err(invalid("gtk4.scrollAreaNew expects Unit")),
                }
                Ok(effect(move |_| {
                    let id = GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let raw = unsafe { gtk_scrolled_window_new() };
                        let id = state.alloc_id();
                        state.scrolled_windows.insert(id, raw);
                        state.widgets.insert(id, raw);
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
                    _ => return Err(invalid("gtk4.scrollAreaSetChild expects Int scroll id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let scrolled =
                            state
                                .scrolled_windows
                                .get(&scroll_id)
                                .copied()
                                .ok_or_else(|| {
                                    RuntimeError::Error(Value::Text(format!(
                                        "gtk4.scrollAreaSetChild unknown scroll id {scroll_id}"
                                    )))
                                })?;
                        let child = widget_ptr(&state, child_id, "scrollAreaSetChild")?;
                        unsafe { gtk_scrolled_window_set_child(scrolled, child) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        // GtkPolicyType: AUTOMATIC=0, ALWAYS=1, NEVER=2, EXTERNAL=3
        fields.insert(
            "scrollAreaSetPolicy".to_string(),
            builtin("gtk4.scrollAreaSetPolicy", 3, |mut args, _| {
                let vpolicy = match args.remove(2) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.scrollAreaSetPolicy expects Int vpolicy")),
                };
                let hpolicy = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.scrollAreaSetPolicy expects Int hpolicy")),
                };
                let scroll_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.scrollAreaSetPolicy expects Int scroll id")),
                };
                Ok(effect(move |_| {
                    let hp = as_i32(hpolicy, "gtk4.scrollAreaSetPolicy hpolicy")?;
                    let vp = as_i32(vpolicy, "gtk4.scrollAreaSetPolicy vpolicy")?;
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let scrolled =
                            state
                                .scrolled_windows
                                .get(&scroll_id)
                                .copied()
                                .ok_or_else(|| {
                                    RuntimeError::Error(Value::Text(format!(
                                        "gtk4.scrollAreaSetPolicy unknown scroll id {scroll_id}"
                                    )))
                                })?;
                        unsafe { gtk_scrolled_window_set_policy(scrolled, hp, vp) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        // ── separator ─────────────────────────────────────────────────

        fields.insert(
            "separatorNew".to_string(),
            builtin("gtk4.separatorNew", 1, |mut args, _| {
                let orientation = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.separatorNew expects Int orientation")),
                };
                Ok(effect(move |_| {
                    let ori = if orientation == 1 { 1 } else { 0 };
                    let id = GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let raw = unsafe { gtk_separator_new(ori) };
                        let id = state.alloc_id();
                        state.separators.insert(id, raw);
                        state.widgets.insert(id, raw);
                        id
                    });
                    Ok(Value::Int(id))
                }))
            }),
        );

        // ── overlay ───────────────────────────────────────────────────

        fields.insert(
            "overlayNew".to_string(),
            builtin("gtk4.overlayNew", 1, |mut args, _| {
                match args.remove(0) {
                    Value::Unit => {}
                    _ => return Err(invalid("gtk4.overlayNew expects Unit")),
                }
                Ok(effect(move |_| {
                    let id = GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let raw = unsafe { gtk_overlay_new() };
                        let id = state.alloc_id();
                        state.overlays.insert(id, raw);
                        state.widgets.insert(id, raw);
                        id
                    });
                    Ok(Value::Int(id))
                }))
            }),
        );

        fields.insert(
            "overlaySetChild".to_string(),
            builtin("gtk4.overlaySetChild", 2, |mut args, _| {
                let child_id = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.overlaySetChild expects Int child id")),
                };
                let overlay_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.overlaySetChild expects Int overlay id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let overlay =
                            state.overlays.get(&overlay_id).copied().ok_or_else(|| {
                                RuntimeError::Error(Value::Text(format!(
                                    "gtk4.overlaySetChild unknown overlay id {overlay_id}"
                                )))
                            })?;
                        let child = widget_ptr(&state, child_id, "overlaySetChild")?;
                        unsafe { gtk_overlay_set_child(overlay, child) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "overlayAddOverlay".to_string(),
            builtin("gtk4.overlayAddOverlay", 2, |mut args, _| {
                let child_id = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.overlayAddOverlay expects Int child id")),
                };
                let overlay_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.overlayAddOverlay expects Int overlay id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let overlay =
                            state.overlays.get(&overlay_id).copied().ok_or_else(|| {
                                RuntimeError::Error(Value::Text(format!(
                                    "gtk4.overlayAddOverlay unknown overlay id {overlay_id}"
                                )))
                            })?;
                        let child = widget_ptr(&state, child_id, "overlayAddOverlay")?;
                        unsafe { gtk_overlay_add_overlay(overlay, child) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        // ── CSS provider (app-level stylesheet) ───────────────────────

        fields.insert(
            "appSetCss".to_string(),
            builtin("gtk4.appSetCss", 2, |mut args, _| {
                let css_text = match args.remove(1) {
                    Value::Text(v) => v,
                    Value::Record(_) => {
                        // Accept record but treat as no-op for now
                        return Ok(effect(|_| Ok(Value::Unit)));
                    }
                    _ => return Err(invalid("gtk4.appSetCss expects Text css or Record")),
                };
                let _app_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.appSetCss expects Int app id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        state.pending_css_texts.push(css_text.clone());
                        if !state.windows.is_empty() {
                            apply_pending_display_customizations(&mut state)?;
                        }
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        // ── tray icon (StatusNotifierItem via zbus) ──────────────────

        fields.insert(
            "trayIconNew".to_string(),
            builtin("gtk4.trayIconNew", 2, |mut args, _| {
                let tooltip = match args.remove(1) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.trayIconNew expects Text tooltip")),
                };
                let icon_name = match args.remove(0) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.trayIconNew expects Text icon_name")),
                };
                Ok(effect(move |_| {
                    let state = Arc::new(Mutex::new(SniTrayState {
                        icon_name: icon_name.clone(),
                        tooltip: tooltip.clone(),
                        visible: true,
                        menu_items: Vec::new(),
                    }));
                    if let Err(e) = spawn_sni_tray(state.clone()) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.trayIconNew: {e}"
                        ))));
                    }
                    let id = GTK_STATE.with(|s| {
                        let mut s = s.borrow_mut();
                        let id = s.alloc_id();
                        s.tray_handles.insert(id, state);
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
                    _ => return Err(invalid("gtk4.trayIconSetTooltip expects Text")),
                };
                let tray_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.trayIconSetTooltip expects Int")),
                };
                Ok(effect(move |_| {
                    let arc = GTK_STATE.with(|s| {
                        s.borrow().tray_handles.get(&tray_id).cloned()
                    });
                    if let Some(arc) = arc {
                        if let Ok(mut ts) = arc.lock() {
                            ts.tooltip = tooltip.clone();
                        }
                    }
                    Ok(Value::Unit)
                }))
            }),
        );

        fields.insert(
            "trayIconSetVisible".to_string(),
            builtin("gtk4.trayIconSetVisible", 2, |mut args, _| {
                let visible = match args.remove(1) {
                    Value::Bool(v) => v,
                    Value::Constructor { ref name, .. } => name == "True",
                    _ => return Err(invalid("gtk4.trayIconSetVisible expects Bool")),
                };
                let tray_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.trayIconSetVisible expects Int")),
                };
                Ok(effect(move |_| {
                    let arc = GTK_STATE.with(|s| {
                        s.borrow().tray_handles.get(&tray_id).cloned()
                    });
                    if let Some(arc) = arc {
                        if let Ok(mut ts) = arc.lock() {
                            ts.visible = visible;
                        }
                    }
                    Ok(Value::Unit)
                }))
            }),
        );

        fields.insert(
            "trayIconSetMenuItems".to_string(),
            builtin("gtk4.trayIconSetMenuItems", 2, |mut args, _| {
                let items_val = args.remove(1);
                let tray_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.trayIconSetMenuItems expects Int tray_id")),
                };
                // Parse list of {label, action} records
                let mut items: Vec<(String, String)> = Vec::new();
                let mut list = items_val;
                loop {
                    match list {
                        Value::Constructor { name, mut args } if name == "Cons" => {
                            let head = args.remove(0);
                            list = args.remove(0);
                            if let Value::Record(fields) = head {
                                let label = fields.get("label")
                                    .and_then(|v| if let Value::Text(t) = v { Some(t.clone()) } else { None })
                                    .unwrap_or_default();
                                let action = fields.get("action")
                                    .and_then(|v| if let Value::Text(t) = v { Some(t.clone()) } else { None })
                                    .unwrap_or_default();
                                items.push((label, action));
                            }
                        }
                        Value::Constructor { name, .. } if name == "Nil" || name == "Empty" => break,
                        _ => break,
                    }
                }
                Ok(effect(move |_| {
                    GTK_STATE.with(|s| {
                        let s = s.borrow();
                        if let Some(handle) = s.tray_handles.get(&tray_id) {
                            if let Ok(mut ts) = handle.lock() {
                                ts.menu_items = items.clone();
                            }
                        }
                        // Signal the tokio thread to emit LayoutUpdated on the DBusMenu.
                        if let Ok(mut flag) = pending_layout_update().lock() {
                            *flag = true;
                        }
                        Ok(Value::Unit)
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
                }
                Ok(effect(move |_| {
                    let id = GTK_STATE.with(|s| {
                        let mut s = s.borrow_mut();
                        s.alloc_id()
                    });
                    Ok(Value::Int(id))
                }))
            }),
        );

        fields.insert(
            "menuModelAppendItem".to_string(),
            builtin("gtk4.menuModelAppendItem", 3, |mut args, _| {
                let _action = match args.remove(2) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.menuModelAppendItem expects Text")),
                };
                let _label = match args.remove(1) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.menuModelAppendItem expects Text")),
                };
                let _menu_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.menuModelAppendItem expects Int")),
                };
                Ok(effect(move |_| Ok(Value::Unit)))
            }),
        );

        fields.insert(
            "osSetBadgeCount".to_string(),
            builtin("gtk4.osSetBadgeCount", 2, |mut args, _| {
                let count = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.osSetBadgeCount expects Int")),
                };
                let _app_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.osSetBadgeCount expects Int")),
                };
                Ok(effect(move |_| {
                    let arc = GTK_STATE.with(|s| {
                        s.borrow().tray_handles.values().last().cloned()
                    });
                    if let Some(arc) = arc {
                        if let Ok(mut ts) = arc.lock() {
                            ts.tooltip = if count > 0 {
                                format!("Mailfox ({count} unread)")
                            } else {
                                "Mailfox".to_string()
                            };
                        }
                    }
                    Ok(Value::Unit)
                }))
            }),
        );

        fields.insert(
            "buildFromNode".to_string(),
            builtin("gtk4.buildFromNode", 1, |mut args, _| {
                let node = args.remove(0);
                Ok(effect(move |_| {
                    let decoded = decode_gtk_node(&node)?;
                    let id = GTK_STATE.with(|state| -> Result<i64, RuntimeError> {
                        let mut state = state.borrow_mut();
                        let mut id_map = HashMap::new();
                        let root = first_object_in_interface(&decoded)?;
                        let (id, live) = build_widget_from_node_real(&mut state, root, &mut id_map)?;
                        state.named_widgets.extend(id_map.clone());
                        for (name, wid) in &id_map {
                            state.widget_id_to_name.insert(*wid, name.clone());
                        }
                        state.live_trees.insert(id, live);
                        Ok(id)
                    })?;
                    Ok(Value::Int(id))
                }))
            }),
        );

        fields.insert(
            "buildWithIds".to_string(),
            builtin("gtk4.buildWithIds", 1, |mut args, _| {
                let node = args.remove(0);
                Ok(effect(move |_| {
                    let decoded = decode_gtk_node(&node)?;
                    let (root_id, id_map) =
                        GTK_STATE.with(|state| -> Result<(i64, HashMap<String, i64>), RuntimeError> {
                            let mut state = state.borrow_mut();
                            let mut id_map = HashMap::new();
                            let root = first_object_in_interface(&decoded)?;
                            let (id, live) = build_widget_from_node_real(&mut state, root, &mut id_map)?;
                            state.named_widgets.extend(id_map.clone());
                            for (name, wid) in &id_map {
                                state.widget_id_to_name.insert(*wid, name.clone());
                            }
                            state.live_trees.insert(id, live);
                            Ok((id, id_map))
                        })?;
                    let mut widgets_map = im::HashMap::new();
                    for (name, wid) in id_map {
                        widgets_map.insert(
                            crate::runtime::values::KeyValue::Text(name),
                            Value::Int(wid),
                        );
                    }
                    let mut record = HashMap::new();
                    record.insert("root".to_string(), Value::Int(root_id));
                    record.insert("widgets".to_string(), Value::Map(Arc::new(widgets_map)));
                    Ok(Value::Record(Arc::new(record)))
                }))
            }),
        );

        fields.insert(
            "reconcileNode".to_string(),
            builtin("gtk4.reconcileNode", 2, |mut args, _| {
                let new_node_val = args.remove(1);
                let root_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.reconcileNode expects Int root widget id")),
                };
                Ok(effect(move |_| {
                    let new_decoded = decode_gtk_node(&new_node_val)?;
                    let new_root = first_object_in_interface(&new_decoded)?;
                    let result_id = GTK_STATE.with(|state| -> Result<i64, RuntimeError> {
                        let mut state = state.borrow_mut();
                        let mut id_map: HashMap<String, i64> = HashMap::new();
                        let mut live = state.live_trees.remove(&root_id).ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.reconcileNode no live tree for root id {root_id}"
                            )))
                        })?;
                        let patched = reconcile_node(&mut state, &mut live, new_root, &mut id_map)?;
                        let final_id = if !patched {
                            // Root widget type changed — rebuild entirely
                            let old_live = live;
                            cleanup_widget_state(&mut state, &old_live);
                            let (new_id, new_live) =
                                build_widget_from_node_real(&mut state, new_root, &mut id_map)?;
                            state.live_trees.insert(new_id, new_live);
                            new_id
                        } else {
                            state.live_trees.insert(root_id, live);
                            root_id
                        };
                        state.named_widgets.extend(id_map.clone());
                        for (name, wid) in &id_map {
                            state.widget_id_to_name.insert(*wid, name.clone());
                        }
                        Ok(final_id)
                    })?;
                    Ok(Value::Int(result_id))
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
                    GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        if let Some(event) = state.signal_events.pop_front() {
                            let widget_name = state
                                .widget_id_to_name
                                .get(&event.widget_id)
                                .cloned()
                                .unwrap_or_default();
                            Ok(Value::Constructor {
                                name: "Some".to_string(),
                                args: vec![make_signal_event_value(event, widget_name)],
                            })
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
            "signalStream".to_string(),
            builtin("gtk4.signalStream", 1, |mut args, _| {
                match args.remove(0) {
                    Value::Unit => {}
                    _ => return Err(invalid("gtk4.signalStream expects Unit")),
                }
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let (sender, receiver) = mpsc::sync_channel(512);
                        let inner = Arc::new(ChannelInner {
                            sender: Mutex::new(None),
                            receiver: Mutex::new(receiver),
                            closed: AtomicBool::new(false),
                        });
                        state.signal_senders.push(sender);
                        Ok(Value::ChannelRecv(Arc::new(ChannelRecv { inner })))
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
                    GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let _ = widget_ptr(&state, widget_id, "signalEmit")?;
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
            "widgetById".to_string(),
            builtin("gtk4.widgetById", 1, |mut args, _| {
                let name = match args.remove(0) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.widgetById expects Text widget id name")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let id = state.named_widgets.get(&name).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.widgetById unknown named widget '{name}'"
                            )))
                        })?;
                        Ok(Value::Int(id))
                    })
                }))
            }),
        );

        fields.insert(
            "signalBindBoolProperty".to_string(),
            builtin("gtk4.signalBindBoolProperty", 4, |mut args, _| {
                let value = match args.remove(3) {
                    Value::Bool(v) => v,
                    _ => return Err(invalid("gtk4.signalBindBoolProperty expects Bool value")),
                };
                let prop_name = match args.remove(2) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.signalBindBoolProperty expects Text property name")),
                };
                let target_widget_id = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.signalBindBoolProperty expects Int target widget id")),
                };
                let handler_name = match args.remove(0) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.signalBindBoolProperty expects Text handler name")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let _ = widget_ptr(&state, target_widget_id, "signalBindBoolProperty")?;
                        state.signal_bool_bindings
                            .entry(handler_name.clone())
                            .or_default()
                            .push(SignalBoolBinding {
                                widget_id: target_widget_id,
                                property: prop_name.clone(),
                                value,
                            });
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "signalBindCssClass".to_string(),
            builtin("gtk4.signalBindCssClass", 4, |mut args, _| {
                let add = match args.remove(3) {
                    Value::Bool(v) => v,
                    _ => return Err(invalid("gtk4.signalBindCssClass expects Bool add")),
                };
                let class_name = match args.remove(2) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.signalBindCssClass expects Text class name")),
                };
                let target_widget_id = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.signalBindCssClass expects Int target widget id")),
                };
                let handler_name = match args.remove(0) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.signalBindCssClass expects Text handler name")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let _ = widget_ptr(&state, target_widget_id, "signalBindCssClass")?;
                        state.signal_css_bindings
                            .entry(handler_name.clone())
                            .or_default()
                            .push(SignalCssBinding {
                                widget_id: target_widget_id,
                                class_name: class_name.clone(),
                                add,
                            });
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "signalBindToggleBoolProperty".to_string(),
            builtin("gtk4.signalBindToggleBoolProperty", 3, |mut args, _| {
                let prop_name = match args.remove(2) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.signalBindToggleBoolProperty expects Text property name")),
                };
                let target_widget_id = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.signalBindToggleBoolProperty expects Int target widget id")),
                };
                let handler_name = match args.remove(0) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.signalBindToggleBoolProperty expects Text handler name")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let _ = widget_ptr(&state, target_widget_id, "signalBindToggleBoolProperty")?;
                        state.signal_toggle_bool_bindings
                            .entry(handler_name.clone())
                            .or_default()
                            .push(SignalToggleBoolBinding {
                                widget_id: target_widget_id,
                                property: prop_name.clone(),
                            });
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "signalToggleCssClass".to_string(),
            builtin("gtk4.signalToggleCssClass", 3, |mut args, _| {
                let class_name = match args.remove(2) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.signalToggleCssClass expects Text class name")),
                };
                let target_widget_id = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.signalToggleCssClass expects Int target widget id")),
                };
                let handler_name = match args.remove(0) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.signalToggleCssClass expects Text handler name")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let _ = widget_ptr(&state, target_widget_id, "signalToggleCssClass")?;
                        state.signal_toggle_css_bindings
                            .entry(handler_name.clone())
                            .or_default()
                            .push(SignalToggleCssBinding {
                                widget_id: target_widget_id,
                                class_name: class_name.clone(),
                            });
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "dialogNew".to_string(),
            builtin("gtk4.dialogNew", 1, |mut args, _| {
                let _app_id = args.remove(0);
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let window = unsafe { gtk_window_new() };
                        if window.is_null() {
                            return Err(RuntimeError::Error(Value::Text(
                                "gtk4.dialogNew failed to create window".to_string(),
                            )));
                        }
                        unsafe { gtk_window_set_modal(window, 1) };
                        let id = state.alloc_id();
                        state.windows.insert(id, window);
                        state.widgets.insert(id, window);
                        Ok(Value::Int(id))
                    })
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
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let dialog = widget_ptr(&state, dialog_id, "dialogSetTitle")?;
                        let title_c = c_text(&title, "gtk4.dialogSetTitle invalid title")?;
                        unsafe { gtk_window_set_title(dialog, title_c.as_ptr()) };
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
                    _ => return Err(invalid("gtk4.dialogSetChild expects Int child widget id")),
                };
                let dialog_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.dialogSetChild expects Int dialog id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        eprintln!("[DEBUG] dialogSetChild: dialog_id={dialog_id}, child_id={child_id}");
                        let dialog = widget_ptr(&state, dialog_id, "dialogSetChild")?;
                        let child = widget_ptr(&state, child_id, "dialogSetChild")?;
                        eprintln!("[DEBUG] dialogSetChild: dialog_ptr={dialog:?}, child_ptr={child:?}");
                        unsafe { gtk_window_set_child(dialog, child) };
                        eprintln!("[DEBUG] dialogSetChild: gtk_window_set_child called successfully");
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "dialogPresent".to_string(),
            builtin("gtk4.dialogPresent", 2, |mut args, _| {
                let dialog_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.dialogPresent expects Int dialog id")),
                };
                let parent_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.dialogPresent expects Int parent window id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        eprintln!("[DEBUG] dialogPresent: dialog_id={dialog_id}, parent_id={parent_id}");
                        let dialog = widget_ptr(&state, dialog_id, "dialogPresent")?;
                        let parent = widget_ptr(&state, parent_id, "dialogPresent")?;
                        eprintln!("[DEBUG] dialogPresent: dialog_ptr={dialog:?}, parent_ptr={parent:?}");
                        unsafe {
                            gtk_window_set_transient_for(dialog, parent);
                            gtk_window_present(dialog);
                        }
                        eprintln!("[DEBUG] dialogPresent: presented successfully");
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
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let dialog = widget_ptr(&state, dialog_id, "dialogClose")?;
                        unsafe { gtk_window_close(dialog) };
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "adwDialogPresent".to_string(),
            builtin("gtk4.adwDialogPresent", 2, |mut args, _| {
                let dialog_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.adwDialogPresent expects Int dialog widget id")),
                };
                let parent_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.adwDialogPresent expects Int parent window id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let dialog = widget_ptr(&state, dialog_id, "adwDialogPresent")?;
                        let parent = widget_ptr(&state, parent_id, "adwDialogPresent")?;
                        call_adw_fn_pp("adw_dialog_present", dialog, parent);
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "windowOnClose".to_string(),
            builtin("gtk4.windowOnClose", 2, |mut args, _| {
                let signal_name = match args.remove(1) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.windowOnClose expects Text signal name")),
                };
                let window_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.windowOnClose expects Int window id")),
                };

                // Returns gboolean: 0 = allow close, 1 = inhibit
                unsafe extern "C" fn on_close_request(
                    _instance: *mut c_void,
                    data: *mut c_void,
                ) -> c_int {
                    if data.is_null() {
                        return 0;
                    }
                    let signal_name = unsafe { &*(data as *const String) };
                    GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let event = Value::Constructor {
                            name: "GtkUnknownSignal".to_string(),
                            args: vec![
                                Value::Int(0),
                                Value::Text(signal_name.clone()),
                                Value::Text(String::new()),
                                Value::Text(String::new()),
                            ],
                        };
                        state.signal_senders.retain(|s| s.try_send(event.clone()).is_ok());
                    });
                    0 // allow the close to proceed
                }

                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let window = widget_ptr(&state, window_id, "windowOnClose")?;
                        let name_box = Box::new(signal_name.clone());
                        let data_ptr = Box::into_raw(name_box) as *mut c_void;
                        let sig = CString::new("close-request").map_err(|_| {
                            invalid("gtk4.windowOnClose: invalid signal name")
                        })?;
                        unsafe {
                            g_signal_connect_data(
                                window,
                                sig.as_ptr(),
                                on_close_request as *const c_void,
                                data_ptr,
                                std::ptr::null_mut(),
                                0,
                            );
                        }
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "signalBindDialogPresent".to_string(),
            builtin("gtk4.signalBindDialogPresent", 3, |mut args, _| {
                let parent_id = match args.remove(2) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.signalBindDialogPresent expects Int parent window id")),
                };
                let dialog_id = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.signalBindDialogPresent expects Int dialog id")),
                };
                let handler_name = match args.remove(0) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.signalBindDialogPresent expects Text handler name")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let _ = widget_ptr(&state, dialog_id, "signalBindDialogPresent")?;
                        let _ = widget_ptr(&state, parent_id, "signalBindDialogPresent")?;
                        state.signal_dialog_bindings
                            .entry(handler_name.clone())
                            .or_default()
                            .push(SignalDialogBinding { dialog_id, parent_id });
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "signalBindStackPage".to_string(),
            builtin("gtk4.signalBindStackPage", 3, |mut args, _| {
                let page_name = match args.remove(2) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.signalBindStackPage expects Text page name")),
                };
                let stack_id = match args.remove(1) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.signalBindStackPage expects Int stack id")),
                };
                let handler_name = match args.remove(0) {
                    Value::Text(v) => v,
                    _ => return Err(invalid("gtk4.signalBindStackPage expects Text handler name")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        let _ = widget_ptr(&state, stack_id, "signalBindStackPage")?;
                        state.signal_stack_page_bindings
                            .entry(handler_name.clone())
                            .or_default()
                            .push(SignalStackPageBinding {
                                stack_id,
                                page_name: page_name.clone(),
                            });
                        Ok(Value::Unit)
                    })
                }))
            }),
        );

        fields.insert(
            "serializeSignal".to_string(),
            builtin("gtk4.serializeSignal", 1, |mut args, _| {
                let val = args.pop().unwrap();
                Ok(Value::Text(serialize_signal_value(&val)))
            }),
        );

        fields
    }
}

/// Drives one iteration of the GTK/GLib main context from any call site
/// (notably `channel.recv`).  No-op on non-GTK platforms.
#[cfg(all(feature = "gtk4-libadwaita", target_os = "linux"))]
pub(super) fn pump_gtk_events() {
    linux::pump_gtk_events();
}

#[cfg(not(all(feature = "gtk4-libadwaita", target_os = "linux")))]
pub(super) fn pump_gtk_events() {}

pub(super) fn build_gtk4_record_real(build_mock: fn() -> Value) -> Option<Value> {
    #[cfg(all(feature = "gtk4-libadwaita", target_os = "linux"))]
    {
        let Value::Record(existing) = build_mock() else {
            return None;
        };
        let fields = linux::build_from_mock((*existing).clone());
        return Some(Value::Record(std::sync::Arc::new(fields)));
    }

    #[allow(unreachable_code)]
    {
        let _ = build_mock;
        None
    }
}
