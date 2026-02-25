use crate::runtime::Value;

#[cfg(all(feature = "gtk4-libadwaita", target_os = "linux"))]
mod linux {
    use std::cell::RefCell;
    use std::collections::{HashMap, VecDeque};
    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_int, c_ulong, c_void};
    use std::ptr::null_mut;
    use std::sync::{Arc, Mutex};

    use super::super::util::builtin;
    use crate::runtime::{EffectValue, RuntimeError, Value};

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
        fn gtk_scrolled_window_set_propagate_natural_width(scrolled: *mut c_void, propagate: c_int);

        fn gtk_separator_new(orientation: c_int) -> *mut c_void;

        fn gtk_overlay_new() -> *mut c_void;
        fn gtk_overlay_set_child(overlay: *mut c_void, child: *mut c_void);
        fn gtk_overlay_add_overlay(overlay: *mut c_void, widget: *mut c_void);
        fn gtk_buildable_add_child(
            buildable: *mut c_void,
            builder: *mut c_void,
            child: *mut c_void,
            type_: *const c_char,
        );

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
        fn g_type_from_name(name: *const c_char) -> usize;
        fn g_object_new(object_type: usize, first_property_name: *const c_char, ...) -> *mut c_void;
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
        signal_events: VecDeque<SignalEventState>,
        tray_handles: HashMap<i64, Arc<Mutex<SniTrayState>>>,
        resources_registered: bool,
    }

    struct SniTrayState {
        icon_name: String,
        tooltip: String,
        visible: bool,
    }

    impl Default for SniTrayState {
        fn default() -> Self {
            Self {
                icon_name: String::new(),
                tooltip: String::new(),
                visible: true,
            }
        }
    }

    fn spawn_sni_tray(state: Arc<Mutex<SniTrayState>>) -> Result<(), String> {
        let state_clone = state.clone();
        std::thread::Builder::new()
            .name("sni-tray".to_string())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("tokio rt");
                rt.block_on(async {
                    let conn = match zbus::Connection::session().await {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("sni-tray: D-Bus session error: {e}");
                            return;
                        }
                    };
                    let pid = std::process::id();
                    let bus_name = format!("org.kde.StatusNotifierItem-{pid}-1");
                    let obj_path = "/StatusNotifierItem";

                    // Create the interface at obj_path using raw method calls
                    // We register an interface implementor manually
                    let sni = SniObject { state: state_clone };
                    if let Err(e) = conn.object_server().at(obj_path, sni).await {
                        eprintln!("sni-tray: object_server error: {e}");
                        return;
                    }
                    if let Err(e) = conn.request_name(bus_name.as_str()).await {
                        eprintln!("sni-tray: request_name error: {e}");
                        return;
                    }
                    // Register with StatusNotifierWatcher
                    let _ = conn
                        .call_method(
                            Some("org.kde.StatusNotifierWatcher"),
                            "/StatusNotifierWatcher",
                            Some("org.kde.StatusNotifierWatcher"),
                            "RegisterStatusNotifierItem",
                            &(bus_name.as_str(),),
                        )
                        .await;
                    // Keep alive
                    std::future::pending::<()>().await;
                });
            })
            .map(|_| ())
            .map_err(|e| format!("spawn sni thread: {e}"))
    }

    struct SniObject {
        state: Arc<Mutex<SniTrayState>>,
    }

    #[zbus::interface(name = "org.kde.StatusNotifierItem")]
    impl SniObject {
        #[zbus(property)]
        fn category(&self) -> &str {
            "Communications"
        }

        #[zbus(property)]
        fn id(&self) -> &str {
            "com-mailfox-desktop"
        }

        #[zbus(property)]
        fn title(&self) -> &str {
            "Mailfox"
        }

        #[zbus(property)]
        fn status(&self) -> &str {
            if self.state.lock().map(|s| s.visible).unwrap_or(true) {
                "Active"
            } else {
                "Passive"
            }
        }

        #[zbus(property)]
        fn icon_name(&self) -> String {
            self.state
                .lock()
                .map(|s| s.icon_name.clone())
                .unwrap_or_default()
        }

        #[zbus(property)]
        fn icon_theme_path(&self) -> &str {
            ""
        }

        #[zbus(property)]
        fn tool_tip(&self) -> (String, Vec<(i32, i32, Vec<u8>)>, String, String) {
            let tip = self
                .state
                .lock()
                .map(|s| s.tooltip.clone())
                .unwrap_or_default();
            (String::new(), Vec::new(), tip, String::new())
        }

        #[zbus(property)]
        fn item_is_menu(&self) -> bool {
            false
        }

        #[zbus(property)]
        fn menu(&self) -> zbus::zvariant::OwnedObjectPath {
            zbus::zvariant::OwnedObjectPath::try_from("/MenuBar")
                .unwrap_or_else(|_| zbus::zvariant::OwnedObjectPath::try_from("/").unwrap())
        }

        fn activate(&self, _x: i32, _y: i32) {}
        fn secondary_activate(&self, _x: i32, _y: i32) {}
        fn scroll(&self, _delta: i32, _orientation: &str) {}
    }

    struct GestureClickState {
        widget_id: i64,
        raw: *mut c_void,
        last_button: i64,
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

    #[derive(Clone, Copy)]
    enum SignalPayloadKind {
        None,
        EditableText,
    }

    struct SignalCallbackData {
        widget_id: i64,
        signal_name: String,
        handler: String,
        payload_kind: SignalPayloadKind,
    }

    impl RealGtkState {
        fn alloc_id(&mut self) -> i64 {
            self.next_id += 1;
            self.next_id
        }
    }

    fn effect<F>(f: F) -> Value
    where
        F: Fn(&mut crate::runtime::Runtime) -> Result<Value, RuntimeError> + Send + Sync + 'static,
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

    fn create_adw_widget_type(type_name: &str) -> Result<*mut c_void, RuntimeError> {
        try_adw_init();
        let class_c = c_text(type_name, "gtk4.buildFromNode invalid Adw class name")?;
        let g_type = unsafe { g_type_from_name(class_c.as_ptr()) };
        if g_type == 0 {
            return Err(RuntimeError::Error(Value::Text(format!(
                "gtk4.buildFromNode unknown Adw class {type_name}"
            ))));
        }
        let raw = unsafe { g_object_new(g_type, std::ptr::null::<c_char>()) };
        if raw.is_null() {
            return Err(RuntimeError::Error(Value::Text(format!(
                "gtk4.buildFromNode failed to create {type_name}"
            ))));
        }
        Ok(raw)
    }

    fn create_adw_widget(class_name: &str) -> Result<*mut c_void, RuntimeError> {
        match class_name {
            "AdwAboutDialog" => create_adw_widget_type("AdwAboutDialog"),
            "AdwAboutWindow" => create_adw_widget_type("AdwAboutWindow"),
            "AdwActionRow" => create_adw_widget_type("AdwActionRow"),
            "AdwAlertDialog" => create_adw_widget_type("AdwAlertDialog"),
            "AdwApplication" => create_adw_widget_type("AdwApplication"),
            "AdwApplicationWindow" => create_adw_widget_type("AdwApplicationWindow"),
            "AdwAvatar" => create_adw_widget_type("AdwAvatar"),
            "AdwBanner" => create_adw_widget_type("AdwBanner"),
            "AdwBin" => create_adw_widget_type("AdwBin"),
            "AdwBottomSheet" => create_adw_widget_type("AdwBottomSheet"),
            "AdwBreakpoint" => create_adw_widget_type("AdwBreakpoint"),
            "AdwBreakpointBin" => create_adw_widget_type("AdwBreakpointBin"),
            "AdwButtonContent" => create_adw_widget_type("AdwButtonContent"),
            "AdwButtonRow" => create_adw_widget_type("AdwButtonRow"),
            "AdwCallbackAnimationTarget" => create_adw_widget_type("AdwCallbackAnimationTarget"),
            "AdwCarousel" => create_adw_widget_type("AdwCarousel"),
            "AdwCarouselIndicatorDots" => create_adw_widget_type("AdwCarouselIndicatorDots"),
            "AdwCarouselIndicatorLines" => create_adw_widget_type("AdwCarouselIndicatorLines"),
            "AdwClamp" => create_adw_widget_type("AdwClamp"),
            "AdwClampLayout" => create_adw_widget_type("AdwClampLayout"),
            "AdwClampScrollable" => create_adw_widget_type("AdwClampScrollable"),
            "AdwComboRow" => create_adw_widget_type("AdwComboRow"),
            "AdwDialog" => create_adw_widget_type("AdwDialog"),
            "AdwEntryRow" => create_adw_widget_type("AdwEntryRow"),
            "AdwEnumListModel" => create_adw_widget_type("AdwEnumListModel"),
            "AdwExpanderRow" => create_adw_widget_type("AdwExpanderRow"),
            "AdwFlap" => create_adw_widget_type("AdwFlap"),
            "AdwHeaderBar" => create_adw_widget_type("AdwHeaderBar"),
            "AdwInlineViewSwitcher" => create_adw_widget_type("AdwInlineViewSwitcher"),
            "AdwLayout" => create_adw_widget_type("AdwLayout"),
            "AdwLayoutSlot" => create_adw_widget_type("AdwLayoutSlot"),
            "AdwLeaflet" => create_adw_widget_type("AdwLeaflet"),
            "AdwMessageDialog" => create_adw_widget_type("AdwMessageDialog"),
            "AdwMultiLayoutView" => create_adw_widget_type("AdwMultiLayoutView"),
            "AdwNavigationPage" => create_adw_widget_type("AdwNavigationPage"),
            "AdwNavigationSplitView" => create_adw_widget_type("AdwNavigationSplitView"),
            "AdwNavigationView" => create_adw_widget_type("AdwNavigationView"),
            "AdwOverlaySplitView" => create_adw_widget_type("AdwOverlaySplitView"),
            "AdwPasswordEntryRow" => create_adw_widget_type("AdwPasswordEntryRow"),
            "AdwPreferencesDialog" => create_adw_widget_type("AdwPreferencesDialog"),
            "AdwPreferencesGroup" => create_adw_widget_type("AdwPreferencesGroup"),
            "AdwPreferencesPage" => create_adw_widget_type("AdwPreferencesPage"),
            "AdwPreferencesRow" => create_adw_widget_type("AdwPreferencesRow"),
            "AdwPreferencesWindow" => create_adw_widget_type("AdwPreferencesWindow"),
            "AdwPropertyAnimationTarget" => create_adw_widget_type("AdwPropertyAnimationTarget"),
            "AdwShortcutLabel" => create_adw_widget_type("AdwShortcutLabel"),
            "AdwShortcutsDialog" => create_adw_widget_type("AdwShortcutsDialog"),
            "AdwShortcutsItem" => create_adw_widget_type("AdwShortcutsItem"),
            "AdwShortcutsSection" => create_adw_widget_type("AdwShortcutsSection"),
            "AdwSpinRow" => create_adw_widget_type("AdwSpinRow"),
            "AdwSpinner" => create_adw_widget_type("AdwSpinner"),
            "AdwSpinnerPaintable" => create_adw_widget_type("AdwSpinnerPaintable"),
            "AdwSplitButton" => create_adw_widget_type("AdwSplitButton"),
            "AdwSpringAnimation" => create_adw_widget_type("AdwSpringAnimation"),
            "AdwSpringParams" => create_adw_widget_type("AdwSpringParams"),
            "AdwSqueezer" => create_adw_widget_type("AdwSqueezer"),
            "AdwStatusPage" => create_adw_widget_type("AdwStatusPage"),
            "AdwSwipeTracker" => create_adw_widget_type("AdwSwipeTracker"),
            "AdwSwitchRow" => create_adw_widget_type("AdwSwitchRow"),
            "AdwTabBar" => create_adw_widget_type("AdwTabBar"),
            "AdwTabButton" => create_adw_widget_type("AdwTabButton"),
            "AdwTabOverview" => create_adw_widget_type("AdwTabOverview"),
            "AdwTabView" => create_adw_widget_type("AdwTabView"),
            "AdwTimedAnimation" => create_adw_widget_type("AdwTimedAnimation"),
            "AdwToast" => create_adw_widget_type("AdwToast"),
            "AdwToastOverlay" => create_adw_widget_type("AdwToastOverlay"),
            "AdwToggle" => create_adw_widget_type("AdwToggle"),
            "AdwToggleGroup" => create_adw_widget_type("AdwToggleGroup"),
            "AdwToolbarView" => create_adw_widget_type("AdwToolbarView"),
            "AdwViewStack" => create_adw_widget_type("AdwViewStack"),
            "AdwViewSwitcher" => create_adw_widget_type("AdwViewSwitcher"),
            "AdwViewSwitcherBar" => create_adw_widget_type("AdwViewSwitcherBar"),
            "AdwViewSwitcherTitle" => create_adw_widget_type("AdwViewSwitcherTitle"),
            "AdwWindow" => create_adw_widget_type("AdwWindow"),
            "AdwWindowTitle" => create_adw_widget_type("AdwWindowTitle"),
            "AdwWrapBox" => create_adw_widget_type("AdwWrapBox"),
            "AdwWrapLayout" => create_adw_widget_type("AdwWrapLayout"),
            _ => Err(RuntimeError::Error(Value::Text(format!(
                "gtk4.buildFromNode unsupported class {class_name}"
            )))),
        }
    }

    fn buildable_add_child(
        parent: *mut c_void,
        child: *mut c_void,
        child_type: Option<&str>,
    ) -> Result<(), RuntimeError> {
        let child_type_c = if let Some(kind) = child_type {
            Some(c_text(kind, "gtk4.buildFromNode invalid child type")?)
        } else {
            None
        };
        unsafe {
            gtk_buildable_add_child(
                parent,
                null_mut(),
                child,
                child_type_c
                    .as_ref()
                    .map(|v| v.as_ptr())
                    .unwrap_or(std::ptr::null()),
            )
        };
        Ok(())
    }

    fn widget_ptr(state: &RealGtkState, id: i64, ctx: &str) -> Result<*mut c_void, RuntimeError> {
        state.widgets.get(&id).copied().ok_or_else(|| {
            RuntimeError::Error(Value::Text(format!("gtk4.{ctx} unknown widget id {id}")))
        })
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

    #[derive(Debug, Clone, Copy)]
    enum CreatedWidgetKind {
        Box,
        HeaderBar,
        ScrolledWindow,
        Overlay,
        ListBox,
        Other,
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
        let val = decode_text(&args[1])
            .ok_or_else(|| invalid("gtk4.buildFromNode invalid attr value"))?;
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

    fn parse_i32_text(text: &str) -> Option<i32> {
        text.trim().parse::<i32>().ok()
    }

    fn parse_usize_text(text: &str) -> Option<usize> {
        text.trim().parse::<usize>().ok()
    }

    fn parse_f64_text(text: &str) -> Option<f64> {
        text.trim().parse::<f64>().ok()
    }

    fn parse_bool_text(text: &str) -> Option<bool> {
        match text.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        }
    }

    fn parse_orientation_text(text: &str) -> c_int {
        match text.trim().to_ascii_lowercase().as_str() {
            "vertical" | "1" => 1,
            _ => 0,
        }
    }

    fn parse_align_text(text: &str) -> Option<c_int> {
        match text.trim().to_ascii_lowercase().as_str() {
            "fill" => Some(0),
            "start" => Some(1),
            "end" => Some(2),
            "center" => Some(3),
            other => other.parse::<c_int>().ok(),
        }
    }

    fn parse_policy_text(text: &str) -> Option<c_int> {
        match text.trim().to_ascii_lowercase().as_str() {
            "always" => Some(0),
            "automatic" => Some(1),
            "never" => Some(2),
            "external" => Some(3),
            other => other.parse::<c_int>().ok(),
        }
    }

    fn parse_ellipsize_text(text: &str) -> Option<c_int> {
        match text.trim().to_ascii_lowercase().as_str() {
            "none" => Some(0),
            "start" => Some(1),
            "middle" => Some(2),
            "end" => Some(3),
            other => other.parse::<c_int>().ok(),
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
            let Some(handler) = node_attr(attrs, "handler").or_else(|| node_attr(attrs, "on"))
            else {
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
                continue;
            }
            if tag == "property" && node_attr(attrs, "name") == Some("child") {
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
                continue;
            }
            if tag == "object" {
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

    fn apply_widget_properties(
        widget: *mut c_void,
        class_name: &str,
        props: &HashMap<String, String>,
    ) -> Result<(), RuntimeError> {
        if let Some(value) = props.get("margin-start").and_then(|v| parse_i32_text(v)) {
            unsafe { gtk_widget_set_margin_start(widget, value) };
        }
        if let Some(value) = props.get("margin-end").and_then(|v| parse_i32_text(v)) {
            unsafe { gtk_widget_set_margin_end(widget, value) };
        }
        if let Some(value) = props.get("margin-top").and_then(|v| parse_i32_text(v)) {
            unsafe { gtk_widget_set_margin_top(widget, value) };
        }
        if let Some(value) = props.get("margin-bottom").and_then(|v| parse_i32_text(v)) {
            unsafe { gtk_widget_set_margin_bottom(widget, value) };
        }
        if let Some(value) = props.get("hexpand").and_then(|v| parse_bool_text(v)) {
            unsafe { gtk_widget_set_hexpand(widget, if value { 1 } else { 0 }) };
        }
        if let Some(value) = props.get("vexpand").and_then(|v| parse_bool_text(v)) {
            unsafe { gtk_widget_set_vexpand(widget, if value { 1 } else { 0 }) };
        }
        if let Some(value) = props.get("halign").and_then(|v| parse_align_text(v)) {
            unsafe { gtk_widget_set_halign(widget, value) };
        }
        if let Some(value) = props.get("valign").and_then(|v| parse_align_text(v)) {
            unsafe { gtk_widget_set_valign(widget, value) };
        }
        if let Some(value) = props.get("width-request").and_then(|v| parse_i32_text(v)) {
            unsafe { gtk_widget_set_size_request(widget, value, -1) };
        }
        if let Some(value) = props.get("height-request").and_then(|v| parse_i32_text(v)) {
            unsafe { gtk_widget_set_size_request(widget, -1, value) };
        }
        if let Some(value) = props.get("tooltip-text") {
            let text_c = c_text(value, "gtk4.buildFromNode invalid tooltip-text")?;
            unsafe { gtk_widget_set_tooltip_text(widget, text_c.as_ptr()) };
        }
        if let Some(value) = props.get("opacity").and_then(|v| parse_f64_text(v)) {
            unsafe { gtk_widget_set_opacity(widget, value) };
        }
        if let Some(value) = props.get("visible").and_then(|v| parse_bool_text(v)) {
            unsafe { gtk_widget_set_visible(widget, if value { 1 } else { 0 }) };
        }
        if let Some(value) = props.get("css-class") {
            for class_name in value.split_whitespace() {
                let class_c = c_text(class_name, "gtk4.buildFromNode invalid css class")?;
                unsafe { gtk_widget_add_css_class(widget, class_c.as_ptr()) };
            }
        }

        match class_name {
            "GtkLabel" => {
                if let Some(value) = props.get("label").or_else(|| props.get("text")) {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid GtkLabel text")?;
                    unsafe { gtk_label_set_text(widget, text_c.as_ptr()) };
                }
                if let Some(value) = props.get("wrap").and_then(|v| parse_bool_text(v)) {
                    unsafe { gtk_label_set_wrap(widget, if value { 1 } else { 0 }) };
                }
                if let Some(value) = props.get("ellipsize").and_then(|v| parse_ellipsize_text(v)) {
                    unsafe { gtk_label_set_ellipsize(widget, value) };
                }
                if let Some(value) = props.get("xalign").and_then(|v| parse_f64_text(v)) {
                    unsafe { gtk_label_set_xalign(widget, value as f32) };
                }
                if let Some(value) = props.get("max-width-chars").and_then(|v| parse_i32_text(v)) {
                    unsafe { gtk_label_set_max_width_chars(widget, value) };
                }
            }
            "GtkButton" => {
                if let Some(value) = props.get("label") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid GtkButton label")?;
                    unsafe { gtk_button_set_label(widget, text_c.as_ptr()) };
                }
            }
            "GtkEntry" => {
                if let Some(value) = props.get("text") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid GtkEntry text")?;
                    unsafe { gtk_editable_set_text(widget, text_c.as_ptr()) };
                }
            }
            "GtkImage" => {
                if let Some(value) = props.get("resource") {
                    let resource_c = c_text(value, "gtk4.buildFromNode invalid GtkImage resource")?;
                    unsafe { gtk_image_set_from_resource(widget, resource_c.as_ptr()) };
                } else if let Some(value) = props.get("file") {
                    let file_c = c_text(value, "gtk4.buildFromNode invalid GtkImage file")?;
                    unsafe { gtk_image_set_from_file(widget, file_c.as_ptr()) };
                }
                if let Some(value) = props.get("pixel-size").and_then(|v| parse_i32_text(v)) {
                    unsafe { gtk_image_set_pixel_size(widget, value) };
                }
            }
            "GtkBox" | "AdwClamp" => {
                if let Some(value) = props.get("homogeneous").and_then(|v| parse_bool_text(v)) {
                    unsafe { gtk_box_set_homogeneous(widget, if value { 1 } else { 0 }) };
                }
            }
            "GtkHeaderBar" | "AdwHeaderBar" => {
                if let Some(value) = props.get("decoration-layout") {
                    let layout_c =
                        c_text(value, "gtk4.buildFromNode invalid headerbar decoration-layout")?;
                    unsafe { gtk_header_bar_set_decoration_layout(widget, layout_c.as_ptr()) };
                }
                if let Some(value) = props
                    .get("show-title-buttons")
                    .or_else(|| props.get("show-end-title-buttons"))
                    .and_then(|v| parse_bool_text(v))
                {
                    unsafe {
                        gtk_header_bar_set_show_title_buttons(widget, if value { 1 } else { 0 })
                    };
                }
            }
            "GtkScrolledWindow" => {
                let h_policy = props
                    .get("hscrollbar-policy")
                    .and_then(|v| parse_policy_text(v))
                    .unwrap_or(1);
                let v_policy = props
                    .get("vscrollbar-policy")
                    .and_then(|v| parse_policy_text(v))
                    .unwrap_or(1);
                unsafe { gtk_scrolled_window_set_policy(widget, h_policy, v_policy) };
                if let Some(value) = props
                    .get("propagate-natural-height")
                    .and_then(|v| parse_bool_text(v))
                {
                    unsafe {
                        gtk_scrolled_window_set_propagate_natural_height(
                            widget,
                            if value { 1 } else { 0 },
                        )
                    };
                }
                if let Some(value) = props
                    .get("propagate-natural-width")
                    .and_then(|v| parse_bool_text(v))
                {
                    unsafe {
                        gtk_scrolled_window_set_propagate_natural_width(
                            widget,
                            if value { 1 } else { 0 },
                        )
                    };
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn build_widget_from_node_real(
        state: &mut RealGtkState,
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
        let class_name = node_attr(attrs, "class")
            .ok_or_else(|| invalid("gtk4.buildFromNode object requires class attribute"))?;
        let props = collect_object_properties(attrs, children);
        let signal_bindings = collect_object_signals(attrs, children);

        let (raw, kind) = match class_name {
            "GtkBox" | "AdwClamp" => {
                let orientation = props
                    .get("orientation")
                    .map(|v| parse_orientation_text(v))
                    .unwrap_or(0);
                let spacing = props
                    .get("spacing")
                    .and_then(|v| parse_i32_text(v))
                    .unwrap_or(0);
                (
                    unsafe { gtk_box_new(orientation, spacing) },
                    CreatedWidgetKind::Box,
                )
            }
            "GtkHeaderBar" | "AdwHeaderBar" => {
                (unsafe { gtk_header_bar_new() }, CreatedWidgetKind::HeaderBar)
            }
            "GtkLabel" => {
                let label = props
                    .get("label")
                    .or_else(|| props.get("text"))
                    .cloned()
                    .unwrap_or_default();
                let label_c = c_text(&label, "gtk4.buildFromNode invalid GtkLabel text")?;
                (
                    unsafe { gtk_label_new(label_c.as_ptr()) },
                    CreatedWidgetKind::Other,
                )
            }
            "GtkButton" => {
                if let Some(icon_name) = props.get("icon-name").cloned() {
                    let icon_c = c_text(&icon_name, "gtk4.buildFromNode invalid GtkButton icon")?;
                    (
                        unsafe { gtk_button_new_from_icon_name(icon_c.as_ptr()) },
                        CreatedWidgetKind::Other,
                    )
                } else {
                    let label = props.get("label").cloned().unwrap_or_default();
                    let label_c = c_text(&label, "gtk4.buildFromNode invalid GtkButton label")?;
                    (
                        unsafe { gtk_button_new_with_label(label_c.as_ptr()) },
                        CreatedWidgetKind::Other,
                    )
                }
            }
            "GtkEntry" => (unsafe { gtk_entry_new() }, CreatedWidgetKind::Other),
            "GtkDrawingArea" => (unsafe { gtk_drawing_area_new() }, CreatedWidgetKind::Other),
            "GtkGestureClick" => (unsafe { gtk_gesture_click_new() }, CreatedWidgetKind::Other),
            "GtkScrolledWindow" => (
                unsafe { gtk_scrolled_window_new() },
                CreatedWidgetKind::ScrolledWindow,
            ),
            "GtkOverlay" => (unsafe { gtk_overlay_new() }, CreatedWidgetKind::Overlay),
            "GtkSeparator" => {
                let orientation = props
                    .get("orientation")
                    .map(|v| parse_orientation_text(v))
                    .unwrap_or(0);
                (
                    unsafe { gtk_separator_new(orientation) },
                    CreatedWidgetKind::Other,
                )
            }
            "GtkImage" => {
                if let Some(resource) = props.get("resource") {
                    let resource_c = c_text(resource, "gtk4.buildFromNode invalid resource")?;
                    (
                        unsafe { gtk_image_new_from_resource(resource_c.as_ptr()) },
                        CreatedWidgetKind::Other,
                    )
                } else if let Some(file) = props.get("file") {
                    let file_c = c_text(file, "gtk4.buildFromNode invalid file")?;
                    (
                        unsafe { gtk_image_new_from_file(file_c.as_ptr()) },
                        CreatedWidgetKind::Other,
                    )
                } else {
                    let icon = props.get("icon-name").cloned().unwrap_or_default();
                    let icon_c = c_text(&icon, "gtk4.buildFromNode invalid icon-name")?;
                    (
                        unsafe { gtk_image_new_from_icon_name(icon_c.as_ptr()) },
                        CreatedWidgetKind::Other,
                    )
                }
            }
            "GtkListBox" => (unsafe { gtk_list_box_new() }, CreatedWidgetKind::ListBox),
            "AdwAboutDialog"
            | "AdwAboutWindow"
            | "AdwActionRow"
            | "AdwAlertDialog"
            | "AdwApplication"
            | "AdwApplicationWindow"
            | "AdwAvatar"
            | "AdwBanner"
            | "AdwBin"
            | "AdwBottomSheet"
            | "AdwBreakpoint"
            | "AdwBreakpointBin"
            | "AdwButtonContent"
            | "AdwButtonRow"
            | "AdwCallbackAnimationTarget"
            | "AdwCarousel"
            | "AdwCarouselIndicatorDots"
            | "AdwCarouselIndicatorLines"
            | "AdwClamp"
            | "AdwClampLayout"
            | "AdwClampScrollable"
            | "AdwComboRow"
            | "AdwDialog"
            | "AdwEntryRow"
            | "AdwEnumListModel"
            | "AdwExpanderRow"
            | "AdwFlap"
            | "AdwHeaderBar"
            | "AdwInlineViewSwitcher"
            | "AdwLayout"
            | "AdwLayoutSlot"
            | "AdwLeaflet"
            | "AdwMessageDialog"
            | "AdwMultiLayoutView"
            | "AdwNavigationPage"
            | "AdwNavigationSplitView"
            | "AdwNavigationView"
            | "AdwOverlaySplitView"
            | "AdwPasswordEntryRow"
            | "AdwPreferencesDialog"
            | "AdwPreferencesGroup"
            | "AdwPreferencesPage"
            | "AdwPreferencesRow"
            | "AdwPreferencesWindow"
            | "AdwPropertyAnimationTarget"
            | "AdwShortcutLabel"
            | "AdwShortcutsDialog"
            | "AdwShortcutsItem"
            | "AdwShortcutsSection"
            | "AdwSpinRow"
            | "AdwSpinner"
            | "AdwSpinnerPaintable"
            | "AdwSplitButton"
            | "AdwSpringAnimation"
            | "AdwSpringParams"
            | "AdwSqueezer"
            | "AdwStatusPage"
            | "AdwSwipeTracker"
            | "AdwSwitchRow"
            | "AdwTabBar"
            | "AdwTabButton"
            | "AdwTabOverview"
            | "AdwTabView"
            | "AdwTimedAnimation"
            | "AdwToast"
            | "AdwToastOverlay"
            | "AdwToggle"
            | "AdwToggleGroup"
            | "AdwToolbarView"
            | "AdwViewStack"
            | "AdwViewSwitcher"
            | "AdwViewSwitcherBar"
            | "AdwViewSwitcherTitle"
            | "AdwWindow"
            | "AdwWindowTitle"
            | "AdwWrapBox"
            | "AdwWrapLayout" => (create_adw_widget(class_name)?, CreatedWidgetKind::Other),
            _ => {
                return Err(RuntimeError::Error(Value::Text(format!(
                    "gtk4.buildFromNode unsupported class {class_name}"
                ))));
            }
        };
        if raw.is_null() {
            return Err(RuntimeError::Error(Value::Text(format!(
                "gtk4.buildFromNode failed to create {class_name}"
            ))));
        }

        let id = state.alloc_id();
        if let Some(object_id) = node_attr(attrs, "id") {
            id_map.insert(object_id.to_string(), id);
        }
        state.widgets.insert(id, raw);
        match class_name {
            "GtkBox" | "AdwClamp" => {
                state.boxes.insert(id, raw);
            }
            "GtkButton" => {
                state.buttons.insert(id, raw);
            }
            "GtkLabel" => {
                state.labels.insert(id, raw);
            }
            "GtkEntry" => {
                state.entries.insert(id, raw);
            }
            "GtkImage" => {
                state.images.insert(id, raw);
            }
            "GtkDrawingArea" => {
                state.draw_areas.insert(id, raw);
            }
            "GtkGestureClick" => {
                state.gesture_clicks.insert(
                    id,
                    GestureClickState {
                        widget_id: 0,
                        raw,
                        last_button: 0,
                    },
                );
            }
            "GtkScrolledWindow" => {
                state.scrolled_windows.insert(id, raw);
            }
            "GtkOverlay" => {
                state.overlays.insert(id, raw);
            }
            "GtkSeparator" => {
                state.separators.insert(id, raw);
            }
            _ => {}
        }

        apply_widget_properties(raw, class_name, &props)?;
        for binding in signal_bindings {
            connect_widget_signal(raw, id, class_name, &binding)?;
        }

        let mut child_objects = collect_child_objects(children);
        child_objects.sort_by_key(|child| child.position.unwrap_or(usize::MAX));
        let mut overlay_root_set = false;
        for child in child_objects {
            let child_id = build_widget_from_node_real(state, child.node, id_map)?;
            let child_raw = widget_ptr(state, child_id, "buildFromNode")?;
            if child.child_type.as_deref() == Some("controller") {
                unsafe { gtk_widget_add_controller(raw, child_raw) };
                if let Some(gesture) = state.gesture_clicks.get_mut(&child_id) {
                    gesture.widget_id = id;
                }
                continue;
            }
            match kind {
                CreatedWidgetKind::Box => unsafe { gtk_box_append(raw, child_raw) },
                CreatedWidgetKind::HeaderBar => match child.child_type.as_deref() {
                    Some("end") => unsafe { gtk_header_bar_pack_end(raw, child_raw) },
                    Some("title") => unsafe { gtk_header_bar_set_title_widget(raw, child_raw) },
                    _ => unsafe { gtk_header_bar_pack_start(raw, child_raw) },
                },
                CreatedWidgetKind::ScrolledWindow => {
                    if child.child_type.as_deref() != Some("overlay") {
                        unsafe { gtk_scrolled_window_set_child(raw, child_raw) };
                    }
                }
                CreatedWidgetKind::Overlay => {
                    if child.child_type.as_deref() == Some("overlay") {
                        unsafe { gtk_overlay_add_overlay(raw, child_raw) };
                    } else if !overlay_root_set {
                        unsafe { gtk_overlay_set_child(raw, child_raw) };
                        overlay_root_set = true;
                    } else {
                        unsafe { gtk_overlay_add_overlay(raw, child_raw) };
                    }
                }
                CreatedWidgetKind::ListBox => unsafe { gtk_list_box_append(raw, child_raw) },
                CreatedWidgetKind::Other => {
                    buildable_add_child(raw, child_raw, child.child_type.as_deref())?;
                }
            }
        }

        Ok(id)
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
        };
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.signal_events.push_back(SignalEventState {
                widget_id: binding.widget_id,
                signal: binding.signal_name.clone(),
                handler: binding.handler.clone(),
                payload,
            });
        });
    }

    fn signal_payload_kind_for(class_name: &str, signal_name: &str) -> Option<SignalPayloadKind> {
        match (class_name, signal_name) {
            ("GtkButton", "clicked") => Some(SignalPayloadKind::None),
            ("GtkEntry", "changed") | ("GtkEntry", "activate") => {
                Some(SignalPayloadKind::EditableText)
            }
            _ => None,
        }
    }

    fn connect_widget_signal(
        widget: *mut c_void,
        widget_id: i64,
        class_name: &str,
        binding: &SignalBindingState,
    ) -> Result<(), RuntimeError> {
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
        unsafe {
            g_signal_connect_data(
                widget,
                signal_c.as_ptr(),
                gtk_signal_callback as *const c_void,
                callback_ptr,
                null_mut(),
                0,
            );
        }
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
                    let path_c = c_text(&path, "gtk4.iconThemeAddSearchPath invalid path")?;
                    unsafe {
                        let display = gdk_display_get_default();
                        if display.is_null() {
                            return Err(RuntimeError::Error(Value::Text(
                                "gtk4.iconThemeAddSearchPath no default display".to_string(),
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
                    unsafe { gtk_style_context_add_provider_for_display(display, provider, 600) };
                    Ok(Value::Unit)
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
                        build_widget_from_node_real(&mut state, root, &mut id_map)
                    })?;
                    Ok(Value::Int(id))
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
