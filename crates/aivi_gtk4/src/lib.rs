#![allow(clippy::type_complexity)]
#![allow(unused_unsafe)]

use std::collections::HashMap;
use std::fmt;

// ── Public Types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Gtk4Error {
    pub message: String,
}

impl Gtk4Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for Gtk4Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Gtk4Error {}

#[derive(Debug, Clone)]
pub enum GtkNode {
    Element {
        tag: String,
        attrs: Vec<(String, String)>,
        children: Vec<GtkNode>,
    },
    Text(String),
}

#[derive(Debug, Clone)]
pub struct SignalEvent {
    pub widget_id: i64,
    pub widget_name: String,
    pub signal: String,
    pub handler: String,
    pub payload: String,
}

pub struct BuildResult {
    pub root_id: i64,
    pub named_widgets: HashMap<String, i64>,
}

#[cfg(target_os = "linux")]
#[allow(dead_code)]
mod linux_impl {
    use std::cell::RefCell;
    use std::collections::{HashMap, VecDeque};
    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_int, c_uint, c_ulong, c_void};
    use std::ptr::null_mut;
    use std::sync::{mpsc, Mutex, OnceLock};

    use super::{BuildResult, Gtk4Error, GtkNode, SignalEvent};

    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default)]
    pub struct GdkRectangle {
        pub x: c_int,
        pub y: c_int,
        pub width: c_int,
        pub height: c_int,
    }

    #[link(name = "gtk-4")]
    unsafe extern "C" {
        fn gtk_init();
        fn gtk_application_new(application_id: *const c_char, flags: c_int) -> *mut c_void;
        fn gtk_window_set_title(window: *mut c_void, title: *const c_char);
        fn gtk_window_set_default_size(window: *mut c_void, width: c_int, height: c_int);
        fn gdk_display_get_default() -> *mut c_void;
        fn gdk_display_get_monitors(display: *mut c_void) -> *mut c_void;
        fn g_list_model_get_n_items(list: *mut c_void) -> c_uint;
        fn g_list_model_get_item(list: *mut c_void, position: c_uint) -> *mut c_void;
        fn gdk_monitor_get_geometry(monitor: *mut c_void, geometry: *mut GdkRectangle);
        fn gtk_native_get_surface(native: *mut c_void) -> *mut c_void;
        fn gtk_widget_get_width(widget: *mut c_void) -> c_int;
        fn gtk_widget_get_height(widget: *mut c_void) -> c_int;
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
        fn gtk_header_bar_set_decoration_layout(header_bar: *mut c_void, layout: *const c_char);
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
        fn gtk_password_entry_set_show_peek_icon(entry: *mut c_void, show_peek_icon: c_int);
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
        fn g_object_new(object_type: usize, first_property_name: *const c_char, ...)
            -> *mut c_void;
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
        static GTK_PUMP_ACTIVE: RefCell<bool> = const { RefCell::new(false) };
    }

    static PENDING_TRAY_ACTIONS: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
    fn pending_tray_actions() -> &'static Mutex<VecDeque<String>> {
        PENDING_TRAY_ACTIONS.get_or_init(|| Mutex::new(VecDeque::new()))
    }

    static PENDING_BADGE_UPDATES: OnceLock<Mutex<VecDeque<i64>>> = OnceLock::new();
    fn pending_badge_updates() -> &'static Mutex<VecDeque<i64>> {
        PENDING_BADGE_UPDATES.get_or_init(|| Mutex::new(VecDeque::new()))
    }

    struct PersonalEmailNotif {
        id: String,
        from: String,
        subject: String,
        markdown_body: String,
    }

    static PENDING_PERSONAL_EMAILS: OnceLock<Mutex<VecDeque<PersonalEmailNotif>>> = OnceLock::new();
    fn pending_personal_emails() -> &'static Mutex<VecDeque<PersonalEmailNotif>> {
        PENDING_PERSONAL_EMAILS.get_or_init(|| Mutex::new(VecDeque::new()))
    }

    static EMAIL_SUGGESTIONS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    fn email_suggestions() -> &'static Mutex<Vec<String>> {
        EMAIL_SUGGESTIONS.get_or_init(|| Mutex::new(Vec::new()))
    }

    // ── Types ──
    struct ScrollFadeData {
        scrolled: *mut c_void,
        top_fade: *mut c_void,
        bottom_fade: *mut c_void,
    }
    unsafe impl Send for ScrollFadeData {}
    unsafe impl Sync for ScrollFadeData {}

    /// Mirrors the live GTK widget tree for reconciliation.
    #[derive(Clone, Debug)]
    struct LiveNode {
        widget_id: i64,
        class_name: String,
        kind: CreatedWidgetKind,
        node_id: Option<String>,
        props: HashMap<String, String>,
        signals: Vec<SignalBindingState>,
        signal_handler_ids: Vec<c_ulong>,
        children: Vec<LiveChild>,
    }

    #[derive(Clone, Debug)]
    struct LiveChild {
        child_type: Option<String>,
        node: LiveNode,
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
        signal_senders: Vec<mpsc::Sender<SignalEvent>>,
        signal_action_bindings: HashMap<String, Vec<SignalAction>>,
        named_widgets: HashMap<String, i64>,
        widget_id_to_name: HashMap<i64, String>,
        pending_icon_search_paths: Vec<String>,
        pending_css_texts: Vec<String>,
        resources_registered: bool,
        /// Root widget id → LiveNode tree for reconciliation.
        live_trees: HashMap<i64, LiveNode>,
    }

    enum SignalAction {
        SetBool {
            widget_id: i64,
            property: String,
            value: bool,
        },
        CssClass {
            widget_id: i64,
            class_name: String,
            add: bool,
        },
        ToggleBool {
            widget_id: i64,
            property: String,
        },
        ToggleCssClass {
            widget_id: i64,
            class_name: String,
        },
        PresentDialog {
            dialog_id: i64,
            parent_id: i64,
        },
        SetStackPage {
            stack_id: i64,
            page_name: String,
        },
    }

    struct GestureClickState {
        widget_id: i64,
        raw: *mut c_void,
        last_button: i64,
    }

    #[derive(Clone, Debug, PartialEq)]
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
    #[allow(dead_code)]
    enum SignalPayloadKind {
        None,
        EditableText,
        ToggleActive,
        FloatValue,
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

    #[derive(Debug, Clone, Copy)]
    enum CreatedWidgetKind {
        Box,
        Button,
        HeaderBar,
        ScrolledWindow,
        Overlay,
        ListBox,
        SplitView,
        Stack,
        Revealer,
        PreferencesDialog,
        PreferencesPage,
        PreferencesGroup,
        ActionRow,
        Other,
    }

    struct ChildSpec<'a> {
        node: &'a GtkNode,
        child_type: Option<String>,
        position: Option<usize>,
    }

    // ── Helpers ──

    fn as_i32(value: i64, what: &str) -> Result<i32, Gtk4Error> {
        i32::try_from(value).map_err(|_| Gtk4Error::new(what))
    }

    fn c_text(text: &str, what: &str) -> Result<CString, Gtk4Error> {
        CString::new(text.as_bytes()).map_err(|_| Gtk4Error::new(what))
    }

    fn bool_to_c(val: bool) -> c_int {
        if val {
            1
        } else {
            0
        }
    }

    /// Set a GObject boolean (c_int) property.
    unsafe fn gobject_set_bool(widget: *mut c_void, prop: &CStr, val: c_int) {
        g_object_set(widget, prop.as_ptr(), val, std::ptr::null::<c_char>());
    }

    /// Set a GObject string property.
    unsafe fn gobject_set_str(widget: *mut c_void, prop: &CStr, val: &CStr) {
        g_object_set(
            widget,
            prop.as_ptr(),
            val.as_ptr(),
            std::ptr::null::<c_char>(),
        );
    }

    /// Set a GObject f64 property.
    unsafe fn gobject_set_f64(widget: *mut c_void, prop: &CStr, val: f64) {
        g_object_set(widget, prop.as_ptr(), val, std::ptr::null::<c_char>());
    }

    /// Set a GObject pointer property.
    unsafe fn gobject_set_ptr(widget: *mut c_void, prop: &CStr, val: *mut c_void) {
        g_object_set(widget, prop.as_ptr(), val, std::ptr::null::<c_char>());
    }

    /// Get a GObject boolean (c_int) property.
    unsafe fn gobject_get_bool(widget: *mut c_void, prop: &CStr) -> c_int {
        let mut val: c_int = 0;
        g_object_get(
            widget,
            prop.as_ptr(),
            &mut val as *mut c_int,
            std::ptr::null::<c_char>(),
        );
        val
    }

    /// Set a GObject string property from a props map entry.
    fn set_obj_str(
        widget: *mut c_void,
        props: &HashMap<String, String>,
        key: &str,
        class_hint: &str,
    ) -> Result<(), Gtk4Error> {
        if let Some(value) = props.get(key) {
            let text_c = c_text(
                value,
                &format!("gtk4.buildFromNode invalid {class_hint} {key}"),
            )?;
            let prop_c = CString::new(key).unwrap();
            unsafe { gobject_set_str(widget, &prop_c, &text_c) };
        }
        Ok(())
    }

    /// Set a GObject boolean property from a props map entry.
    fn set_obj_bool(widget: *mut c_void, props: &HashMap<String, String>, key: &str) {
        if let Some(value) = props.get(key).and_then(|v| parse_bool_text(v)) {
            let prop_c = CString::new(key).unwrap();
            unsafe { gobject_set_bool(widget, &prop_c, bool_to_c(value)) };
        }
    }

    /// Set a GObject f64 property from a props map entry.
    fn set_obj_f64(widget: *mut c_void, props: &HashMap<String, String>, key: &str) {
        if let Some(value) = props.get(key).and_then(|v| parse_f64_text(v)) {
            let prop_c = CString::new(key).unwrap();
            unsafe { gobject_set_f64(widget, &prop_c, value) };
        }
    }

    fn apply_pending_display_customizations(state: &mut RealGtkState) -> Result<(), Gtk4Error> {
        let display = unsafe { gdk_display_get_default() };
        if display.is_null() {
            return Ok(());
        }

        if !state.pending_icon_search_paths.is_empty() {
            let theme = unsafe { gtk_icon_theme_get_for_display(display) };
            for path in std::mem::take(&mut state.pending_icon_search_paths) {
                let path_c = c_text(&path, "gtk4.iconThemeAddSearchPath invalid path")?;
                unsafe { gtk_icon_theme_add_search_path(theme, path_c.as_ptr()) };
            }
        }

        for css_text in std::mem::take(&mut state.pending_css_texts) {
            let css_c = c_text(&css_text, "gtk4.appSetCss invalid css")?;
            let provider = unsafe { gtk_css_provider_new() };
            unsafe {
                gtk_css_provider_load_from_string(provider, css_c.as_ptr());
                // GTK_STYLE_PROVIDER_PRIORITY_APPLICATION = 600
                gtk_style_context_add_provider_for_display(display, provider, 600);
            }
        }

        Ok(())
    }

    fn widget_ptr(state: &RealGtkState, id: i64, ctx: &str) -> Result<*mut c_void, Gtk4Error> {
        state
            .widgets
            .get(&id)
            .copied()
            .ok_or_else(|| Gtk4Error::new(format!("gtk4.{ctx} unknown widget id {id}")))
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

    fn parse_wrap_mode_text(text: &str) -> Option<c_int> {
        match text.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "none" => Some(0),
            "char" => Some(1),
            "word" => Some(2),
            "word_char" | "word-char" => Some(3),
            other => other.parse::<c_int>().ok(),
        }
    }

    fn node_attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
        attrs
            .iter()
            .find_map(|(key, value)| (key == name).then_some(value.as_str()))
    }

    fn collect_text(children: &[GtkNode]) -> String {
        let mut out = String::new();
        for child in children {
            if let GtkNode::Text(text) = child {
                out.push_str(text);
            }
        }
        out.trim().to_string()
    }

    fn invalid(name: &str) -> Gtk4Error {
        Gtk4Error::new(name.to_string())
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

    fn call_adw_fn_pp(fn_name: &str, arg0: *mut c_void, arg1: *mut c_void) {
        const RTLD_NOW: c_int = 2;
        const RTLD_NODELETE: c_int = 0x1000;
        for lib_name in ["libadwaita-1.so.0", "libadwaita-1.so"] {
            let Ok(name) = CString::new(lib_name) else {
                continue;
            };
            let handle = unsafe { dlopen(name.as_ptr(), RTLD_NOW | RTLD_NODELETE) };
            if handle.is_null() {
                continue;
            }
            let Ok(sym) = CString::new(fn_name) else {
                break;
            };
            let ptr = unsafe { dlsym(handle, sym.as_ptr()) };
            if !ptr.is_null() {
                let f: unsafe extern "C" fn(*mut c_void, *mut c_void) =
                    unsafe { std::mem::transmute(ptr) };
                unsafe { f(arg0, arg1) };
            }
            let _ = unsafe { dlclose(handle) };
            break;
        }
    }

    fn maybe_register_gresource_bundle() -> Result<(), Gtk4Error> {
        const GRESOURCE_ENV: &str = "AIVI_GTK4_GRESOURCE_PATH";
        let path = match std::env::var(GRESOURCE_ENV) {
            Ok(path) => path,
            Err(std::env::VarError::NotPresent) => return Ok(()),
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(Gtk4Error::new(format!(
                    "{GRESOURCE_ENV} must be valid UTF-8"
                )))
            }
        };
        if path.is_empty() {
            return Err(Gtk4Error::new(format!("{GRESOURCE_ENV} cannot be empty")));
        }
        let path_c = c_text(
            &path,
            "gtk4.init invalid gresource path from AIVI_GTK4_GRESOURCE_PATH",
        )?;
        let mut err = null_mut();
        let resource = unsafe { g_resource_load(path_c.as_ptr(), &mut err) };
        if resource.is_null() {
            return Err(Gtk4Error::new(format!(
                "gtk4.init failed to load gresource bundle from {path}"
            )));
        }
        unsafe { g_resources_register(resource) };
        Ok(())
    }

    // ── Signals ──
    unsafe extern "C" fn scroll_fade_cb(_adj: *mut c_void, data: *mut c_void) {
        let d = &*(data as *const ScrollFadeData);
        let adj = gtk_scrolled_window_get_vadjustment(d.scrolled);
        if adj.is_null() {
            return;
        }
        let value = gtk_adjustment_get_value(adj);
        let upper = gtk_adjustment_get_upper(adj);
        let page_size = gtk_adjustment_get_page_size(adj);
        let fade_px = 50.0_f64;
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

    fn wire_scroll_fades(scrolled: *mut c_void, top_fade: *mut c_void, bottom_fade: *mut c_void) {
        let data = Box::into_raw(Box::new(ScrollFadeData {
            scrolled,
            top_fade,
            bottom_fade,
        }));
        unsafe {
            let adj = gtk_scrolled_window_get_vadjustment(scrolled);
            if adj.is_null() {
                return;
            }
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

    fn parse_constructor_handler(handler: &str) -> (String, String) {
        if let Some(paren_pos) = handler.find('(') {
            let name = handler[..paren_pos].to_string();
            let arg = handler[paren_pos + 1..handler.len().saturating_sub(1)].to_string();
            (name, arg)
        } else {
            (handler.to_string(), String::new())
        }
    }

    fn make_signal_event(event: SignalEventState, widget_name: String) -> SignalEvent {
        SignalEvent {
            widget_id: event.widget_id,
            widget_name,
            signal: event.signal,
            handler: event.handler,
            payload: event.payload,
        }
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
        // Collect all deferred GTK mutations while holding the borrow,
        // then execute them after releasing it.  GTK C calls (g_object_set,
        // gtk_widget_add_css_class, …) can re-enter this callback via
        // signal emissions, so the RefCell must NOT be borrowed during those
        // calls.
        enum DeferredMutation {
            SetBool {
                widget: *mut c_void,
                property: CString,
                value: c_int,
            },
            ToggleBool {
                widget: *mut c_void,
                property: CString,
            },
            CssClass {
                widget: *mut c_void,
                class: CString,
                add: bool,
            },
            ToggleCssClass {
                widget: *mut c_void,
                class: CString,
            },
            PresentDialog {
                dialog: *mut c_void,
                parent: *mut c_void,
            },
            SetStackPage {
                stack: *mut c_void,
                page: CString,
            },
        }

        let deferred: Vec<DeferredMutation> = GTK_STATE.with(|state| {
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
            let typed_event = make_signal_event(event.clone(), widget_name);
            state
                .signal_senders
                .retain(|s| s.send(typed_event.clone()).is_ok());
            state.signal_events.push_back(event);

            let mut mutations = Vec::new();

            // Collect all signal action bindings for this handler
            if let Some(actions) = state.signal_action_bindings.get(&binding.handler) {
                for action in actions {
                    match action {
                        SignalAction::SetBool {
                            widget_id,
                            property,
                            value,
                        } => {
                            if let Some(&widget) = state.widgets.get(widget_id) {
                                if let Ok(prop_c) = CString::new(property.as_str()) {
                                    mutations.push(DeferredMutation::SetBool {
                                        widget,
                                        property: prop_c,
                                        value: bool_to_c(*value),
                                    });
                                }
                            }
                        }
                        SignalAction::CssClass {
                            widget_id,
                            class_name,
                            add,
                        } => {
                            if let Some(&widget) = state.widgets.get(widget_id) {
                                if let Ok(class_c) = CString::new(class_name.as_str()) {
                                    mutations.push(DeferredMutation::CssClass {
                                        widget,
                                        class: class_c,
                                        add: *add,
                                    });
                                }
                            }
                        }
                        SignalAction::ToggleBool {
                            widget_id,
                            property,
                        } => {
                            if let Some(&widget) = state.widgets.get(widget_id) {
                                if let Ok(prop_c) = CString::new(property.as_str()) {
                                    mutations.push(DeferredMutation::ToggleBool {
                                        widget,
                                        property: prop_c,
                                    });
                                }
                            }
                        }
                        SignalAction::ToggleCssClass {
                            widget_id,
                            class_name,
                        } => {
                            if let Some(&widget) = state.widgets.get(widget_id) {
                                if let Ok(class_c) = CString::new(class_name.as_str()) {
                                    mutations.push(DeferredMutation::ToggleCssClass {
                                        widget,
                                        class: class_c,
                                    });
                                }
                            }
                        }
                        SignalAction::PresentDialog {
                            dialog_id,
                            parent_id,
                        } => {
                            if let (Some(&dialog), Some(&parent)) =
                                (state.widgets.get(dialog_id), state.widgets.get(parent_id))
                            {
                                mutations.push(DeferredMutation::PresentDialog { dialog, parent });
                            }
                        }
                        SignalAction::SetStackPage {
                            stack_id,
                            page_name,
                        } => {
                            if let Some(&stack) = state.widgets.get(stack_id) {
                                if let Ok(page_c) = CString::new(page_name.as_str()) {
                                    mutations.push(DeferredMutation::SetStackPage {
                                        stack,
                                        page: page_c,
                                    });
                                }
                            }
                        }
                    }
                }
            }

            mutations
        });

        // Now apply all deferred GTK mutations without holding the RefCell borrow.
        for m in deferred {
            match m {
                DeferredMutation::SetBool {
                    widget,
                    property,
                    value,
                } => unsafe {
                    gobject_set_bool(widget, &property, value);
                },
                DeferredMutation::ToggleBool { widget, property } => unsafe {
                    let current = gobject_get_bool(widget, &property);
                    gobject_set_bool(widget, &property, if current != 0 { 0 } else { 1 });
                },
                DeferredMutation::CssClass { widget, class, add } => unsafe {
                    if add {
                        gtk_widget_add_css_class(widget, class.as_ptr());
                    } else {
                        gtk_widget_remove_css_class(widget, class.as_ptr());
                    }
                },
                DeferredMutation::ToggleCssClass { widget, class } => unsafe {
                    if gtk_widget_has_css_class(widget, class.as_ptr()) != 0 {
                        gtk_widget_remove_css_class(widget, class.as_ptr());
                    } else {
                        gtk_widget_add_css_class(widget, class.as_ptr());
                    }
                },
                DeferredMutation::PresentDialog { dialog, parent } => {
                    call_adw_fn_pp("adw_dialog_present", dialog, parent);
                }
                DeferredMutation::SetStackPage { stack, page } => unsafe {
                    gtk_stack_set_visible_child_name(stack, page.as_ptr());
                },
            }
        }
    }

    fn signal_payload_kind_for(class_name: &str, signal_name: &str) -> Option<SignalPayloadKind> {
        match (class_name, signal_name) {
            ("GtkButton", "clicked") => Some(SignalPayloadKind::None),
            ("GtkEntry", "changed")
            | ("GtkEntry", "activate")
            | ("GtkPasswordEntry", "changed")
            | ("GtkPasswordEntry", "activate")
            | ("AdwEntryRow", "changed")
            | ("AdwPasswordEntryRow", "changed") => Some(SignalPayloadKind::EditableText),
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
    ) -> Result<c_ulong, Gtk4Error> {
        let Some(payload_kind) = signal_payload_kind_for(class_name, &binding.signal) else {
            return Err(Gtk4Error::new(format!(
                "gtk4.buildFromNode unsupported signal `{}` on class `{class_name}`",
                binding.signal
            )));
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

    /// Starts the D-Bus server (MailfoxDesktopObject).
    fn spawn_dbus_server() -> Result<(), String> {
        std::thread::Builder::new()
            .name("mailfox-dbus".to_string())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("tokio rt");
                rt.block_on(async {
                    let conn = match zbus::Connection::session().await {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("mailfox-dbus: session error: {e}");
                            return;
                        }
                    };
                    let mailfox_dbus = MailfoxDesktopObject;
                    if let Err(e) = conn
                        .object_server()
                        .at("/com/mailfox/desktop", mailfox_dbus)
                        .await
                    {
                        eprintln!("mailfox-dbus: register error: {e}");
                    }
                    if let Err(e) = conn.request_name("com.mailfox.desktop.tray").await {
                        eprintln!("mailfox-dbus: request_name error: {e}");
                    }
                    // Emit BadgeUpdate and NewPersonalEmail D-Bus signals
                    let conn_dbus_sigs = conn.clone();
                    tokio::spawn(async move {
                        loop {
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            let badge_updates: Vec<i64> = pending_badge_updates()
                                .lock()
                                .map(|mut q| q.drain(..).collect())
                                .unwrap_or_default();
                            for count in badge_updates {
                                if let Ok(iface_ref) = conn_dbus_sigs
                                    .object_server()
                                    .interface::<_, MailfoxDesktopObject>("/com/mailfox/desktop")
                                    .await
                                {
                                    let _ = MailfoxDesktopObject::badge_update(
                                        iface_ref.signal_emitter(),
                                        count as i32,
                                    )
                                    .await;
                                }
                            }
                            let emails: Vec<PersonalEmailNotif> = pending_personal_emails()
                                .lock()
                                .map(|mut q| q.drain(..).collect())
                                .unwrap_or_default();
                            for email in emails {
                                if let Ok(iface_ref) = conn_dbus_sigs
                                    .object_server()
                                    .interface::<_, MailfoxDesktopObject>("/com/mailfox/desktop")
                                    .await
                                {
                                    let _ = MailfoxDesktopObject::new_personal_email(
                                        iface_ref.signal_emitter(),
                                        &email.id,
                                        &email.from,
                                        &email.subject,
                                        &email.markdown_body,
                                    )
                                    .await;
                                }
                            }
                        }
                    });
                    std::future::pending::<()>().await;
                });
            })
            .map(|_| ())
            .map_err(|e| format!("spawn dbus server thread: {e}"))
    }

    pub(super) fn dbus_server_start() -> Result<(), Gtk4Error> {
        spawn_dbus_server().map_err(|e| Gtk4Error::new(format!("gtk4.dbusServerStart: {e}")))
    }

    struct MailfoxDesktopObject;

    #[zbus::interface(name = "com.mailfox.Desktop")]
    impl MailfoxDesktopObject {
        fn action(&self, action: String) {
            eprintln!("mailfox-dbus: Action({}) called", action);
            if let Ok(mut q) = pending_tray_actions().lock() {
                q.push_back(action);
            }
        }

        fn send_reply(&self, email_id: String, body: String) {
            if let Ok(mut q) = pending_tray_actions().lock() {
                q.push_back(format!("send_reply:{email_id}:{body}"));
            }
        }

        fn send_compose(&self, to: String, subject: String, body: String) {
            if let Ok(mut q) = pending_tray_actions().lock() {
                q.push_back(format!("send_compose:{to}:{subject}:{body}"));
            }
        }

        fn open_email(&self, email_id: String) {
            if let Ok(mut q) = pending_tray_actions().lock() {
                q.push_back(format!("open_email:{email_id}"));
            }
        }

        fn get_email_suggestions(&self, prefix: String) -> Vec<String> {
            email_suggestions()
                .lock()
                .map(|sug| {
                    let lc = prefix.to_lowercase();
                    sug.iter()
                        .filter(|s| s.to_lowercase().contains(&lc))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default()
        }

        #[zbus(signal)]
        async fn badge_update(
            emitter: &zbus::object_server::SignalEmitter<'_>,
            count: i32,
        ) -> zbus::Result<()>;

        #[zbus(signal)]
        async fn new_personal_email(
            emitter: &zbus::object_server::SignalEmitter<'_>,
            id: &str,
            from: &str,
            subject: &str,
            markdown_body: &str,
        ) -> zbus::Result<()>;
    }

    // ── Widget Builder ──
    fn create_adw_widget_type(type_name: &str) -> Result<*mut c_void, Gtk4Error> {
        try_adw_init();
        let class_c = c_text(type_name, "gtk4.buildFromNode invalid Adw class name")?;
        let g_type = unsafe { g_type_from_name(class_c.as_ptr()) };
        if g_type == 0 {
            return Err(Gtk4Error::new(format!(
                "gtk4.buildFromNode unknown Adw class {type_name}"
            )));
        }
        let raw = unsafe { g_object_new(g_type, std::ptr::null::<c_char>()) };
        if raw.is_null() {
            return Err(Gtk4Error::new(format!(
                "gtk4.buildFromNode failed to create {type_name}"
            )));
        }
        Ok(raw)
    }

    fn create_adw_widget(class_name: &str) -> Result<*mut c_void, Gtk4Error> {
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
            _ => Err(Gtk4Error::new(format!(
                "gtk4.buildFromNode unsupported class {class_name}"
            ))),
        }
    }

    fn collect_object_properties(
        attrs: &[(String, String)],
        children: &[GtkNode],
    ) -> HashMap<String, String> {
        let mut out = HashMap::new();
        for (name, value) in attrs {
            if let Some(prop) = name.strip_prefix("prop:") {
                out.insert(prop.to_string(), value.clone());
            }
        }
        for child in children {
            let GtkNode::Element {
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
                    let GtkNode::Element {
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
        children: &[GtkNode],
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
            let GtkNode::Element {
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

    fn child_packing_position(children: &[GtkNode]) -> Option<usize> {
        for child in children {
            let GtkNode::Element {
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
                let GtkNode::Element {
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

    fn collect_child_objects(children: &[GtkNode]) -> Vec<ChildSpec<'_>> {
        let mut out = Vec::new();
        for child in children {
            let GtkNode::Element {
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
                        GtkNode::Element {
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
                        GtkNode::Element {
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

    fn first_object_in_interface(node: &GtkNode) -> Result<&GtkNode, Gtk4Error> {
        let GtkNode::Element { tag, children, .. } = node else {
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
        fn find_first_object(node: &GtkNode) -> Option<&GtkNode> {
            let GtkNode::Element { tag, children, .. } = node else {
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
        state: &RealGtkState,
    ) -> Result<(), Gtk4Error> {
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
            unsafe { gtk_widget_set_hexpand(widget, bool_to_c(value)) };
        }
        if let Some(value) = props.get("vexpand").and_then(|v| parse_bool_text(v)) {
            unsafe { gtk_widget_set_vexpand(widget, bool_to_c(value)) };
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
            unsafe { gtk_widget_set_visible(widget, bool_to_c(value)) };
        }
        if let Some(value) = props.get("sensitive").and_then(|v| parse_bool_text(v)) {
            unsafe { gtk_widget_set_sensitive(widget, bool_to_c(value)) };
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
                    unsafe { gtk_label_set_wrap(widget, bool_to_c(value)) };
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
            "GtkEntry" | "GtkPasswordEntry" => {
                if let Some(value) = props.get("text") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid GtkEntry text")?;
                    unsafe { gtk_editable_set_text(widget, text_c.as_ptr()) };
                }
                set_obj_str(widget, props, "placeholder-text", "GtkEntry")?;
                if class_name == "GtkPasswordEntry" {
                    if let Some(value) =
                        props.get("show-peek-icon").and_then(|v| parse_bool_text(v))
                    {
                        unsafe { gtk_password_entry_set_show_peek_icon(widget, bool_to_c(value)) };
                    }
                }
            }
            "GtkTextView" => {
                if let Some(value) = props.get("wrap-mode").and_then(|v| parse_wrap_mode_text(v)) {
                    unsafe { gtk_text_view_set_wrap_mode(widget, value) };
                }
                if let Some(value) = props.get("top-margin").and_then(|v| parse_i32_text(v)) {
                    unsafe { gtk_text_view_set_top_margin(widget, value) };
                }
                if let Some(value) = props.get("bottom-margin").and_then(|v| parse_i32_text(v)) {
                    unsafe { gtk_text_view_set_bottom_margin(widget, value) };
                }
                if let Some(value) = props.get("left-margin").and_then(|v| parse_i32_text(v)) {
                    unsafe { gtk_text_view_set_left_margin(widget, value) };
                }
                if let Some(value) = props.get("right-margin").and_then(|v| parse_i32_text(v)) {
                    unsafe { gtk_text_view_set_right_margin(widget, value) };
                }
                if let Some(value) = props.get("editable").and_then(|v| parse_bool_text(v)) {
                    unsafe { gtk_text_view_set_editable(widget, bool_to_c(value)) };
                }
                if let Some(value) = props.get("cursor-visible").and_then(|v| parse_bool_text(v)) {
                    unsafe { gtk_text_view_set_cursor_visible(widget, bool_to_c(value)) };
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
                    unsafe { gtk_box_set_homogeneous(widget, bool_to_c(value)) };
                }
            }
            "GtkHeaderBar" | "AdwHeaderBar" => {
                if let Some(value) = props.get("decoration-layout") {
                    let layout_c = c_text(
                        value,
                        "gtk4.buildFromNode invalid headerbar decoration-layout",
                    )?;
                    unsafe { gtk_header_bar_set_decoration_layout(widget, layout_c.as_ptr()) };
                }
                if let Some(value) = props
                    .get("show-title-buttons")
                    .or_else(|| props.get("show-end-title-buttons"))
                    .and_then(|v| parse_bool_text(v))
                {
                    unsafe { gtk_header_bar_set_show_title_buttons(widget, bool_to_c(value)) };
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
                        gtk_scrolled_window_set_propagate_natural_height(widget, bool_to_c(value))
                    };
                }
                if let Some(value) = props
                    .get("propagate-natural-width")
                    .and_then(|v| parse_bool_text(v))
                {
                    unsafe {
                        gtk_scrolled_window_set_propagate_natural_width(widget, bool_to_c(value))
                    };
                }
            }
            "AdwOverlaySplitView" => {
                if let Some(value) = props.get("sidebar-position") {
                    let pos: c_int = if value == "end" { 1 } else { 0 };
                    let prop_c = CString::new("sidebar-position").unwrap();
                    unsafe { gobject_set_bool(widget, &prop_c, pos) };
                }
                set_obj_bool(widget, props, "collapsed");
                set_obj_bool(widget, props, "show-sidebar");
                set_obj_f64(widget, props, "max-sidebar-width");
                set_obj_f64(widget, props, "min-sidebar-width");
                set_obj_f64(widget, props, "sidebar-width-fraction");
            }
            "AdwButtonContent" => {
                set_obj_str(widget, props, "label", "AdwButtonContent")?;
                set_obj_str(widget, props, "icon-name", "AdwButtonContent")?;
            }
            "GtkRevealer" => {
                if let Some(value) = props.get("transition-type") {
                    let t: c_int = match value.as_str() {
                        "none" => 0,
                        "crossfade" => 1,
                        "slide-right" => 2,
                        "slide-left" => 3,
                        "slide-up" => 4,
                        "slide-down" => 5,
                        "swing-right" => 6,
                        "swing-left" => 7,
                        "swing-up" => 8,
                        "swing-down" => 9,
                        _ => 0,
                    };
                    unsafe { gtk_revealer_set_transition_type(widget, t) };
                }
                if let Some(value) = props
                    .get("transition-duration")
                    .and_then(|v| v.parse::<u32>().ok())
                {
                    unsafe { gtk_revealer_set_transition_duration(widget, value) };
                }
                if let Some(value) = props.get("reveal-child").and_then(|v| parse_bool_text(v)) {
                    unsafe { gtk_revealer_set_reveal_child(widget, bool_to_c(value)) };
                }
            }
            "GtkMenuButton" => {
                set_obj_str(widget, props, "label", "GtkMenuButton")?;
                set_obj_str(widget, props, "icon-name", "GtkMenuButton")?;
                if let Some(id_str) = props.get("menu-model") {
                    if let Ok(id) = id_str.parse::<i64>() {
                        if let Some(&menu_raw) = state.widgets.get(&id) {
                            let prop_c = CString::new("menu-model").unwrap();
                            unsafe { gobject_set_ptr(widget, &prop_c, menu_raw) };
                        }
                    }
                }
            }
            "AdwPreferencesDialog" => {
                set_obj_str(widget, props, "title", "AdwPreferencesDialog")?;
            }
            "AdwPreferencesPage" => {
                set_obj_str(widget, props, "title", "AdwPreferencesPage")?;
                set_obj_str(widget, props, "icon-name", "AdwPreferencesPage")?;
                set_obj_str(widget, props, "name", "AdwPreferencesPage")?;
            }
            "AdwPreferencesGroup" => {
                set_obj_str(widget, props, "title", "AdwPreferencesGroup")?;
                set_obj_str(widget, props, "description", "AdwPreferencesGroup")?;
            }
            "AdwActionRow" | "AdwExpanderRow" | "AdwPreferencesRow" | "AdwSpinRow" => {
                set_obj_str(widget, props, "title", "AdwActionRow")?;
                set_obj_str(widget, props, "subtitle", "AdwActionRow")?;
            }
            "AdwEntryRow" | "AdwPasswordEntryRow" => {
                set_obj_str(widget, props, "title", "AdwEntryRow")?;
                if let Some(value) = props.get("text") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid AdwEntryRow text")?;
                    unsafe { gtk_editable_set_text(widget, text_c.as_ptr()) };
                }
            }
            "AdwSwitchRow" => {
                set_obj_str(widget, props, "title", "AdwSwitchRow")?;
                set_obj_str(widget, props, "subtitle", "AdwSwitchRow")?;
                set_obj_bool(widget, props, "active");
            }
            _ => {}
        }
        Ok(())
    }

    fn build_widget_from_node_real(
        state: &mut RealGtkState,
        node: &GtkNode,
        id_map: &mut HashMap<String, i64>,
    ) -> Result<(i64, LiveNode), Gtk4Error> {
        let GtkNode::Element {
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
            let wid = id_map.get(ref_id).copied().ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk4.buildFromNode unresolved object reference id '{ref_id}'"
                ))
            })?;
            // Referenced nodes are aliases — return a minimal LiveNode (no children to reconcile).
            let live = LiveNode {
                widget_id: wid,
                class_name: String::new(),
                kind: CreatedWidgetKind::Other,
                node_id: Some(ref_id.to_string()),
                props: HashMap::new(),
                signals: Vec::new(),
                signal_handler_ids: Vec::new(),
                children: Vec::new(),
            };
            return Ok((wid, live));
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
            "GtkHeaderBar" | "AdwHeaderBar" => (
                unsafe { gtk_header_bar_new() },
                CreatedWidgetKind::HeaderBar,
            ),
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
                        CreatedWidgetKind::Button,
                    )
                } else {
                    let label = props.get("label").cloned().unwrap_or_default();
                    let label_c = c_text(&label, "gtk4.buildFromNode invalid GtkButton label")?;
                    (
                        unsafe { gtk_button_new_with_label(label_c.as_ptr()) },
                        CreatedWidgetKind::Button,
                    )
                }
            }
            "GtkEntry" => (unsafe { gtk_entry_new() }, CreatedWidgetKind::Other),
            "GtkPasswordEntry" => (
                unsafe { gtk_password_entry_new() },
                CreatedWidgetKind::Other,
            ),
            "GtkTextView" => (unsafe { gtk_text_view_new() }, CreatedWidgetKind::Other),
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
            "GtkMenuButton" => (unsafe { gtk_menu_button_new() }, CreatedWidgetKind::Other),
            "GtkStack" => (unsafe { gtk_stack_new() }, CreatedWidgetKind::Stack),
            "GtkRevealer" => (unsafe { gtk_revealer_new() }, CreatedWidgetKind::Revealer),
            "AdwOverlaySplitView" => (create_adw_widget(class_name)?, CreatedWidgetKind::SplitView),
            "AdwPreferencesDialog" => (
                create_adw_widget(class_name)?,
                CreatedWidgetKind::PreferencesDialog,
            ),
            "AdwPreferencesPage" => (
                create_adw_widget(class_name)?,
                CreatedWidgetKind::PreferencesPage,
            ),
            "AdwPreferencesGroup" => (
                create_adw_widget(class_name)?,
                CreatedWidgetKind::PreferencesGroup,
            ),
            "AdwActionRow" => (create_adw_widget(class_name)?, CreatedWidgetKind::ActionRow),
            "AdwExpanderRow" => (create_adw_widget(class_name)?, CreatedWidgetKind::ActionRow),
            "AdwAboutDialog"
            | "AdwAboutWindow"
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
            | "AdwClampLayout"
            | "AdwClampScrollable"
            | "AdwComboRow"
            | "AdwDialog"
            | "AdwEntryRow"
            | "AdwEnumListModel"
            | "AdwFlap"
            | "AdwInlineViewSwitcher"
            | "AdwLayout"
            | "AdwLayoutSlot"
            | "AdwLeaflet"
            | "AdwMessageDialog"
            | "AdwMultiLayoutView"
            | "AdwNavigationPage"
            | "AdwNavigationSplitView"
            | "AdwNavigationView"
            | "AdwPasswordEntryRow"
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
                return Err(Gtk4Error::new(format!(
                    "gtk4.buildFromNode unsupported class {class_name}"
                )));
            }
        };
        if raw.is_null() {
            return Err(Gtk4Error::new(format!(
                "gtk4.buildFromNode failed to create {class_name}"
            )));
        }

        let id = state.alloc_id();
        if let Some(object_id) = node_attr(attrs, "id") {
            id_map.insert(object_id.to_string(), id);
            if let Ok(name_c) = CString::new(object_id.as_bytes()) {
                unsafe {
                    // Set the GTK CSS widget name (used for styling/lookup)
                    gtk_widget_set_name(raw, name_c.as_ptr());
                    // For GtkBox (which defaults to GTK_ACCESSIBLE_ROLE_GENERIC),
                    // upgrade to GROUP (18) so the accessible name is exposed by AT-SPI.
                    // GTK_ACCESSIBLE_ROLE_GROUP = 18
                    if matches!(class_name, "GtkBox") {
                        if let Ok(role_prop) = CString::new("accessible-role") {
                            gobject_set_bool(raw, &role_prop, 18);
                        }
                    }
                    // Set the AT-SPI accessible label so AT-SPI clients can find
                    // widgets by their id. GTK_ACCESSIBLE_PROPERTY_LABEL = 4,
                    // terminated by -1.
                    gtk_accessible_update_property(
                        raw,
                        4i32,            // GTK_ACCESSIBLE_PROPERTY_LABEL
                        name_c.as_ptr(), // label value (const char*)
                        -1i32,           // sentinel
                    );
                }
            }
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
            "GtkEntry" | "GtkPasswordEntry" => {
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

        apply_widget_properties(raw, class_name, &props, state)?;
        let mut signal_handler_ids = Vec::new();
        for binding in &signal_bindings {
            let hid = connect_widget_signal(raw, id, class_name, binding)?;
            signal_handler_ids.push(hid);
        }

        let mut child_objects = collect_child_objects(children);
        child_objects.sort_by_key(|child| child.position.unwrap_or(usize::MAX));
        let mut overlay_root_set = false;
        let mut live_children: Vec<LiveChild> = Vec::new();
        // For auto-wiring scroll fades inside GtkOverlay
        let mut scroll_fade_scrolled: *mut c_void = std::ptr::null_mut();
        let mut scroll_fade_top: *mut c_void = std::ptr::null_mut();
        let mut scroll_fade_bottom: *mut c_void = std::ptr::null_mut();
        for child in child_objects {
            // Track child CSS class for scroll-fade auto-wiring
            let child_css = if matches!(kind, CreatedWidgetKind::Overlay) {
                if let GtkNode::Element {
                    attrs,
                    children: cc,
                    ..
                } = child.node
                {
                    let p = collect_object_properties(attrs, cc);
                    p.get("css-class").cloned().unwrap_or_default()
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            let child_class_name = if matches!(kind, CreatedWidgetKind::Overlay) {
                if let GtkNode::Element { attrs, .. } = child.node {
                    node_attr(attrs, "class").unwrap_or("").to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            let (child_id, child_live) = build_widget_from_node_real(state, child.node, id_map)?;
            let child_raw = widget_ptr(state, child_id, "buildFromNode")?;

            // Track for scroll-fade auto-wiring
            if matches!(kind, CreatedWidgetKind::Overlay) {
                if child_class_name == "GtkScrolledWindow" && child_css.contains("fading-scroll") {
                    scroll_fade_scrolled = child_raw;
                }
                if child_css.contains("fade-top") {
                    scroll_fade_top = child_raw;
                }
                if child_css.contains("fade-bottom") {
                    scroll_fade_bottom = child_raw;
                }
            }
            if child.child_type.as_deref() == Some("controller") {
                unsafe { gtk_widget_add_controller(raw, child_raw) };
                if let Some(gesture) = state.gesture_clicks.get_mut(&child_id) {
                    gesture.widget_id = id;
                }
                live_children.push(LiveChild {
                    child_type: child.child_type.clone(),
                    node: child_live,
                });
                continue;
            }
            match kind {
                CreatedWidgetKind::Box => unsafe { gtk_box_append(raw, child_raw) },
                CreatedWidgetKind::Button => unsafe { gtk_button_set_child(raw, child_raw) },
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
                CreatedWidgetKind::Revealer => unsafe { gtk_revealer_set_child(raw, child_raw) },
                CreatedWidgetKind::Stack => {
                    let page_name = child.child_type.as_deref().unwrap_or("page");
                    if let Ok(name_c) = CString::new(page_name) {
                        unsafe { gtk_stack_add_named(raw, child_raw, name_c.as_ptr()) };
                    }
                }
                CreatedWidgetKind::SplitView => {
                    let prop_name = match child.child_type.as_deref() {
                        Some("sidebar") => "sidebar",
                        _ => "content",
                    };
                    let prop_c = CString::new(prop_name).unwrap();
                    unsafe { gobject_set_ptr(raw, &prop_c, child_raw) };
                }
                CreatedWidgetKind::Other => {}
                CreatedWidgetKind::PreferencesDialog => {
                    call_adw_fn_pp("adw_preferences_dialog_add", raw, child_raw);
                }
                CreatedWidgetKind::PreferencesPage => {
                    call_adw_fn_pp("adw_preferences_page_add", raw, child_raw);
                }
                CreatedWidgetKind::PreferencesGroup => {
                    call_adw_fn_pp("adw_preferences_group_add", raw, child_raw);
                }
                CreatedWidgetKind::ActionRow => {
                    call_adw_fn_pp("adw_action_row_add_suffix", raw, child_raw);
                }
            }
            live_children.push(LiveChild {
                child_type: child.child_type.clone(),
                node: child_live,
            });
        }

        // Auto-wire scroll fades for GtkOverlay containing a fading-scroll scrolled window.
        if !scroll_fade_scrolled.is_null()
            && (!scroll_fade_top.is_null() || !scroll_fade_bottom.is_null())
        {
            wire_scroll_fades(scroll_fade_scrolled, scroll_fade_top, scroll_fade_bottom);
        }

        let node_id = node_attr(attrs, "id").map(str::to_string);
        let live = LiveNode {
            widget_id: id,
            class_name: class_name.to_string(),
            kind,
            node_id,
            props,
            signals: signal_bindings,
            signal_handler_ids,
            children: live_children,
        };
        Ok((id, live))
    }

    // ── Reconciler ──
    // ── Reconciliation ────────────────────────────────────────────────────────

    /// Patch CSS classes: remove old-only, add new-only.
    fn patch_css_classes(
        widget: *mut c_void,
        old_css: Option<&String>,
        new_css: Option<&String>,
    ) -> Result<(), Gtk4Error> {
        let old_set: std::collections::HashSet<&str> = old_css
            .map(|s| s.split_whitespace().collect())
            .unwrap_or_default();
        let new_set: std::collections::HashSet<&str> = new_css
            .map(|s| s.split_whitespace().collect())
            .unwrap_or_default();
        for cls in old_set.difference(&new_set) {
            let c = c_text(cls, "reconcile css remove")?;
            unsafe { gtk_widget_remove_css_class(widget, c.as_ptr()) };
        }
        for cls in new_set.difference(&old_set) {
            let c = c_text(cls, "reconcile css add")?;
            unsafe { gtk_widget_add_css_class(widget, c.as_ptr()) };
        }
        Ok(())
    }

    /// Patch properties on an existing widget. Reapplies all new props and diffs CSS classes.
    fn patch_widget_properties(
        widget: *mut c_void,
        class_name: &str,
        old_props: &HashMap<String, String>,
        new_props: &HashMap<String, String>,
        state: &RealGtkState,
    ) -> Result<(), Gtk4Error> {
        // Diff CSS classes specially
        patch_css_classes(
            widget,
            old_props.get("css-class"),
            new_props.get("css-class"),
        )?;
        // Apply the non-CSS properties unconditionally (cheap FFI calls, only changed props matter)
        let mut props_no_css = new_props.clone();
        props_no_css.remove("css-class");
        apply_widget_properties(widget, class_name, &props_no_css, state)?;
        Ok(())
    }

    /// Recursively clean up state entries for a widget and its children in the live tree.
    fn cleanup_widget_state(state: &mut RealGtkState, live: &LiveNode) {
        let id = live.widget_id;
        state.widgets.remove(&id);
        state.labels.remove(&id);
        state.entries.remove(&id);
        state.images.remove(&id);
        state.draw_areas.remove(&id);
        state.gesture_clicks.remove(&id);
        state.separators.remove(&id);
        if let Some(ref name) = live.node_id {
            state.named_widgets.remove(name);
        }
        state.widget_id_to_name.remove(&id);
        state.live_trees.remove(&id);
        for child in &live.children {
            cleanup_widget_state(state, &child.node);
        }
    }

    /// Remove a child widget from its parent container and clean up state.
    fn remove_child_from_parent(
        state: &mut RealGtkState,
        parent_id: i64,
        parent_kind: CreatedWidgetKind,
        child_id: i64,
        child_type: Option<&str>,
        child_live: Option<&LiveNode>,
    ) -> Result<(), Gtk4Error> {
        let parent_raw = widget_ptr(state, parent_id, "reconcile")?;
        let child_raw = widget_ptr(state, child_id, "reconcile")?;
        match parent_kind {
            CreatedWidgetKind::Box => unsafe { gtk_box_remove(parent_raw, child_raw) },
            CreatedWidgetKind::ListBox => unsafe { gtk_list_box_remove(parent_raw, child_raw) },
            CreatedWidgetKind::Overlay => {
                if child_type == Some("overlay") {
                    unsafe { gtk_overlay_remove_overlay(parent_raw, child_raw) };
                } else {
                    unsafe { gtk_overlay_set_child(parent_raw, std::ptr::null_mut()) };
                }
            }
            _ => unsafe { gtk_widget_unparent(child_raw) },
        }
        if let Some(live) = child_live {
            let live_clone = live.clone();
            cleanup_widget_state(state, &live_clone);
        }
        Ok(())
    }

    /// Add a child widget to a parent container (mirrors the build logic).
    fn add_child_to_parent(
        parent_raw: *mut c_void,
        parent_kind: CreatedWidgetKind,
        child_raw: *mut c_void,
        child_type: Option<&str>,
        overlay_index: usize,
    ) {
        match parent_kind {
            CreatedWidgetKind::Box => unsafe { gtk_box_append(parent_raw, child_raw) },
            CreatedWidgetKind::Button => unsafe { gtk_button_set_child(parent_raw, child_raw) },
            CreatedWidgetKind::HeaderBar => match child_type {
                Some("end") => unsafe { gtk_header_bar_pack_end(parent_raw, child_raw) },
                Some("title") => unsafe { gtk_header_bar_set_title_widget(parent_raw, child_raw) },
                _ => unsafe { gtk_header_bar_pack_start(parent_raw, child_raw) },
            },
            CreatedWidgetKind::ScrolledWindow => {
                if child_type != Some("overlay") {
                    unsafe { gtk_scrolled_window_set_child(parent_raw, child_raw) };
                }
            }
            CreatedWidgetKind::Overlay => {
                if child_type == Some("overlay") || overlay_index > 0 {
                    unsafe { gtk_overlay_add_overlay(parent_raw, child_raw) };
                } else {
                    unsafe { gtk_overlay_set_child(parent_raw, child_raw) };
                }
            }
            CreatedWidgetKind::ListBox => unsafe { gtk_list_box_append(parent_raw, child_raw) },
            CreatedWidgetKind::Revealer => unsafe { gtk_revealer_set_child(parent_raw, child_raw) },
            CreatedWidgetKind::Stack => {
                let page_name = child_type.unwrap_or("page");
                if let Ok(name_c) = CString::new(page_name) {
                    unsafe { gtk_stack_add_named(parent_raw, child_raw, name_c.as_ptr()) };
                }
            }
            CreatedWidgetKind::SplitView => {
                let prop_name = match child_type {
                    Some("sidebar") => "sidebar",
                    _ => "content",
                };
                let prop_c = CString::new(prop_name).unwrap();
                unsafe { gobject_set_ptr(parent_raw, &prop_c, child_raw) };
            }
            CreatedWidgetKind::PreferencesDialog => {
                call_adw_fn_pp("adw_preferences_dialog_add", parent_raw, child_raw);
            }
            CreatedWidgetKind::PreferencesPage => {
                call_adw_fn_pp("adw_preferences_page_add", parent_raw, child_raw);
            }
            CreatedWidgetKind::PreferencesGroup => {
                call_adw_fn_pp("adw_preferences_group_add", parent_raw, child_raw);
            }
            CreatedWidgetKind::ActionRow => {
                call_adw_fn_pp("adw_action_row_add_suffix", parent_raw, child_raw);
            }
            CreatedWidgetKind::Other => {}
        }
    }

    /// Reconcile a single live node against a new GtkNode.
    /// Returns the (possibly replaced) LiveNode.
    fn reconcile_node(
        state: &mut RealGtkState,
        live: &mut LiveNode,
        new_node: &GtkNode,
        id_map: &mut HashMap<String, i64>,
    ) -> Result<bool, Gtk4Error> {
        let GtkNode::Element {
            tag,
            attrs,
            children,
        } = new_node
        else {
            return Ok(false);
        };
        if tag != "object" {
            return Ok(false);
        }
        let new_class = match node_attr(attrs, "class") {
            Some(c) => c,
            None => return Ok(false),
        };
        // Different widget class → cannot patch in-place
        if live.class_name != new_class {
            return Ok(false);
        }
        let new_props = collect_object_properties(attrs, children);
        let new_signals = collect_object_signals(attrs, children);
        let raw = widget_ptr(state, live.widget_id, "reconcile")?;

        // Disconnect old signals BEFORE patching properties to prevent
        // re-entrant GTK_STATE borrows: property changes (e.g. setting text)
        // can fire signals synchronously, which would try to borrow_mut the
        // already-borrowed state.
        for &hid in &live.signal_handler_ids {
            if hid != 0 {
                unsafe { g_signal_handler_disconnect(raw, hid) };
            }
        }

        // Patch properties (safe now — no signals connected)
        patch_widget_properties(raw, new_class, &live.props, &new_props, state)?;
        live.props = new_props;

        // Reconnect signals
        let mut new_handler_ids = Vec::new();
        for binding in &new_signals {
            let hid = connect_widget_signal(raw, live.widget_id, new_class, binding)?;
            new_handler_ids.push(hid);
        }
        live.signals = new_signals;
        live.signal_handler_ids = new_handler_ids;

        // Update node_id if it changed
        let new_node_id = node_attr(attrs, "id").map(str::to_string);
        if new_node_id != live.node_id {
            if let Some(ref name) = new_node_id {
                id_map.insert(name.clone(), live.widget_id);
            }
            live.node_id = new_node_id;
        }

        // Reconcile children
        let mut new_child_objects = collect_child_objects(children);
        new_child_objects.sort_by_key(|child| child.position.unwrap_or(usize::MAX));
        reconcile_children(state, live, &new_child_objects, id_map)?;

        Ok(true)
    }

    /// Reconcile the children of a live node against new child specs.
    fn reconcile_children(
        state: &mut RealGtkState,
        parent: &mut LiveNode,
        new_children: &[ChildSpec<'_>],
        id_map: &mut HashMap<String, i64>,
    ) -> Result<(), Gtk4Error> {
        let parent_id = parent.widget_id;
        let parent_kind = parent.kind;
        let min_len = parent.children.len().min(new_children.len());

        // Reconcile overlapping positions
        #[allow(clippy::needless_range_loop)]
        for i in 0..min_len {
            let patched = reconcile_node(
                state,
                &mut parent.children[i].node,
                new_children[i].node,
                id_map,
            )?;
            if !patched {
                // Different type — remove old, build new, insert at same position
                let old_wid = parent.children[i].node.widget_id;
                let old_ct = parent.children[i].child_type.clone();
                let old_live = parent.children[i].node.clone();
                remove_child_from_parent(
                    state,
                    parent_id,
                    parent_kind,
                    old_wid,
                    old_ct.as_deref(),
                    Some(&old_live),
                )?;
                let (new_id, new_live) =
                    build_widget_from_node_real(state, new_children[i].node, id_map)?;
                let new_raw = widget_ptr(state, new_id, "reconcile")?;
                let parent_raw = widget_ptr(state, parent_id, "reconcile")?;
                // For GtkBox, insert at position using the previous sibling
                if matches!(parent_kind, CreatedWidgetKind::Box) && i > 0 {
                    let prev_id = parent.children[i - 1].node.widget_id;
                    if let Ok(prev_raw) = widget_ptr(state, prev_id, "reconcile") {
                        unsafe { gtk_box_insert_child_after(parent_raw, new_raw, prev_raw) };
                    } else {
                        add_child_to_parent(
                            parent_raw,
                            parent_kind,
                            new_raw,
                            new_children[i].child_type.as_deref(),
                            i,
                        );
                    }
                } else {
                    add_child_to_parent(
                        parent_raw,
                        parent_kind,
                        new_raw,
                        new_children[i].child_type.as_deref(),
                        i,
                    );
                }
                parent.children[i] = LiveChild {
                    child_type: new_children[i].child_type.clone(),
                    node: new_live,
                };
            } else {
                parent.children[i].child_type = new_children[i].child_type.clone();
            }
        }

        // Remove excess old children (iterate in reverse so indices stay valid)
        for i in (min_len..parent.children.len()).rev() {
            let old_wid = parent.children[i].node.widget_id;
            let old_ct = parent.children[i].child_type.clone();
            let old_live = parent.children[i].node.clone();
            let _ = remove_child_from_parent(
                state,
                parent_id,
                parent_kind,
                old_wid,
                old_ct.as_deref(),
                Some(&old_live),
            );
        }
        parent.children.truncate(min_len);

        // Build and add new children
        for (i, new_spec) in new_children.iter().enumerate().skip(min_len) {
            let (new_id, new_live) = build_widget_from_node_real(state, new_spec.node, id_map)?;
            let new_raw = widget_ptr(state, new_id, "reconcile")?;
            let parent_raw = widget_ptr(state, parent_id, "reconcile")?;
            add_child_to_parent(
                parent_raw,
                parent_kind,
                new_raw,
                new_spec.child_type.as_deref(),
                i,
            );
            parent.children.push(LiveChild {
                child_type: new_spec.child_type.clone(),
                node: new_live,
            });
        }
        Ok(())
    }

    pub(super) fn is_pump_active() -> bool {
        GTK_PUMP_ACTIVE.with(|active| *active.borrow())
    }

    pub(super) fn pump_gtk_events() {
        GTK_PUMP_ACTIVE.with(|active| {
            if *active.borrow() {
                unsafe {
                    let ctx = g_main_context_default();
                    while g_main_context_pending(ctx) != 0 {
                        g_main_context_iteration(ctx, 0);
                    }
                }
                let actions: Vec<String> = pending_tray_actions()
                    .lock()
                    .map(|mut q| q.drain(..).collect())
                    .unwrap_or_default();
                for raw_action in actions {
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
                    let typed_event = make_signal_event(event.clone(), String::new());
                    GTK_STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        state
                            .signal_senders
                            .retain(|s| s.send(typed_event.clone()).is_ok());
                        state.signal_events.push_back(event);
                    });
                }
            }
        });
    }

    // ── Public API Functions ──────────────────────────────────────────────────

    pub(super) fn init() -> Result<(), Gtk4Error> {
        unsafe { gtk_init() };
        try_adw_init();
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            if !state.resources_registered {
                maybe_register_gresource_bundle()?;
                state.resources_registered = true;
            }
            Ok(())
        })
    }

    pub(super) fn app_new(id: &str) -> Result<i64, Gtk4Error> {
        let app_id_c = c_text(id, "gtk4.appNew invalid application id")?;
        unsafe { gtk_init() };
        try_adw_init();
        let raw = unsafe { gtk_application_new(app_id_c.as_ptr(), 0) };
        if raw.is_null() {
            return Err(Gtk4Error::new(
                "gtk4.appNew failed to create GTK application",
            ));
        }
        unsafe { g_object_ref_sink(raw) };
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
        GTK_PUMP_ACTIVE.with(|active| *active.borrow_mut() = true);
        Ok(id)
    }

    pub(super) fn app_run(app_id: i64) -> Result<(), Gtk4Error> {
        let app = GTK_STATE.with(|state| {
            state
                .borrow()
                .apps
                .get(&app_id)
                .copied()
                .ok_or_else(|| Gtk4Error::new(format!("gtk4.appRun unknown app id {app_id}")))
        })?;
        unsafe {
            let _ = g_application_run(app, 0, null_mut());
        }
        Ok(())
    }

    pub(super) fn app_set_css(_app_id: i64, css: &str) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.pending_css_texts.push(css.to_string());
            if !state.windows.is_empty() {
                apply_pending_display_customizations(&mut state)?;
            }
            Ok(())
        })
    }

    pub(super) fn window_new(
        app_id: i64,
        title: &str,
        width: i32,
        height: i32,
    ) -> Result<i64, Gtk4Error> {
        let title_c = c_text(title, "gtk4.windowNew invalid title")?;
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let _ =
                state.apps.get(&app_id).copied().ok_or_else(|| {
                    Gtk4Error::new(format!("gtk4.windowNew unknown app id {app_id}"))
                })?;
            let window = unsafe { gtk_window_new() };
            if window.is_null() {
                return Err(Gtk4Error::new("gtk4.windowNew failed to create window"));
            }
            unsafe {
                gtk_window_set_title(window, title_c.as_ptr());
                gtk_window_set_default_size(window, width, height);
            }
            let id = state.alloc_id();
            state.windows.insert(id, window);
            state.widgets.insert(id, window);
            apply_pending_display_customizations(&mut state)?;
            Ok(id)
        })
    }

    pub(super) fn window_set_title(win_id: i64, title: &str) -> Result<(), Gtk4Error> {
        let title_c = c_text(title, "gtk4.windowSetTitle invalid title")?;
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let window = state.windows.get(&win_id).copied().ok_or_else(|| {
                Gtk4Error::new(format!("gtk4.windowSetTitle unknown window id {win_id}"))
            })?;
            unsafe { gtk_window_set_title(window, title_c.as_ptr()) };
            Ok(())
        })
    }

    pub(super) fn window_set_titlebar(win_id: i64, titlebar_id: i64) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let window = state.windows.get(&win_id).copied().ok_or_else(|| {
                Gtk4Error::new(format!("gtk4.windowSetTitlebar unknown window id {win_id}"))
            })?;
            let titlebar = widget_ptr(&state, titlebar_id, "windowSetTitlebar")?;
            unsafe { gtk_window_set_titlebar(window, titlebar) };
            Ok(())
        })
    }

    pub(super) fn window_set_child(win_id: i64, child_id: i64) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let window = state.windows.get(&win_id).copied().ok_or_else(|| {
                Gtk4Error::new(format!("gtk4.windowSetChild unknown window id {win_id}"))
            })?;
            let child = widget_ptr(&state, child_id, "windowSetChild")?;
            unsafe { gtk_window_set_child(window, child) };
            Ok(())
        })
    }

    pub(super) fn window_present(win_id: i64) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let window = state.windows.get(&win_id).copied().ok_or_else(|| {
                Gtk4Error::new(format!("gtk4.windowPresent unknown window id {win_id}"))
            })?;
            unsafe { gtk_window_present(window) };
            Ok(())
        })
    }

    pub(super) fn window_close(win_id: i64) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let window = state.windows.get(&win_id).copied().ok_or_else(|| {
                Gtk4Error::new(format!("gtk4.windowClose unknown window id {win_id}"))
            })?;
            unsafe { gtk_window_close(window) };
            Ok(())
        })
    }

    pub(super) fn window_on_close(win_id: i64, signal_name: &str) -> Result<(), Gtk4Error> {
        unsafe extern "C" fn on_close_request(_instance: *mut c_void, data: *mut c_void) -> c_int {
            if data.is_null() {
                return 0;
            }
            let signal_name = unsafe { &*(data as *const String) };
            GTK_STATE.with(|state| {
                let mut state = state.borrow_mut();
                let event = SignalEventState {
                    widget_id: 0,
                    signal: "close-request".to_string(),
                    handler: signal_name.clone(),
                    payload: String::new(),
                };
                let typed_event = make_signal_event(event.clone(), signal_name.clone());
                state
                    .signal_senders
                    .retain(|s| s.send(typed_event.clone()).is_ok());
                state.signal_events.push_back(event);
            });
            0
        }

        let signal_name_owned = signal_name.to_string();
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let window = widget_ptr(&state, win_id, "windowOnClose")?;
            let name_box = Box::new(signal_name_owned);
            let data_ptr = Box::into_raw(name_box) as *mut c_void;
            let sig = CString::new("close-request")
                .map_err(|_| Gtk4Error::new("gtk4.windowOnClose: invalid signal name"))?;
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
            Ok(())
        })
    }

    pub(super) fn window_set_hide_on_close(win_id: i64, hide: bool) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let window = state.windows.get(&win_id).copied().ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk4.windowSetHideOnClose unknown window id {win_id}"
                ))
            })?;
            unsafe { gtk_window_set_hide_on_close(window, bool_to_c(hide)) };
            Ok(())
        })
    }

    pub(super) fn window_set_decorated(win_id: i64, decorated: bool) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let window = *state.windows.get(&win_id).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk4.windowSetDecorated unknown window id {win_id}"
                ))
            })?;
            unsafe { gtk_window_set_decorated(window, bool_to_c(decorated)) };
            Ok(())
        })
    }

    pub(super) fn widget_show(id: i64) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let widget = widget_ptr(&state, id, "widgetShow")?;
            unsafe { gtk_widget_set_visible(widget, 1) };
            Ok(())
        })
    }

    pub(super) fn widget_hide(id: i64) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let widget = widget_ptr(&state, id, "widgetHide")?;
            unsafe { gtk_widget_set_visible(widget, 0) };
            Ok(())
        })
    }

    pub(super) fn widget_set_bool_property(
        id: i64,
        prop: &str,
        value: bool,
    ) -> Result<(), Gtk4Error> {
        let prop_c = c_text(prop, "gtk4.widgetSetBoolProperty invalid property name")?;
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let widget = widget_ptr(&state, id, "widgetSetBoolProperty")?;
            unsafe { gobject_set_bool(widget, &prop_c, bool_to_c(value)) };
            Ok(())
        })
    }

    pub(super) fn widget_set_size_request(id: i64, w: i32, h: i32) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let widget = widget_ptr(&state, id, "widgetSetSizeRequest")?;
            unsafe { gtk_widget_set_size_request(widget, w, h) };
            Ok(())
        })
    }

    pub(super) fn widget_set_hexpand(id: i64, expand: bool) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let widget = widget_ptr(&state, id, "widgetSetHexpand")?;
            unsafe { gtk_widget_set_hexpand(widget, bool_to_c(expand)) };
            Ok(())
        })
    }

    pub(super) fn widget_set_vexpand(id: i64, expand: bool) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let widget = widget_ptr(&state, id, "widgetSetVexpand")?;
            unsafe { gtk_widget_set_vexpand(widget, bool_to_c(expand)) };
            Ok(())
        })
    }

    pub(super) fn widget_set_halign(id: i64, align: i32) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let widget = widget_ptr(&state, id, "widgetSetHalign")?;
            unsafe { gtk_widget_set_halign(widget, align) };
            Ok(())
        })
    }

    pub(super) fn widget_set_valign(id: i64, align: i32) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let widget = widget_ptr(&state, id, "widgetSetValign")?;
            unsafe { gtk_widget_set_valign(widget, align) };
            Ok(())
        })
    }

    pub(super) fn widget_set_margin_start(id: i64, margin: i32) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let w = widget_ptr(&state, id, "widgetSetMarginStart")?;
            unsafe { gtk_widget_set_margin_start(w, margin) };
            Ok(())
        })
    }

    pub(super) fn widget_set_margin_end(id: i64, margin: i32) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let w = widget_ptr(&state, id, "widgetSetMarginEnd")?;
            unsafe { gtk_widget_set_margin_end(w, margin) };
            Ok(())
        })
    }

    pub(super) fn widget_set_margin_top(id: i64, margin: i32) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let w = widget_ptr(&state, id, "widgetSetMarginTop")?;
            unsafe { gtk_widget_set_margin_top(w, margin) };
            Ok(())
        })
    }

    pub(super) fn widget_set_margin_bottom(id: i64, margin: i32) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let w = widget_ptr(&state, id, "widgetSetMarginBottom")?;
            unsafe { gtk_widget_set_margin_bottom(w, margin) };
            Ok(())
        })
    }

    pub(super) fn widget_add_css_class(id: i64, class: &str) -> Result<(), Gtk4Error> {
        let c = c_text(class, "gtk4.widgetAddCssClass invalid class")?;
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let w = widget_ptr(&state, id, "widgetAddCssClass")?;
            unsafe { gtk_widget_add_css_class(w, c.as_ptr()) };
            Ok(())
        })
    }

    pub(super) fn widget_remove_css_class(id: i64, class: &str) -> Result<(), Gtk4Error> {
        let c = c_text(class, "gtk4.widgetRemoveCssClass invalid class")?;
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let w = widget_ptr(&state, id, "widgetRemoveCssClass")?;
            unsafe { gtk_widget_remove_css_class(w, c.as_ptr()) };
            Ok(())
        })
    }

    pub(super) fn widget_set_tooltip_text(id: i64, text: &str) -> Result<(), Gtk4Error> {
        let c = c_text(text, "gtk4.widgetSetTooltipText invalid text")?;
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let w = widget_ptr(&state, id, "widgetSetTooltipText")?;
            unsafe { gtk_widget_set_tooltip_text(w, c.as_ptr()) };
            Ok(())
        })
    }

    pub(super) fn widget_set_opacity(id: i64, opacity: f64) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let w = widget_ptr(&state, id, "widgetSetOpacity")?;
            unsafe { gtk_widget_set_opacity(w, opacity) };
            Ok(())
        })
    }

    pub(super) fn widget_set_css(id: i64, _css: &str) -> Result<(), Gtk4Error> {
        let _w = GTK_STATE.with(|state| {
            let state = state.borrow();
            widget_ptr(&state, id, "widgetSetCss")
        })?;
        Ok(())
    }

    pub(super) fn widget_by_id(name: &str) -> Result<i64, Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            state.named_widgets.get(name).copied().ok_or_else(|| {
                Gtk4Error::new(format!("gtk4.widgetById unknown named widget '{name}'"))
            })
        })
    }

    pub(super) fn widget_add_controller(
        widget_id: i64,
        controller_id: i64,
    ) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let widget = widget_ptr(&state, widget_id, "widgetAddController")?;
            let gesture = state.gesture_clicks.get(&controller_id).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk4.widgetAddController unknown controller id {controller_id}"
                ))
            })?;
            unsafe { gtk_widget_add_controller(widget, gesture.raw) };
            Ok(())
        })
    }

    pub(super) fn widget_add_shortcut(_widget_id: i64, _shortcut_id: i64) -> Result<(), Gtk4Error> {
        Ok(()) // stub
    }

    pub(super) fn widget_set_layout_manager(_widget_id: i64, _lm_id: i64) -> Result<(), Gtk4Error> {
        Ok(()) // stub
    }

    pub(super) fn box_new(orientation: i64, spacing: i32) -> Result<i64, Gtk4Error> {
        let ori: i32 = if orientation == 1 { 1 } else { 0 };
        let id = GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let raw = unsafe { gtk_box_new(ori, spacing) };
            let id = state.alloc_id();
            state.boxes.insert(id, raw);
            state.widgets.insert(id, raw);
            id
        });
        Ok(id)
    }

    pub(super) fn box_append(box_id: i64, child_id: i64) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let container =
                state.boxes.get(&box_id).copied().ok_or_else(|| {
                    Gtk4Error::new(format!("gtk4.boxAppend unknown box id {box_id}"))
                })?;
            let child = widget_ptr(&state, child_id, "boxAppend")?;
            unsafe { gtk_box_append(container, child) };
            Ok(())
        })
    }

    pub(super) fn box_set_homogeneous(box_id: i64, homogeneous: bool) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let boxw = state.boxes.get(&box_id).copied().ok_or_else(|| {
                Gtk4Error::new(format!("gtk4.boxSetHomogeneous unknown box id {box_id}"))
            })?;
            unsafe { gtk_box_set_homogeneous(boxw, bool_to_c(homogeneous)) };
            Ok(())
        })
    }

    pub(super) fn button_new(label: &str) -> Result<i64, Gtk4Error> {
        let c = c_text(label, "gtk4.buttonNew invalid label")?;
        let id = GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let raw = unsafe { gtk_button_new_with_label(c.as_ptr()) };
            let id = state.alloc_id();
            state.buttons.insert(id, raw);
            state.widgets.insert(id, raw);
            id
        });
        Ok(id)
    }

    pub(super) fn button_set_label(id: i64, label: &str) -> Result<(), Gtk4Error> {
        let c = c_text(label, "gtk4.buttonSetLabel invalid label")?;
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let button = state.buttons.get(&id).copied().ok_or_else(|| {
                Gtk4Error::new(format!("gtk4.buttonSetLabel unknown button id {id}"))
            })?;
            unsafe { gtk_button_set_label(button, c.as_ptr()) };
            Ok(())
        })
    }

    pub(super) fn button_new_from_icon_name(icon: &str) -> Result<i64, Gtk4Error> {
        let c = c_text(icon, "gtk4.buttonNewFromIconName invalid icon name")?;
        let id = GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let raw = unsafe { gtk_button_new_from_icon_name(c.as_ptr()) };
            let id = state.alloc_id();
            state.buttons.insert(id, raw);
            state.widgets.insert(id, raw);
            id
        });
        Ok(id)
    }

    pub(super) fn button_set_child(button_id: i64, child_id: i64) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let button = state.buttons.get(&button_id).copied().ok_or_else(|| {
                Gtk4Error::new(format!("gtk4.buttonSetChild unknown button id {button_id}"))
            })?;
            let child = state.widgets.get(&child_id).copied().ok_or_else(|| {
                Gtk4Error::new(format!("gtk4.buttonSetChild unknown child id {child_id}"))
            })?;
            unsafe { gtk_button_set_child(button, child) };
            Ok(())
        })
    }

    pub(super) fn label_new(text: &str) -> Result<i64, Gtk4Error> {
        let c = c_text(text, "gtk4.labelNew invalid text")?;
        let id = GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            let raw = unsafe { gtk_label_new(c.as_ptr()) };
            let id = s.alloc_id();
            s.labels.insert(id, raw);
            s.widgets.insert(id, raw);
            id
        });
        Ok(id)
    }

    pub(super) fn label_set_text(id: i64, text: &str) -> Result<(), Gtk4Error> {
        let c = c_text(text, "gtk4.labelSetText invalid text")?;
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let l = state.labels.get(&id).copied().ok_or_else(|| {
                Gtk4Error::new(format!("gtk4.labelSetText unknown label id {id}"))
            })?;
            unsafe { gtk_label_set_text(l, c.as_ptr()) };
            Ok(())
        })
    }

    pub(super) fn label_set_wrap(id: i64, wrap: bool) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let l = state
                .labels
                .get(&id)
                .copied()
                .ok_or_else(|| Gtk4Error::new(format!("unknown label id {id}")))?;
            unsafe { gtk_label_set_wrap(l, bool_to_c(wrap)) };
            Ok(())
        })
    }

    pub(super) fn label_set_ellipsize(id: i64, mode: i32) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let l = state
                .labels
                .get(&id)
                .copied()
                .ok_or_else(|| Gtk4Error::new(format!("unknown label id {id}")))?;
            unsafe { gtk_label_set_ellipsize(l, mode) };
            Ok(())
        })
    }

    pub(super) fn label_set_xalign(id: i64, xalign: f32) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let l = state
                .labels
                .get(&id)
                .copied()
                .ok_or_else(|| Gtk4Error::new(format!("unknown label id {id}")))?;
            unsafe { gtk_label_set_xalign(l, xalign) };
            Ok(())
        })
    }

    pub(super) fn label_set_max_width_chars(id: i64, n: i32) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let l = state
                .labels
                .get(&id)
                .copied()
                .ok_or_else(|| Gtk4Error::new(format!("unknown label id {id}")))?;
            unsafe { gtk_label_set_max_width_chars(l, n) };
            Ok(())
        })
    }

    pub(super) fn entry_new() -> Result<i64, Gtk4Error> {
        let id = GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            let raw = unsafe { gtk_entry_new() };
            let id = s.alloc_id();
            s.entries.insert(id, raw);
            s.widgets.insert(id, raw);
            id
        });
        Ok(id)
    }

    pub(super) fn entry_set_text(id: i64, text: &str) -> Result<(), Gtk4Error> {
        let c = c_text(text, "gtk4.entrySetText invalid text")?;
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let e = state
                .entries
                .get(&id)
                .copied()
                .ok_or_else(|| Gtk4Error::new(format!("unknown entry id {id}")))?;
            unsafe { gtk_editable_set_text(e, c.as_ptr()) };
            Ok(())
        })
    }

    pub(super) fn entry_text(id: i64) -> Result<String, Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let entry = state
                .entries
                .get(&id)
                .copied()
                .ok_or_else(|| Gtk4Error::new(format!("unknown entry id {id}")))?;
            let text_ptr = unsafe { gtk_editable_get_text(entry) };
            if text_ptr.is_null() {
                return Ok(String::new());
            }
            Ok(unsafe { CStr::from_ptr(text_ptr) }
                .to_string_lossy()
                .into_owned())
        })
    }

    pub(super) fn image_new_from_file(path: &str) -> Result<i64, Gtk4Error> {
        let c = c_text(path, "gtk4.imageNewFromFile invalid path")?;
        GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            let raw = unsafe { gtk_image_new_from_file(c.as_ptr()) };
            if raw.is_null() {
                return Err(Gtk4Error::new("gtk4.imageNewFromFile failed"));
            }
            let id = s.alloc_id();
            s.images.insert(id, raw);
            s.widgets.insert(id, raw);
            Ok(id)
        })
    }

    pub(super) fn image_set_file(id: i64, path: &str) -> Result<(), Gtk4Error> {
        let c = c_text(path, "gtk4.imageSetFile invalid path")?;
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let img = state
                .images
                .get(&id)
                .copied()
                .ok_or_else(|| Gtk4Error::new(format!("unknown image id {id}")))?;
            unsafe { gtk_image_set_from_file(img, c.as_ptr()) };
            Ok(())
        })
    }

    pub(super) fn image_new_from_resource(path: &str) -> Result<i64, Gtk4Error> {
        let c = c_text(path, "gtk4.imageNewFromResource invalid path")?;
        GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            let raw = unsafe { gtk_image_new_from_resource(c.as_ptr()) };
            if raw.is_null() {
                return Err(Gtk4Error::new("gtk4.imageNewFromResource failed"));
            }
            let id = s.alloc_id();
            s.images.insert(id, raw);
            s.widgets.insert(id, raw);
            Ok(id)
        })
    }

    pub(super) fn image_set_resource(id: i64, path: &str) -> Result<(), Gtk4Error> {
        let c = c_text(path, "gtk4.imageSetResource invalid path")?;
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let img = state
                .images
                .get(&id)
                .copied()
                .ok_or_else(|| Gtk4Error::new(format!("unknown image id {id}")))?;
            unsafe { gtk_image_set_from_resource(img, c.as_ptr()) };
            Ok(())
        })
    }

    pub(super) fn image_new_from_icon_name(icon: &str) -> Result<i64, Gtk4Error> {
        let c = c_text(icon, "gtk4.imageNewFromIconName invalid icon name")?;
        let id = GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            let raw = unsafe { gtk_image_new_from_icon_name(c.as_ptr()) };
            let id = s.alloc_id();
            s.images.insert(id, raw);
            s.widgets.insert(id, raw);
            id
        });
        Ok(id)
    }

    pub(super) fn image_set_pixel_size(id: i64, size: i32) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let img = state
                .images
                .get(&id)
                .copied()
                .ok_or_else(|| Gtk4Error::new(format!("unknown image id {id}")))?;
            unsafe { gtk_image_set_pixel_size(img, size) };
            Ok(())
        })
    }

    pub(super) fn icon_theme_add_search_path(path: &str) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.pending_icon_search_paths.push(path.to_string());
            if !state.windows.is_empty() {
                apply_pending_display_customizations(&mut state)?;
            }
            Ok(())
        })
    }

    pub(super) fn scroll_area_new() -> Result<i64, Gtk4Error> {
        let id = GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            let raw = unsafe { gtk_scrolled_window_new() };
            let id = s.alloc_id();
            s.scrolled_windows.insert(id, raw);
            s.widgets.insert(id, raw);
            id
        });
        Ok(id)
    }

    pub(super) fn scroll_area_set_child(scroll_id: i64, child_id: i64) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let scrolled = state
                .scrolled_windows
                .get(&scroll_id)
                .copied()
                .ok_or_else(|| Gtk4Error::new(format!("unknown scroll id {scroll_id}")))?;
            let child = widget_ptr(&state, child_id, "scrollAreaSetChild")?;
            unsafe { gtk_scrolled_window_set_child(scrolled, child) };
            Ok(())
        })
    }

    pub(super) fn scroll_area_set_policy(scroll_id: i64, h: i32, v: i32) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let scrolled = state
                .scrolled_windows
                .get(&scroll_id)
                .copied()
                .ok_or_else(|| Gtk4Error::new(format!("unknown scroll id {scroll_id}")))?;
            unsafe { gtk_scrolled_window_set_policy(scrolled, h, v) };
            Ok(())
        })
    }

    pub(super) fn separator_new(orientation: i64) -> Result<i64, Gtk4Error> {
        let ori = if orientation == 1 { 1 } else { 0 };
        let id = GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            let raw = unsafe { gtk_separator_new(ori) };
            let id = s.alloc_id();
            s.separators.insert(id, raw);
            s.widgets.insert(id, raw);
            id
        });
        Ok(id)
    }

    pub(super) fn overlay_new() -> Result<i64, Gtk4Error> {
        let id = GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            let raw = unsafe { gtk_overlay_new() };
            let id = s.alloc_id();
            s.overlays.insert(id, raw);
            s.widgets.insert(id, raw);
            id
        });
        Ok(id)
    }

    pub(super) fn overlay_set_child(overlay_id: i64, child_id: i64) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let o = state
                .overlays
                .get(&overlay_id)
                .copied()
                .ok_or_else(|| Gtk4Error::new(format!("unknown overlay id {overlay_id}")))?;
            let c = widget_ptr(&state, child_id, "overlaySetChild")?;
            unsafe { gtk_overlay_set_child(o, c) };
            Ok(())
        })
    }

    pub(super) fn overlay_add_overlay(overlay_id: i64, child_id: i64) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let o = state
                .overlays
                .get(&overlay_id)
                .copied()
                .ok_or_else(|| Gtk4Error::new(format!("unknown overlay id {overlay_id}")))?;
            let c = widget_ptr(&state, child_id, "overlayAddOverlay")?;
            unsafe { gtk_overlay_add_overlay(o, c) };
            Ok(())
        })
    }

    pub(super) fn draw_area_new(w: i32, h: i32) -> Result<i64, Gtk4Error> {
        let id = GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            let raw = unsafe { gtk_drawing_area_new() };
            unsafe { gtk_widget_set_size_request(raw, w, h) };
            let id = s.alloc_id();
            s.draw_areas.insert(id, raw);
            s.widgets.insert(id, raw);
            id
        });
        Ok(id)
    }

    pub(super) fn draw_area_set_content_size(id: i64, w: i32, h: i32) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let d = state
                .draw_areas
                .get(&id)
                .copied()
                .ok_or_else(|| Gtk4Error::new(format!("unknown draw area id {id}")))?;
            unsafe { gtk_widget_set_size_request(d, w, h) };
            Ok(())
        })
    }

    pub(super) fn draw_area_queue_draw(id: i64) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let d = state
                .draw_areas
                .get(&id)
                .copied()
                .ok_or_else(|| Gtk4Error::new(format!("unknown draw area id {id}")))?;
            unsafe { gtk_widget_queue_draw(d) };
            Ok(())
        })
    }

    pub(super) fn gesture_click_new(widget_id: i64) -> Result<i64, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let _w = widget_ptr(&state, widget_id, "gestureClickNew")?;
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
            Ok(id)
        })
    }

    pub(super) fn gesture_click_last_button(id: i64) -> Result<i64, Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let g = state
                .gesture_clicks
                .get(&id)
                .ok_or_else(|| Gtk4Error::new(format!("unknown gesture id {id}")))?;
            Ok(g.last_button)
        })
    }

    pub(super) fn clipboard_default() -> Result<i64, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            Ok(s.alloc_id())
        })
    }

    pub(super) fn clipboard_set_text(_id: i64, _text: &str) -> Result<(), Gtk4Error> {
        Ok(())
    }
    pub(super) fn clipboard_text(_id: i64) -> Result<String, Gtk4Error> {
        Ok(String::new())
    }

    pub(super) fn action_new(_name: &str) -> Result<i64, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            Ok(s.alloc_id())
        })
    }

    pub(super) fn action_set_enabled(_id: i64, _enabled: bool) -> Result<(), Gtk4Error> {
        Ok(())
    }
    pub(super) fn app_add_action(_app_id: i64, _action_id: i64) -> Result<(), Gtk4Error> {
        Ok(())
    }

    pub(super) fn shortcut_new(_accel: &str, _action: &str) -> Result<i64, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            Ok(s.alloc_id())
        })
    }

    pub(super) fn notification_new(_title: &str, _body: &str) -> Result<i64, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            Ok(s.alloc_id())
        })
    }

    pub(super) fn notification_set_body(_id: i64, _body: &str) -> Result<(), Gtk4Error> {
        Ok(())
    }
    pub(super) fn app_send_notification(
        _app_id: i64,
        _tag: &str,
        _notif_id: i64,
    ) -> Result<(), Gtk4Error> {
        Ok(())
    }
    pub(super) fn app_withdraw_notification(_app_id: i64, _tag: &str) -> Result<(), Gtk4Error> {
        Ok(())
    }

    pub(super) fn layout_manager_new(_name: &str) -> Result<i64, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            Ok(s.alloc_id())
        })
    }

    pub(super) fn drag_source_new(_widget_id: i64) -> Result<i64, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            Ok(s.alloc_id())
        })
    }
    pub(super) fn drag_source_set_text(_id: i64, _text: &str) -> Result<(), Gtk4Error> {
        Ok(())
    }
    pub(super) fn drop_target_new(_widget_id: i64) -> Result<i64, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            Ok(s.alloc_id())
        })
    }
    pub(super) fn drop_target_last_text(_id: i64) -> Result<String, Gtk4Error> {
        Ok(String::new())
    }

    pub(super) fn menu_model_new() -> Result<i64, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            Ok(s.alloc_id())
        })
    }
    pub(super) fn menu_model_append_item(
        _id: i64,
        _label: &str,
        _action: &str,
    ) -> Result<(), Gtk4Error> {
        Ok(())
    }
    pub(super) fn menu_button_new(_label: &str) -> Result<i64, Gtk4Error> {
        let c = c_text("", "menu_button_new")?;
        let id = GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            let raw = unsafe { gtk_menu_button_new() };
            let id = s.alloc_id();
            s.widgets.insert(id, raw);
            let _ = c;
            id
        });
        Ok(id)
    }
    pub(super) fn menu_button_set_menu_model(
        _button_id: i64,
        _model_id: i64,
    ) -> Result<(), Gtk4Error> {
        Ok(())
    }

    pub(super) fn dialog_new() -> Result<i64, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let window = unsafe { gtk_window_new() };
            if window.is_null() {
                return Err(Gtk4Error::new("gtk4.dialogNew failed"));
            }
            unsafe { gtk_window_set_modal(window, 1) };
            let id = state.alloc_id();
            state.windows.insert(id, window);
            state.widgets.insert(id, window);
            Ok(id)
        })
    }

    pub(super) fn dialog_set_title(id: i64, title: &str) -> Result<(), Gtk4Error> {
        let c = c_text(title, "gtk4.dialogSetTitle invalid title")?;
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let d = widget_ptr(&state, id, "dialogSetTitle")?;
            unsafe { gtk_window_set_title(d, c.as_ptr()) };
            Ok(())
        })
    }

    pub(super) fn dialog_set_child(id: i64, child_id: i64) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let d = widget_ptr(&state, id, "dialogSetChild")?;
            let c = widget_ptr(&state, child_id, "dialogSetChild")?;
            unsafe { gtk_window_set_child(d, c) };
            Ok(())
        })
    }

    pub(super) fn dialog_present(dialog_id: i64, parent_id: i64) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let dialog = widget_ptr(&state, dialog_id, "dialogPresent")?;
            let parent = widget_ptr(&state, parent_id, "dialogPresent")?;
            unsafe {
                gtk_window_set_transient_for(dialog, parent);
                gtk_window_present(dialog);
            }
            Ok(())
        })
    }

    pub(super) fn dialog_close(id: i64) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let d = widget_ptr(&state, id, "dialogClose")?;
            unsafe { gtk_window_close(d) };
            Ok(())
        })
    }

    pub(super) fn adw_dialog_present(dialog_id: i64, parent_id: i64) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let dialog = widget_ptr(&state, dialog_id, "adwDialogPresent")?;
            let parent = widget_ptr(&state, parent_id, "adwDialogPresent")?;
            call_adw_fn_pp("adw_dialog_present", dialog, parent);
            Ok(())
        })
    }

    pub(super) fn file_dialog_new() -> Result<i64, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            Ok(s.alloc_id())
        })
    }
    pub(super) fn file_dialog_select_file(_id: i64) -> Result<String, Gtk4Error> {
        Ok(String::new())
    }

    pub(super) fn list_store_new() -> Result<i64, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            Ok(s.alloc_id())
        })
    }
    pub(super) fn list_store_append_text(_id: i64, _text: &str) -> Result<(), Gtk4Error> {
        Ok(())
    }
    pub(super) fn list_store_items(_id: i64) -> Result<Vec<String>, Gtk4Error> {
        Ok(Vec::new())
    }
    pub(super) fn list_view_new() -> Result<i64, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            Ok(s.alloc_id())
        })
    }
    pub(super) fn list_view_set_model(_view_id: i64, _store_id: i64) -> Result<(), Gtk4Error> {
        Ok(())
    }
    pub(super) fn tree_view_new() -> Result<i64, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut s = state.borrow_mut();
            Ok(s.alloc_id())
        })
    }
    pub(super) fn tree_view_set_model(_view_id: i64, _store_id: i64) -> Result<(), Gtk4Error> {
        Ok(())
    }

    pub(super) fn os_open_uri(_app_id: i64, _uri: &str) -> Result<(), Gtk4Error> {
        Ok(())
    }
    pub(super) fn os_show_in_file_manager(_path: &str) -> Result<(), Gtk4Error> {
        Ok(())
    }

    pub(super) fn os_set_badge_count(_app_id: i64, count: i64) -> Result<(), Gtk4Error> {
        if let Ok(mut q) = pending_badge_updates().lock() {
            q.push_back(count);
        }
        Ok(())
    }

    pub(super) fn os_theme_preference() -> Result<String, Gtk4Error> {
        Ok("default".to_string())
    }

    pub(super) fn signal_poll() -> Result<Option<SignalEvent>, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            if let Some(event) = state.signal_events.pop_front() {
                let widget_name = state
                    .widget_id_to_name
                    .get(&event.widget_id)
                    .cloned()
                    .unwrap_or_default();
                Ok(Some(make_signal_event(event, widget_name)))
            } else {
                Ok(None)
            }
        })
    }

    pub(super) fn signal_stream() -> Result<std::sync::mpsc::Receiver<SignalEvent>, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let (sender, receiver) = mpsc::channel();
            state.signal_senders.push(sender);
            Ok(receiver)
        })
    }

    pub(super) fn signal_emit(
        widget_id: i64,
        signal: &str,
        handler: &str,
        payload: &str,
    ) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            if widget_id != 0 {
                let _ = widget_ptr(&state, widget_id, "signalEmit")?;
            }
            state.signal_events.push_back(SignalEventState {
                widget_id,
                signal: signal.to_string(),
                handler: handler.to_string(),
                payload: payload.to_string(),
            });
            Ok(())
        })
    }

    fn push_signal_action(
        handler: &str,
        action: SignalAction,
        validate_ids: &[i64],
        fn_name: &str,
    ) -> Result<(), Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            for &id in validate_ids {
                let _ = widget_ptr(&state, id, fn_name)?;
            }
            state
                .signal_action_bindings
                .entry(handler.to_string())
                .or_default()
                .push(action);
            Ok(())
        })
    }

    pub(super) fn signal_bind_bool_property(
        handler: &str,
        widget_id: i64,
        prop: &str,
        value: bool,
    ) -> Result<(), Gtk4Error> {
        push_signal_action(
            handler,
            SignalAction::SetBool {
                widget_id,
                property: prop.to_string(),
                value,
            },
            &[widget_id],
            "signalBindBoolProperty",
        )
    }

    pub(super) fn signal_bind_css_class(
        handler: &str,
        widget_id: i64,
        class: &str,
        add: bool,
    ) -> Result<(), Gtk4Error> {
        push_signal_action(
            handler,
            SignalAction::CssClass {
                widget_id,
                class_name: class.to_string(),
                add,
            },
            &[widget_id],
            "signalBindCssClass",
        )
    }

    pub(super) fn signal_bind_toggle_bool_property(
        handler: &str,
        widget_id: i64,
        prop: &str,
    ) -> Result<(), Gtk4Error> {
        push_signal_action(
            handler,
            SignalAction::ToggleBool {
                widget_id,
                property: prop.to_string(),
            },
            &[widget_id],
            "signalBindToggleBoolProperty",
        )
    }

    pub(super) fn signal_toggle_css_class(
        handler: &str,
        widget_id: i64,
        class: &str,
    ) -> Result<(), Gtk4Error> {
        push_signal_action(
            handler,
            SignalAction::ToggleCssClass {
                widget_id,
                class_name: class.to_string(),
            },
            &[widget_id],
            "signalToggleCssClass",
        )
    }

    pub(super) fn signal_bind_dialog_present(
        handler: &str,
        dialog_id: i64,
        parent_id: i64,
    ) -> Result<(), Gtk4Error> {
        push_signal_action(
            handler,
            SignalAction::PresentDialog {
                dialog_id,
                parent_id,
            },
            &[dialog_id, parent_id],
            "signalBindDialogPresent",
        )
    }

    pub(super) fn signal_bind_stack_page(
        handler: &str,
        stack_id: i64,
        page_name: &str,
    ) -> Result<(), Gtk4Error> {
        push_signal_action(
            handler,
            SignalAction::SetStackPage {
                stack_id,
                page_name: page_name.to_string(),
            },
            &[stack_id],
            "signalBindStackPage",
        )
    }

    pub(super) fn build_from_node(node: &super::GtkNode) -> Result<i64, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let mut id_map = HashMap::new();
            let root = first_object_in_interface(node)?;
            let (id, live) = build_widget_from_node_real(&mut state, root, &mut id_map)?;
            state.named_widgets.extend(id_map.clone());
            for (name, wid) in &id_map {
                state.widget_id_to_name.insert(*wid, name.clone());
            }
            state.live_trees.insert(id, live);
            Ok(id)
        })
    }

    pub(super) fn build_with_ids(node: &super::GtkNode) -> Result<BuildResult, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let mut id_map = HashMap::new();
            let root = first_object_in_interface(node)?;
            let (id, live) = build_widget_from_node_real(&mut state, root, &mut id_map)?;
            state.named_widgets.extend(id_map.clone());
            for (name, wid) in &id_map {
                state.widget_id_to_name.insert(*wid, name.clone());
            }
            state.live_trees.insert(id, live);
            Ok(BuildResult {
                root_id: id,
                named_widgets: id_map,
            })
        })
    }

    pub(super) fn reconcile_node_fn(root_id: i64, node: &super::GtkNode) -> Result<i64, Gtk4Error> {
        let new_root = first_object_in_interface(node)?;
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let mut id_map: HashMap<String, i64> = HashMap::new();
            let mut live = state.live_trees.remove(&root_id).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk4.reconcileNode no live tree for root id {root_id}"
                ))
            })?;
            let patched = reconcile_node(&mut state, &mut live, new_root, &mut id_map)?;
            let final_id = if !patched {
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
        })
    }
} // mod linux_impl

