use crate::Value;

#[cfg(all(feature = "gtk4-libadwaita", target_os = "linux"))]
mod linux {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_int, c_ulong, c_void};
    use std::ptr::null_mut;
    use std::sync::Arc;

    use super::super::util::builtin;
    use crate::{EffectValue, RuntimeError, Value};

    #[link(name = "gtk-4")]
    unsafe extern "C" {
        fn gtk_init();
        fn gtk_application_new(application_id: *const c_char, flags: c_int) -> *mut c_void;
        fn gtk_application_window_new(application: *mut c_void) -> *mut c_void;
        fn gtk_window_set_title(window: *mut c_void, title: *const c_char);
        fn gtk_window_set_default_size(window: *mut c_void, width: c_int, height: c_int);
        fn gtk_window_set_titlebar(window: *mut c_void, titlebar: *mut c_void);
        fn gtk_window_set_child(window: *mut c_void, child: *mut c_void);
        fn gtk_window_present(window: *mut c_void);

        fn gtk_widget_set_visible(widget: *mut c_void, visible: c_int);
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
        fn gtk_widget_set_tooltip_text(widget: *mut c_void, text: *const c_char);
        fn gtk_widget_queue_draw(widget: *mut c_void);
        fn gtk_widget_set_opacity(widget: *mut c_void, opacity: f64);

        fn gtk_box_new(orientation: c_int, spacing: c_int) -> *mut c_void;
        fn gtk_box_append(container: *mut c_void, child: *mut c_void);
        fn gtk_box_set_homogeneous(boxw: *mut c_void, homogeneous: c_int);

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
        fn gtk_editable_set_text(editable: *mut c_void, text: *const c_char);
        fn gtk_editable_get_text(editable: *mut c_void) -> *const c_char;

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
        fn gtk_scrolled_window_set_propagate_natural_width(
            scrolled: *mut c_void,
            propagate: c_int,
        );

        fn gtk_separator_new(orientation: c_int) -> *mut c_void;

        fn gtk_overlay_new() -> *mut c_void;
        fn gtk_overlay_set_child(overlay: *mut c_void, child: *mut c_void);
        fn gtk_overlay_add_overlay(overlay: *mut c_void, widget: *mut c_void);

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
        fn gtk_icon_theme_add_search_path(
            icon_theme: *mut c_void,
            path: *const c_char,
        );
        fn gtk_button_set_child(button: *mut c_void, child: *mut c_void);

        fn gdk_display_get_default() -> *mut c_void;
    }

    #[link(name = "gio-2.0")]
    unsafe extern "C" {
        fn g_application_register(
            application: *mut c_void,
            cancellable: *mut c_void,
            error: *mut *mut c_void,
        ) -> c_int;
        fn g_application_run(
            application: *mut c_void,
            argc: c_int,
            argv: *mut *mut c_char,
        ) -> c_int;
        fn g_resource_load(filename: *const c_char, error: *mut *mut c_void) -> *mut c_void;
        fn g_resources_register(resource: *mut c_void);
    }

    #[link(name = "gobject-2.0")]
    unsafe extern "C" {
        fn g_signal_connect_data(
            instance: *mut c_void,
            detailed_signal: *const c_char,
            c_handler: *const c_void,
            data: *mut c_void,
            destroy_data: *mut c_void,
            connect_flags: c_int,
        ) -> c_ulong;
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
    }

    #[derive(Default)]
    struct RealGtkState {
        next_id: i64,
        apps: HashMap<i64, *mut c_void>,
        windows: HashMap<i64, *mut c_void>,
        widgets: HashMap<i64, *mut c_void>,
        boxes: HashMap<i64, *mut c_void>,
        buttons: HashMap<i64, *mut c_void>,
        labels: HashMap<i64, *mut c_void>,
        entries: HashMap<i64, *mut c_void>,
        images: HashMap<i64, *mut c_void>,
        draw_areas: HashMap<i64, *mut c_void>,
        scrolled_windows: HashMap<i64, *mut c_void>,
        overlays: HashMap<i64, *mut c_void>,
        separators: HashMap<i64, *mut c_void>,
        gesture_clicks: HashMap<i64, GestureClickState>,
        resources_registered: bool,
    }

    struct GestureClickState {
        widget_id: i64,
        raw: *mut c_void,
        last_button: i64,
    }

    impl RealGtkState {
        fn alloc_id(&mut self) -> i64 {
            self.next_id += 1;
            self.next_id
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

    fn as_i32(value: i64, what: &str) -> Result<i32, RuntimeError> {
        i32::try_from(value).map_err(|_| invalid(what))
    }

    fn c_text(text: &str, what: &str) -> Result<CString, RuntimeError> {
        CString::new(text.as_bytes()).map_err(|_| invalid(what))
    }

    fn widget_ptr(state: &RealGtkState, id: i64, ctx: &str) -> Result<*mut c_void, RuntimeError> {
        state.widgets.get(&id).copied().ok_or_else(|| {
            RuntimeError::Error(Value::Text(format!("gtk4.{ctx} unknown widget id {id}")))
        })
    }

    fn try_adw_init() {
        const RTLD_NOW: c_int = 2;
        const RTLD_NODELETE: c_int = 0x1000;
        let symbol = CString::new("adw_init").expect("adw_init symbol");
        for lib_name in ["libadwaita-1.so.0", "libadwaita-1.so"] {
            let Ok(name) = CString::new(lib_name) else {
                continue;
            };
            let handle = unsafe { dlopen(name.as_ptr(), RTLD_NOW | RTLD_NODELETE) };
            if handle.is_null() {
                continue;
            }
            let init_ptr = unsafe { dlsym(handle, symbol.as_ptr()) };
            if !init_ptr.is_null() {
                let init: unsafe extern "C" fn() = unsafe { std::mem::transmute(init_ptr) };
                unsafe { init() };
            }
            let _ = unsafe { dlclose(handle) };
            break;
        }
    }

    fn maybe_register_gresource_bundle() -> Result<(), RuntimeError> {
        const GRESOURCE_ENV: &str = "AIVI_GTK4_GRESOURCE_PATH";
        let path = match std::env::var(GRESOURCE_ENV) {
            Ok(path) => path,
            Err(std::env::VarError::NotPresent) => return Ok(()),
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(RuntimeError::Error(Value::Text(format!(
                    "{GRESOURCE_ENV} must be valid UTF-8"
                ))))
            }
        };
        if path.is_empty() {
            return Err(RuntimeError::Error(Value::Text(format!(
                "{GRESOURCE_ENV} cannot be empty"
            ))));
        }
        let path_c = c_text(
            &path,
            "gtk4.init invalid gresource path from AIVI_GTK4_GRESOURCE_PATH",
        )?;
        let mut err = null_mut();
        let resource = unsafe { g_resource_load(path_c.as_ptr(), &mut err) };
        if resource.is_null() {
            return Err(RuntimeError::Error(Value::Text(format!(
                "gtk4.init failed to load gresource bundle from {path}"
            ))));
        }
        unsafe { g_resources_register(resource) };
        Ok(())
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
                    let registered = unsafe { g_application_register(raw, null_mut(), null_mut()) };
                    if registered == 0 {
                        return Err(RuntimeError::Error(Value::Text(
                            "gtk4.appNew failed to register GTK application".to_string(),
                        )));
                    }
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
                        let _app = state.apps.get(&app_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.windowNew unknown app id {app_id}"
                            )))
                        })?;
                        let window = unsafe { gtk_application_window_new(_app) };
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
            "appRun".to_string(),
            builtin("gtk4.appRun", 1, |mut args, _| {
                let app_id = match args.remove(0) {
                    Value::Int(v) => v,
                    _ => return Err(invalid("gtk4.appRun expects Int app id")),
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let app = state.apps.get(&app_id).copied().ok_or_else(|| {
                            RuntimeError::Error(Value::Text(format!(
                                "gtk4.appRun unknown app id {app_id}"
                            )))
                        })?;
                        unsafe {
                            let _ = g_application_run(app, 0, null_mut());
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
                        unsafe {
                            gtk_box_set_homogeneous(boxw, if homogeneous { 1 } else { 0 })
                        };
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
                    let icon_c =
                        c_text(&icon_name, "gtk4.imageNewFromIconName invalid icon name")?;
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
                    _ => {
                        return Err(invalid(
                            "gtk4.iconThemeAddSearchPath expects Text path",
                        ))
                    }
                };
                Ok(effect(move |_| {
                    let path_c =
                        c_text(&path, "gtk4.iconThemeAddSearchPath invalid path")?;
                    unsafe {
                        let display = gdk_display_get_default();
                        if display.is_null() {
                            return Err(RuntimeError::Error(Value::Text(
                                "gtk4.iconThemeAddSearchPath no default display"
                                    .to_string(),
                            )));
                        }
                        let theme = gtk_icon_theme_get_for_display(display);
                        gtk_icon_theme_add_search_path(theme, path_c.as_ptr());
                    }
                    Ok(Value::Unit)
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
                    _ => {
                        return Err(invalid("gtk4.buttonSetChild expects Int button id"))
                    }
                };
                Ok(effect(move |_| {
                    GTK_STATE.with(|state| {
                        let state = state.borrow();
                        let button =
                            state.buttons.get(&button_id).copied().ok_or_else(|| {
                                RuntimeError::Error(Value::Text(format!(
                                    "gtk4.buttonSetChild unknown button id {button_id}"
                                )))
                            })?;
                        let child =
                            state.widgets.get(&child_id).copied().ok_or_else(|| {
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
                    let css_c = c_text(&css_text, "gtk4.appSetCss invalid css")?;
                    let display = unsafe { gdk_display_get_default() };
                    if display.is_null() {
                        return Err(RuntimeError::Error(Value::Text(
                            "gtk4.appSetCss no default display".to_string(),
                        )));
                    }
                    let provider = unsafe { gtk_css_provider_new() };
                    unsafe { gtk_css_provider_load_from_string(provider, css_c.as_ptr()) };
                    // GTK_STYLE_PROVIDER_PRIORITY_APPLICATION = 600
                    unsafe {
                        gtk_style_context_add_provider_for_display(display, provider, 600)
                    };
                    Ok(Value::Unit)
                }))
            }),
        );

        fields
    }
}

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