// ── Public API ────────────────────────────────────────────────────────────────

#[cfg(not(target_os = "linux"))]
fn not_available() -> Gtk4Error {
    Gtk4Error::new("GTK4 runtime is not available on this platform")
}

macro_rules! delegate {
    ($fn_name:ident () -> $ret:ty) => {
        pub fn $fn_name() -> $ret {
            #[cfg(target_os = "linux")]
            { linux_impl::$fn_name() }
            #[cfg(not(target_os = "linux"))]
            { Err(not_available()) }
        }
    };
    ($fn_name:ident ($($arg:ident : $ty:ty),+) -> $ret:ty) => {
        pub fn $fn_name($($arg: $ty),+) -> $ret {
            #[cfg(target_os = "linux")]
            { linux_impl::$fn_name($($arg),+) }
            #[cfg(not(target_os = "linux"))]
            { $(let _ = $arg;)+ Err(not_available()) }
        }
    };
}

delegate!(init() -> Result<(), Gtk4Error>);
delegate!(app_new(id: &str) -> Result<i64, Gtk4Error>);
delegate!(app_run(app_id: i64) -> Result<(), Gtk4Error>);
delegate!(app_set_css(app_id: i64, css: &str) -> Result<(), Gtk4Error>);
delegate!(window_new(app_id: i64, title: &str, width: i32, height: i32) -> Result<i64, Gtk4Error>);
delegate!(window_set_title(win_id: i64, title: &str) -> Result<(), Gtk4Error>);
delegate!(window_set_titlebar(win_id: i64, titlebar_id: i64) -> Result<(), Gtk4Error>);
delegate!(window_set_child(win_id: i64, child_id: i64) -> Result<(), Gtk4Error>);
delegate!(window_present(win_id: i64) -> Result<(), Gtk4Error>);
delegate!(window_close(win_id: i64) -> Result<(), Gtk4Error>);
delegate!(window_on_close(win_id: i64, signal_name: &str) -> Result<(), Gtk4Error>);
delegate!(window_set_hide_on_close(win_id: i64, hide: bool) -> Result<(), Gtk4Error>);
delegate!(window_set_decorated(win_id: i64, decorated: bool) -> Result<(), Gtk4Error>);
delegate!(widget_show(id: i64) -> Result<(), Gtk4Error>);
delegate!(widget_hide(id: i64) -> Result<(), Gtk4Error>);
delegate!(widget_set_bool_property(id: i64, prop: &str, value: bool) -> Result<(), Gtk4Error>);
delegate!(widget_set_size_request(id: i64, w: i32, h: i32) -> Result<(), Gtk4Error>);
delegate!(widget_set_hexpand(id: i64, expand: bool) -> Result<(), Gtk4Error>);
delegate!(widget_set_vexpand(id: i64, expand: bool) -> Result<(), Gtk4Error>);
delegate!(widget_set_halign(id: i64, align: i32) -> Result<(), Gtk4Error>);
delegate!(widget_set_valign(id: i64, align: i32) -> Result<(), Gtk4Error>);
delegate!(widget_set_margin_start(id: i64, margin: i32) -> Result<(), Gtk4Error>);
delegate!(widget_set_margin_end(id: i64, margin: i32) -> Result<(), Gtk4Error>);
delegate!(widget_set_margin_top(id: i64, margin: i32) -> Result<(), Gtk4Error>);
delegate!(widget_set_margin_bottom(id: i64, margin: i32) -> Result<(), Gtk4Error>);
delegate!(widget_add_css_class(id: i64, class: &str) -> Result<(), Gtk4Error>);
delegate!(widget_remove_css_class(id: i64, class: &str) -> Result<(), Gtk4Error>);
delegate!(widget_set_tooltip_text(id: i64, text: &str) -> Result<(), Gtk4Error>);
delegate!(widget_set_opacity(id: i64, opacity: f64) -> Result<(), Gtk4Error>);
delegate!(widget_set_css(id: i64, css: &str) -> Result<(), Gtk4Error>);
delegate!(widget_by_id(name: &str) -> Result<i64, Gtk4Error>);
delegate!(widget_add_controller(widget_id: i64, controller_id: i64) -> Result<(), Gtk4Error>);
delegate!(widget_add_shortcut(widget_id: i64, shortcut_id: i64) -> Result<(), Gtk4Error>);
delegate!(widget_set_layout_manager(widget_id: i64, lm_id: i64) -> Result<(), Gtk4Error>);
delegate!(box_new(orientation: i64, spacing: i32) -> Result<i64, Gtk4Error>);
delegate!(box_append(box_id: i64, child_id: i64) -> Result<(), Gtk4Error>);
delegate!(box_set_homogeneous(box_id: i64, homogeneous: bool) -> Result<(), Gtk4Error>);
delegate!(button_new(label: &str) -> Result<i64, Gtk4Error>);
delegate!(button_set_label(id: i64, label: &str) -> Result<(), Gtk4Error>);
delegate!(button_new_from_icon_name(icon: &str) -> Result<i64, Gtk4Error>);
delegate!(button_set_child(button_id: i64, child_id: i64) -> Result<(), Gtk4Error>);
delegate!(label_new(text: &str) -> Result<i64, Gtk4Error>);
delegate!(label_set_text(id: i64, text: &str) -> Result<(), Gtk4Error>);
delegate!(label_set_wrap(id: i64, wrap: bool) -> Result<(), Gtk4Error>);
delegate!(label_set_ellipsize(id: i64, mode: i32) -> Result<(), Gtk4Error>);
delegate!(label_set_xalign(id: i64, xalign: f32) -> Result<(), Gtk4Error>);
delegate!(label_set_max_width_chars(id: i64, n: i32) -> Result<(), Gtk4Error>);
delegate!(entry_new() -> Result<i64, Gtk4Error>);
delegate!(entry_set_text(id: i64, text: &str) -> Result<(), Gtk4Error>);
delegate!(entry_text(id: i64) -> Result<String, Gtk4Error>);
delegate!(scroll_area_new() -> Result<i64, Gtk4Error>);
delegate!(scroll_area_set_child(scroll_id: i64, child_id: i64) -> Result<(), Gtk4Error>);
delegate!(scroll_area_set_policy(scroll_id: i64, h: i32, v: i32) -> Result<(), Gtk4Error>);
delegate!(separator_new(orientation: i64) -> Result<i64, Gtk4Error>);
delegate!(overlay_new() -> Result<i64, Gtk4Error>);
delegate!(overlay_set_child(overlay_id: i64, child_id: i64) -> Result<(), Gtk4Error>);
delegate!(overlay_add_overlay(overlay_id: i64, child_id: i64) -> Result<(), Gtk4Error>);
delegate!(draw_area_new(w: i32, h: i32) -> Result<i64, Gtk4Error>);
delegate!(draw_area_set_content_size(id: i64, w: i32, h: i32) -> Result<(), Gtk4Error>);
delegate!(draw_area_queue_draw(id: i64) -> Result<(), Gtk4Error>);
delegate!(image_new_from_file(path: &str) -> Result<i64, Gtk4Error>);
delegate!(image_set_file(id: i64, path: &str) -> Result<(), Gtk4Error>);
delegate!(image_new_from_resource(path: &str) -> Result<i64, Gtk4Error>);
delegate!(image_set_resource(id: i64, path: &str) -> Result<(), Gtk4Error>);
delegate!(image_new_from_icon_name(icon: &str) -> Result<i64, Gtk4Error>);
delegate!(image_set_pixel_size(id: i64, size: i32) -> Result<(), Gtk4Error>);
delegate!(icon_theme_add_search_path(path: &str) -> Result<(), Gtk4Error>);
delegate!(gesture_click_new(widget_id: i64) -> Result<i64, Gtk4Error>);
delegate!(gesture_click_last_button(id: i64) -> Result<i64, Gtk4Error>);
delegate!(clipboard_default() -> Result<i64, Gtk4Error>);
delegate!(clipboard_set_text(id: i64, text: &str) -> Result<(), Gtk4Error>);
delegate!(clipboard_text(id: i64) -> Result<String, Gtk4Error>);
delegate!(action_new(name: &str) -> Result<i64, Gtk4Error>);
delegate!(action_set_enabled(id: i64, enabled: bool) -> Result<(), Gtk4Error>);
delegate!(app_add_action(app_id: i64, action_id: i64) -> Result<(), Gtk4Error>);
delegate!(shortcut_new(accel: &str, action: &str) -> Result<i64, Gtk4Error>);
delegate!(notification_new(title: &str, body: &str) -> Result<i64, Gtk4Error>);
delegate!(notification_set_body(id: i64, body: &str) -> Result<(), Gtk4Error>);
delegate!(app_send_notification(app_id: i64, tag: &str, notif_id: i64) -> Result<(), Gtk4Error>);
delegate!(app_withdraw_notification(app_id: i64, tag: &str) -> Result<(), Gtk4Error>);
delegate!(layout_manager_new(name: &str) -> Result<i64, Gtk4Error>);
delegate!(dbus_server_start() -> Result<(), Gtk4Error>);
delegate!(drag_source_new(widget_id: i64) -> Result<i64, Gtk4Error>);
delegate!(drag_source_set_text(id: i64, text: &str) -> Result<(), Gtk4Error>);
delegate!(drop_target_new(widget_id: i64) -> Result<i64, Gtk4Error>);
delegate!(drop_target_last_text(id: i64) -> Result<String, Gtk4Error>);
delegate!(menu_model_new() -> Result<i64, Gtk4Error>);
delegate!(menu_model_append_item(id: i64, label: &str, action: &str) -> Result<(), Gtk4Error>);
delegate!(menu_button_new(label: &str) -> Result<i64, Gtk4Error>);
delegate!(menu_button_set_menu_model(button_id: i64, model_id: i64) -> Result<(), Gtk4Error>);
delegate!(dialog_new() -> Result<i64, Gtk4Error>);
delegate!(dialog_set_title(id: i64, title: &str) -> Result<(), Gtk4Error>);
delegate!(dialog_set_child(id: i64, child_id: i64) -> Result<(), Gtk4Error>);
delegate!(dialog_present(dialog_id: i64, parent_id: i64) -> Result<(), Gtk4Error>);
delegate!(dialog_close(id: i64) -> Result<(), Gtk4Error>);
delegate!(adw_dialog_present(dialog_id: i64, parent_id: i64) -> Result<(), Gtk4Error>);
delegate!(file_dialog_new() -> Result<i64, Gtk4Error>);
delegate!(file_dialog_select_file(id: i64) -> Result<String, Gtk4Error>);
delegate!(list_store_new() -> Result<i64, Gtk4Error>);
delegate!(list_store_append_text(id: i64, text: &str) -> Result<(), Gtk4Error>);
delegate!(list_store_items(id: i64) -> Result<Vec<String>, Gtk4Error>);
delegate!(list_view_new() -> Result<i64, Gtk4Error>);
delegate!(list_view_set_model(view_id: i64, store_id: i64) -> Result<(), Gtk4Error>);
delegate!(tree_view_new() -> Result<i64, Gtk4Error>);
delegate!(tree_view_set_model(view_id: i64, store_id: i64) -> Result<(), Gtk4Error>);
delegate!(os_open_uri(app_id: i64, uri: &str) -> Result<(), Gtk4Error>);
delegate!(os_show_in_file_manager(path: &str) -> Result<(), Gtk4Error>);
delegate!(os_set_badge_count(app_id: i64, count: i64) -> Result<(), Gtk4Error>);
delegate!(os_theme_preference() -> Result<String, Gtk4Error>);
delegate!(signal_poll() -> Result<Option<SignalEvent>, Gtk4Error>);
delegate!(signal_stream() -> Result<std::sync::mpsc::Receiver<SignalEvent>, Gtk4Error>);
delegate!(signal_emit(widget_id: i64, signal: &str, handler: &str, payload: &str) -> Result<(), Gtk4Error>);
delegate!(signal_bind_bool_property(handler: &str, widget_id: i64, prop: &str, value: bool) -> Result<(), Gtk4Error>);
delegate!(signal_bind_css_class(handler: &str, widget_id: i64, class: &str, add: bool) -> Result<(), Gtk4Error>);
delegate!(signal_bind_toggle_bool_property(handler: &str, widget_id: i64, prop: &str) -> Result<(), Gtk4Error>);
delegate!(signal_toggle_css_class(handler: &str, widget_id: i64, class: &str) -> Result<(), Gtk4Error>);
delegate!(signal_bind_dialog_present(handler: &str, dialog_id: i64, parent_id: i64) -> Result<(), Gtk4Error>);
delegate!(signal_bind_stack_page(handler: &str, stack_id: i64, page_name: &str) -> Result<(), Gtk4Error>);
delegate!(build_from_node(node: &GtkNode) -> Result<i64, Gtk4Error>);
delegate!(build_with_ids(node: &GtkNode) -> Result<BuildResult, Gtk4Error>);

pub fn reconcile_node(root_id: i64, node: &GtkNode) -> Result<i64, Gtk4Error> {
    #[cfg(target_os = "linux")]
    {
        linux_impl::reconcile_node_fn(root_id, node)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (root_id, node);
        Err(not_available())
    }
}

pub fn pump_events() {
    #[cfg(target_os = "linux")]
    linux_impl::pump_gtk_events();
}

pub fn is_pump_active() -> bool {
    #[cfg(target_os = "linux")]
    {
        linux_impl::is_pump_active()
    }
    #[cfg(not(target_os = "linux"))]
    false
}
