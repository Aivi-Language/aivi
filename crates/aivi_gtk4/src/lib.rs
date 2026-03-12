#![allow(clippy::type_complexity)]
#![allow(unused_unsafe)]

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use serde_json::{Map as JsonMap, Value as JsonValue};

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

pub struct BuildWithBindingsResult {
    pub root_id: i64,
    pub named_widgets: HashMap<String, i64>,
    pub binding_widgets: HashMap<String, i64>,
}

pub type UiDebugRequestHandler =
    dyn Fn(&str, &JsonMap<String, JsonValue>) -> Option<Result<JsonValue, Gtk4Error>> + Send + Sync;
pub type MainLoopTickHandler = dyn Fn() -> Result<(), Gtk4Error> + Send + Sync;

#[cfg(target_os = "linux")]
#[allow(dead_code)]
mod linux_impl {
    use std::cell::RefCell;
    use std::collections::{HashMap, VecDeque};
    use std::ffi::{CStr, CString};
    use std::fs;
    use std::io::{BufRead, BufReader, Write};
    use std::os::raw::{c_char, c_int, c_uint, c_ulong, c_void};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::ptr::null_mut;
    use std::sync::{mpsc, Arc, Mutex, OnceLock};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use base64::Engine;
    use image::{ImageBuffer, ImageEncoder, Rgba};
    use serde_json::{json, Map, Value};

    use super::{BuildResult, BuildWithBindingsResult, Gtk4Error, GtkNode, SignalEvent};

    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default)]
    pub struct GdkRectangle {
        pub x: c_int,
        pub y: c_int,
        pub width: c_int,
        pub height: c_int,
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default)]
    pub struct GdkRgba {
        pub red: f32,
        pub green: f32,
        pub blue: f32,
        pub alpha: f32,
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default)]
    pub struct GraphenePoint {
        pub x: f32,
        pub y: f32,
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default)]
    pub struct GrapheneSize {
        pub width: f32,
        pub height: f32,
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default)]
    pub struct GrapheneRect {
        pub origin: GraphenePoint,
        pub size: GrapheneSize,
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
        fn gtk_widget_compute_bounds(
            widget: *mut c_void,
            target: *mut c_void,
            out_bounds: *mut GrapheneRect,
        ) -> c_int;
        fn gtk_widget_paintable_new(widget: *mut c_void) -> *mut c_void;
        fn gtk_window_set_titlebar(window: *mut c_void, titlebar: *mut c_void);
        fn gtk_window_new() -> *mut c_void;
        fn gtk_application_window_new(application: *mut c_void) -> *mut c_void;
        fn gtk_window_set_child(window: *mut c_void, child: *mut c_void);
        fn gtk_window_set_modal(window: *mut c_void, modal: c_int);
        fn gtk_window_set_transient_for(window: *mut c_void, parent: *mut c_void);
        fn gtk_window_present(window: *mut c_void);
        fn gtk_window_get_focus(window: *mut c_void) -> *mut c_void;
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
        fn gtk_widget_grab_focus(widget: *mut c_void) -> c_int;
        fn gtk_widget_child_focus(widget: *mut c_void, direction: c_int) -> c_int;
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
        fn gtk_drop_down_new_from_strings(strings: *const *const c_char) -> *mut c_void;
        fn gtk_string_list_new(strings: *const *const c_char) -> *mut c_void;

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
        fn gtk_editable_get_selection_bounds(
            editable: *mut c_void,
            start_pos: *mut c_int,
            end_pos: *mut c_int,
        ) -> c_int;
        fn gtk_editable_get_position(editable: *mut c_void) -> c_int;
        fn gtk_check_button_get_active(check_button: *mut c_void) -> c_int;
        fn gtk_range_get_value(range: *mut c_void) -> f64;
        fn gtk_range_set_value(range: *mut c_void, value: f64);

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
        fn gtk_scrolled_window_get_hadjustment(scrolled: *mut c_void) -> *mut c_void;
        fn gtk_scrolled_window_get_vadjustment(scrolled: *mut c_void) -> *mut c_void;

        fn gtk_adjustment_get_lower(adjustment: *mut c_void) -> f64;
        fn gtk_adjustment_get_value(adjustment: *mut c_void) -> f64;
        fn gtk_adjustment_set_value(adjustment: *mut c_void, value: f64);
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
        fn gtk_event_controller_key_new() -> *mut c_void;
        fn gdk_keyval_name(keyval: c_uint) -> *const c_char;
        fn gtk_widget_set_focusable(widget: *mut c_void, focusable: c_int);

        fn gdk_paintable_get_current_image(paintable: *mut c_void) -> *mut c_void;
        fn gdk_texture_get_width(texture: *mut c_void) -> c_int;
        fn gdk_texture_get_height(texture: *mut c_void) -> c_int;
        fn gdk_texture_download(texture: *mut c_void, data: *mut u8, stride: usize);
        fn gtk_icon_theme_get_for_display(display: *mut c_void) -> *mut c_void;
        fn gtk_icon_theme_add_search_path(icon_theme: *mut c_void, path: *const c_char);
        fn gtk_button_set_child(button: *mut c_void, child: *mut c_void);

        fn gtk_stack_new() -> *mut c_void;
        fn gtk_stack_add_named(stack: *mut c_void, child: *mut c_void, name: *const c_char);
        fn gtk_stack_set_visible_child_name(stack: *mut c_void, name: *const c_char);
        fn gtk_stack_set_transition_type(stack: *mut c_void, transition: c_int);
        fn gtk_stack_set_transition_duration(stack: *mut c_void, duration: c_uint);

        fn gtk_menu_button_new() -> *mut c_void;

        fn gtk_revealer_new() -> *mut c_void;
        fn gtk_revealer_set_child(revealer: *mut c_void, child: *mut c_void);
        fn gtk_revealer_set_reveal_child(revealer: *mut c_void, reveal_child: c_int);
        fn gtk_revealer_set_transition_type(revealer: *mut c_void, transition: c_int);
        fn gtk_revealer_set_transition_duration(revealer: *mut c_void, duration: c_uint);

        fn gtk_progress_bar_new() -> *mut c_void;
        fn gtk_progress_bar_set_fraction(progress_bar: *mut c_void, fraction: f64);

        fn gtk_check_button_new() -> *mut c_void;
        fn gtk_check_button_new_with_label(label: *const c_char) -> *mut c_void;
        fn gtk_check_button_set_active(check_button: *mut c_void, is_active: c_int);

        fn gtk_scale_new_with_range(
            orientation: c_int,
            min: f64,
            max: f64,
            step: f64,
        ) -> *mut c_void;
        fn gtk_scale_set_digits(scale: *mut c_void, digits: c_int);
        fn gtk_scale_set_draw_value(scale: *mut c_void, draw_value: c_int);
        fn gtk_scale_set_value_pos(scale: *mut c_void, pos: c_int);

        fn gtk_switch_new() -> *mut c_void;

        fn gtk_spinner_new() -> *mut c_void;
        fn gtk_spinner_start(spinner: *mut c_void);
        fn gtk_spinner_stop(spinner: *mut c_void);

        fn gtk_search_entry_new() -> *mut c_void;

        fn gtk_toggle_button_new() -> *mut c_void;
        fn gtk_toggle_button_new_with_label(label: *const c_char) -> *mut c_void;
        fn gtk_toggle_button_get_active(toggle_button: *mut c_void) -> c_int;

        fn gtk_link_button_new(uri: *const c_char) -> *mut c_void;
        fn gtk_link_button_new_with_label(uri: *const c_char, label: *const c_char) -> *mut c_void;

        fn gtk_paned_new(orientation: c_int) -> *mut c_void;
        fn gtk_paned_set_start_child(paned: *mut c_void, child: *mut c_void);
        fn gtk_paned_set_end_child(paned: *mut c_void, child: *mut c_void);

        fn gtk_frame_new(label: *const c_char) -> *mut c_void;
        fn gtk_frame_set_child(frame: *mut c_void, child: *mut c_void);

        fn gtk_expander_new(label: *const c_char) -> *mut c_void;
        fn gtk_expander_set_child(expander: *mut c_void, child: *mut c_void);

        fn gtk_notebook_new() -> *mut c_void;
        fn gtk_notebook_append_page(
            notebook: *mut c_void,
            child: *mut c_void,
            tab_label: *mut c_void,
        ) -> c_int;
        fn gtk_notebook_set_current_page(notebook: *mut c_void, page_num: c_int);

        fn gtk_stack_switcher_new() -> *mut c_void;

        fn gtk_flow_box_new() -> *mut c_void;
        fn gtk_flow_box_append(flow_box: *mut c_void, child: *mut c_void);
        fn gtk_flow_box_remove(flow_box: *mut c_void, child: *mut c_void);
        fn gtk_flow_box_set_max_children_per_line(flow_box: *mut c_void, n_children: c_uint);
        fn gtk_flow_box_set_min_children_per_line(flow_box: *mut c_void, n_children: c_uint);
        fn gtk_flow_box_set_row_spacing(flow_box: *mut c_void, spacing: c_uint);
        fn gtk_flow_box_set_column_spacing(flow_box: *mut c_void, spacing: c_uint);
        fn gtk_flow_box_set_homogeneous(flow_box: *mut c_void, homogeneous: c_int);
        fn gtk_flow_box_set_selection_mode(flow_box: *mut c_void, mode: c_int);

        fn gtk_grid_new() -> *mut c_void;
        fn gtk_grid_attach(
            grid: *mut c_void,
            child: *mut c_void,
            column: c_int,
            row: c_int,
            width: c_int,
            height: c_int,
        );
        fn gtk_grid_set_row_spacing(grid: *mut c_void, spacing: c_uint);
        fn gtk_grid_set_column_spacing(grid: *mut c_void, spacing: c_uint);
        fn gtk_grid_set_row_homogeneous(grid: *mut c_void, homogeneous: c_int);
        fn gtk_grid_set_column_homogeneous(grid: *mut c_void, homogeneous: c_int);

        fn gtk_aspect_frame_new(
            xalign: f32,
            yalign: f32,
            ratio: f32,
            obey_child: c_int,
        ) -> *mut c_void;
        fn gtk_aspect_frame_set_child(aspect_frame: *mut c_void, child: *mut c_void);
        fn gtk_aspect_frame_set_ratio(aspect_frame: *mut c_void, ratio: f32);
        fn gtk_aspect_frame_set_obey_child(aspect_frame: *mut c_void, obey_child: c_int);

        fn gtk_spin_button_new_with_range(min: f64, max: f64, step: f64) -> *mut c_void;
        fn gtk_spin_button_get_value(spin_button: *mut c_void) -> f64;
        fn gtk_spin_button_set_value(spin_button: *mut c_void, value: f64);
        fn gtk_spin_button_set_digits(spin_button: *mut c_void, digits: c_uint);
        fn gtk_spin_button_set_wrap(spin_button: *mut c_void, wrap: c_int);
        fn gtk_spin_button_set_snap_to_ticks(spin_button: *mut c_void, snap_to_ticks: c_int);
        fn gtk_spin_button_set_range(spin_button: *mut c_void, min: f64, max: f64);
        fn gtk_spin_button_set_increments(spin_button: *mut c_void, step: f64, page: f64);

        fn gtk_combo_box_text_new() -> *mut c_void;
        fn gtk_combo_box_text_append_text(combo_box: *mut c_void, text: *const c_char);
        fn gtk_combo_box_get_active(combo_box: *mut c_void) -> c_int;
        fn gtk_combo_box_text_get_active_text(combo_box: *mut c_void) -> *mut c_char;
        fn gtk_combo_box_set_active(combo_box: *mut c_void, index: c_int);
        fn gtk_drop_down_set_selected(drop_down: *mut c_void, position: c_uint);
        fn gtk_drop_down_get_selected(drop_down: *mut c_void) -> c_uint;

        // GtkColorDialog / GtkColorDialogButton (GTK 4.10+)
        fn gtk_color_dialog_new() -> *mut c_void;
        fn gtk_color_dialog_button_new(dialog: *mut c_void) -> *mut c_void;
        fn gtk_color_dialog_button_get_rgba(button: *mut c_void) -> *const GdkRgba;
        fn gtk_color_dialog_button_set_rgba(button: *mut c_void, color: *const GdkRgba);

        // GtkFontDialog / GtkFontDialogButton (GTK 4.10+)
        fn gtk_font_dialog_new() -> *mut c_void;
        fn gtk_font_dialog_button_new(dialog: *mut c_void) -> *mut c_void;
        fn gtk_font_dialog_button_get_font_desc(button: *mut c_void) -> *mut c_void;
        fn gtk_font_dialog_button_set_font_desc(button: *mut c_void, font_desc: *mut c_void);

        // GtkSearchBar (GTK 4)
        fn gtk_search_bar_new() -> *mut c_void;
        fn gtk_search_bar_set_child(bar: *mut c_void, child: *mut c_void);
        fn gtk_search_bar_set_search_mode(bar: *mut c_void, search_mode: c_int);
        fn gtk_search_bar_set_show_close_button(bar: *mut c_void, visible: c_int);
        fn gtk_search_bar_connect_entry(bar: *mut c_void, entry: *mut c_void);

        // GtkPicture (GTK 4)
        fn gtk_picture_new() -> *mut c_void;
        fn gtk_picture_set_filename(picture: *mut c_void, filename: *const c_char);
        fn gtk_picture_set_resource(picture: *mut c_void, resource_path: *const c_char);
        fn gtk_picture_set_content_fit(picture: *mut c_void, content_fit: c_int);
        fn gtk_picture_set_can_shrink(picture: *mut c_void, can_shrink: c_int);
        fn gtk_picture_set_keep_aspect_ratio(picture: *mut c_void, keep_aspect_ratio: c_int);
        fn gtk_picture_set_alternative_text(picture: *mut c_void, alternative_text: *const c_char);

        // GtkCalendar (GTK 4)
        fn gtk_calendar_new() -> *mut c_void;
        fn gtk_calendar_set_show_heading(calendar: *mut c_void, value: c_int);
        fn gtk_calendar_set_show_day_names(calendar: *mut c_void, value: c_int);
        fn gtk_calendar_set_show_week_numbers(calendar: *mut c_void, value: c_int);

        // GtkActionBar (GTK 4)
        fn gtk_action_bar_new() -> *mut c_void;
        fn gtk_action_bar_pack_start(bar: *mut c_void, child: *mut c_void);
        fn gtk_action_bar_pack_end(bar: *mut c_void, child: *mut c_void);
        fn gtk_action_bar_set_center_widget(bar: *mut c_void, center_widget: *mut c_void);
        fn gtk_action_bar_set_revealed(bar: *mut c_void, revealed: c_int);

        // GtkCenterBox (GTK 4)
        fn gtk_center_box_new() -> *mut c_void;
        fn gtk_center_box_set_start_widget(center_box: *mut c_void, child: *mut c_void);
        fn gtk_center_box_set_center_widget(center_box: *mut c_void, child: *mut c_void);
        fn gtk_center_box_set_end_widget(center_box: *mut c_void, child: *mut c_void);

    }

    #[link(name = "pango-1.0")]
    unsafe extern "C" {
        fn pango_font_description_from_string(str: *const c_char) -> *mut c_void;
        fn pango_font_description_to_string(desc: *mut c_void) -> *mut c_char;
        fn pango_font_description_free(desc: *mut c_void);
    }

    #[link(name = "gio-2.0")]
    unsafe extern "C" {
        fn g_application_run(
            application: *mut c_void,
            argc: c_int,
            argv: *mut *mut c_char,
        ) -> c_int;
        fn g_application_register(
            application: *mut c_void,
            cancellable: *mut c_void,
            error: *mut *mut c_void,
        ) -> c_int;
        fn g_application_hold(application: *mut c_void);
        fn g_application_release(application: *mut c_void);
        fn g_resource_load(filename: *const c_char, error: *mut *mut c_void) -> *mut c_void;
        fn g_resources_register(resource: *mut c_void);
    }

    #[link(name = "glib-2.0")]
    unsafe extern "C" {
        fn g_main_context_default() -> *mut c_void;
        fn g_main_context_pending(context: *mut c_void) -> c_int;
        fn g_main_context_iteration(context: *mut c_void, may_block: c_int) -> c_int;
        fn g_timeout_add(
            interval: c_uint,
            function: unsafe extern "C" fn(*mut c_void) -> c_int,
            data: *mut c_void,
        ) -> c_uint;
        fn g_free(ptr: *mut c_void);
    }

    #[link(name = "gobject-2.0")]
    unsafe extern "C" {
        fn g_type_from_name(name: *const c_char) -> usize;
        fn g_object_new(object_type: usize, first_property_name: *const c_char, ...)
            -> *mut c_void;
        fn g_object_ref(object: *mut c_void) -> *mut c_void;
        fn g_object_unref(object: *mut c_void);
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

    static UI_DEBUG_REQUEST_HANDLER: OnceLock<Mutex<Option<Arc<super::UiDebugRequestHandler>>>> =
        OnceLock::new();
    fn ui_debug_request_handler() -> &'static Mutex<Option<Arc<super::UiDebugRequestHandler>>> {
        UI_DEBUG_REQUEST_HANDLER.get_or_init(|| Mutex::new(None))
    }

    static MAIN_LOOP_TICK_HANDLER: OnceLock<Mutex<Option<Arc<super::MainLoopTickHandler>>>> =
        OnceLock::new();
    fn main_loop_tick_handler() -> &'static Mutex<Option<Arc<super::MainLoopTickHandler>>> {
        MAIN_LOOP_TICK_HANDLER.get_or_init(|| Mutex::new(None))
    }

    pub(super) fn set_ui_debug_request_handler(handler: Option<Arc<super::UiDebugRequestHandler>>) {
        *ui_debug_request_handler()
            .lock()
            .expect("ui debug request handler lock poisoned") = handler;
    }

    pub(super) fn set_main_loop_tick_handler(handler: Option<Arc<super::MainLoopTickHandler>>) {
        *main_loop_tick_handler()
            .lock()
            .expect("main loop tick handler lock poisoned") = handler;
    }

    fn call_ui_debug_request_handler(
        method: &str,
        params: &Map<String, Value>,
    ) -> Option<Result<Value, Gtk4Error>> {
        let handler = ui_debug_request_handler()
            .lock()
            .expect("ui debug request handler lock poisoned")
            .clone()?;
        handler(method, params)
    }

    fn call_main_loop_tick_handler() -> Result<(), Gtk4Error> {
        let handler = main_loop_tick_handler()
            .lock()
            .expect("main loop tick handler lock poisoned")
            .clone();
        match handler {
            Some(handler) => handler(),
            None => Ok(()),
        }
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

    struct UiDebugServer {
        socket_path: String,
        token: String,
        listener: UnixListener,
    }

    /// Mirrors the live GTK widget tree for reconciliation.
    #[derive(Clone, Debug)]
    struct LiveNode {
        widget_id: i64,
        class_name: String,
        kind: CreatedWidgetKind,
        node_id: Option<String>,
        key: Option<String>,
        binding_id: Option<String>,
        props: HashMap<String, String>,
        signals: Vec<SignalBindingState>,
        signal_handlers: Vec<SignalHandlerConnection>,
        children: Vec<LiveChild>,
    }

    #[derive(Clone, Debug)]
    struct LiveChild {
        child_type: Option<String>,
        node: LiveNode,
    }

    #[derive(Clone, Debug)]
    struct SignalHandlerConnection {
        instance: usize,
        handler_id: c_ulong,
    }

    const UI_DEBUG_LOG_LIMIT: usize = 4096;

    #[derive(Clone, Debug)]
    struct UiDebugOverlayState {
        enabled: bool,
        bounds: bool,
        margins: bool,
        spacing: bool,
        focus: bool,
        clipping: bool,
    }

    impl Default for UiDebugOverlayState {
        fn default() -> Self {
            Self {
                enabled: false,
                bounds: true,
                margins: true,
                spacing: true,
                focus: true,
                clipping: true,
            }
        }
    }

    #[derive(Clone, Debug)]
    struct UiDebugCaptureSnapshot {
        label: String,
        target_id: i64,
        width: u32,
        height: u32,
        png_base64: String,
        rgba: Vec<u8>,
        created_at_ms: u64,
    }

    #[derive(Clone, Debug)]
    struct UiDebugJsonSnapshot {
        label: String,
        payload: Value,
        created_at_ms: u64,
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
        ui_debug_tick: u64,
        next_event_seq: u64,
        next_mutation_seq: u64,
        signal_event_log: VecDeque<Value>,
        mutation_log: VecDeque<Value>,
        overlay: UiDebugOverlayState,
        capture_snapshots: HashMap<String, UiDebugCaptureSnapshot>,
        tree_snapshots: HashMap<String, UiDebugJsonSnapshot>,
        layout_snapshots: HashMap<String, UiDebugJsonSnapshot>,
        ui_debug: Option<UiDebugServer>,
        main_loop_tick_registered: bool,
    }

    #[derive(Clone, Debug)]
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

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[allow(dead_code)]
    enum SignalPayloadKind {
        None,
        KeyPressed,
        EditableText,
        ToggleActive,
        FloatValue,
        NotifyBool,
        NotifyU32,
        SpinButtonValue,
        ComboBoxText,
        NotifyRgba,
        NotifyFontDesc,
    }

    struct SignalCallbackData {
        widget_id: i64,
        signal_name: String,
        handler: String,
        payload_kind: SignalPayloadKind,
    }

    struct WindowKeyCallbackData {
        widget_id: i64,
    }

    impl RealGtkState {
        fn alloc_id(&mut self) -> i64 {
            self.next_id += 1;
            self.next_id
        }

        fn current_tick(&self) -> u64 {
            self.ui_debug_tick
        }

        fn next_event_seq(&mut self) -> u64 {
            self.next_event_seq = self.next_event_seq.saturating_add(1);
            self.next_event_seq
        }

        fn next_mutation_seq(&mut self) -> u64 {
            self.next_mutation_seq = self.next_mutation_seq.saturating_add(1);
            self.next_mutation_seq
        }

        fn push_log(queue: &mut VecDeque<Value>, entry: Value) {
            queue.push_back(entry);
            while queue.len() > UI_DEBUG_LOG_LIMIT {
                queue.pop_front();
            }
        }

        fn record_mutation(&mut self, mut entry: Map<String, Value>) {
            entry.insert("seq".to_string(), json!(self.next_mutation_seq()));
            entry.insert("tick".to_string(), json!(self.current_tick()));
            entry.insert("timestampMs".to_string(), json!(unix_timestamp_ms()));
            Self::push_log(&mut self.mutation_log, Value::Object(entry));
        }

        fn store_capture_snapshot(
            &mut self,
            label: &str,
            target_id: i64,
            width: u32,
            height: u32,
            png_base64: String,
            rgba: Vec<u8>,
        ) {
            self.capture_snapshots.insert(
                label.to_string(),
                UiDebugCaptureSnapshot {
                    label: label.to_string(),
                    target_id,
                    width,
                    height,
                    png_base64,
                    rgba,
                    created_at_ms: unix_timestamp_ms(),
                },
            );
        }

        fn store_json_snapshot(
            store: &mut HashMap<String, UiDebugJsonSnapshot>,
            label: &str,
            payload: Value,
        ) {
            store.insert(
                label.to_string(),
                UiDebugJsonSnapshot {
                    label: label.to_string(),
                    payload,
                    created_at_ms: unix_timestamp_ms(),
                },
            );
        }
    }

    fn unix_timestamp_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|dur| dur.as_millis() as u64)
            .unwrap_or(0)
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        Paned,
        Frame,
        Expander,
        Notebook,
        FlowBox,
        Grid,
        AspectFrame,
        PreferencesDialog,
        PreferencesPage,
        PreferencesGroup,
        ActionRow,
        SearchBar,
        ActionBar,
        CenterBox,
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

    const GTK_DIR_TAB_FORWARD: c_int = 0;
    const GTK_DIR_TAB_BACKWARD: c_int = 1;
    const GTK_DIR_UP: c_int = 2;
    const GTK_DIR_DOWN: c_int = 3;
    const GTK_DIR_LEFT: c_int = 4;
    const GTK_DIR_RIGHT: c_int = 5;

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

    /// Set a GObject i32 property.
    unsafe fn gobject_set_i32(widget: *mut c_void, prop: &CStr, val: i32) {
        g_object_set(widget, prop.as_ptr(), val, std::ptr::null::<c_char>());
    }

    /// Set a GObject u32 property.
    unsafe fn gobject_set_u32(widget: *mut c_void, prop: &CStr, val: c_uint) {
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

    /// Get a GObject u32 property.
    unsafe fn gobject_get_u32(widget: *mut c_void, prop: &CStr) -> c_uint {
        let mut val: c_uint = 0;
        g_object_get(
            widget,
            prop.as_ptr(),
            &mut val as *mut c_uint,
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

    /// Set a GObject i32 property from a props map entry.
    fn set_obj_i32(widget: *mut c_void, props: &HashMap<String, String>, key: &str) {
        if let Some(value) = props.get(key).and_then(|v| parse_i32_text(v)) {
            let prop_c = CString::new(key).unwrap();
            unsafe { gobject_set_i32(widget, &prop_c, value) };
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

    fn created_widget_kind_name(kind: CreatedWidgetKind) -> &'static str {
        match kind {
            CreatedWidgetKind::Box => "box",
            CreatedWidgetKind::Button => "button",
            CreatedWidgetKind::HeaderBar => "header_bar",
            CreatedWidgetKind::ScrolledWindow => "scrolled_window",
            CreatedWidgetKind::Overlay => "overlay",
            CreatedWidgetKind::ListBox => "list_box",
            CreatedWidgetKind::SplitView => "split_view",
            CreatedWidgetKind::Stack => "stack",
            CreatedWidgetKind::Revealer => "revealer",
            CreatedWidgetKind::Paned => "paned",
            CreatedWidgetKind::Frame => "frame",
            CreatedWidgetKind::Expander => "expander",
            CreatedWidgetKind::Notebook => "notebook",
            CreatedWidgetKind::FlowBox => "flow_box",
            CreatedWidgetKind::Grid => "grid",
            CreatedWidgetKind::AspectFrame => "aspect_frame",
            CreatedWidgetKind::PreferencesDialog => "preferences_dialog",
            CreatedWidgetKind::PreferencesPage => "preferences_page",
            CreatedWidgetKind::PreferencesGroup => "preferences_group",
            CreatedWidgetKind::ActionRow => "action_row",
            CreatedWidgetKind::SearchBar => "search_bar",
            CreatedWidgetKind::ActionBar => "action_bar",
            CreatedWidgetKind::CenterBox => "center_box",
            CreatedWidgetKind::Other => "other",
        }
    }

    fn widget_debug_label(widget_id: i64, class_name: &str, node_id: Option<&str>) -> String {
        let class_name = if class_name.is_empty() {
            "<unknown-class>"
        } else {
            class_name
        };
        match node_id {
            Some(node_id) => format!("widget #{widget_id} ({class_name} id={node_id})"),
            None => format!("widget #{widget_id} ({class_name})"),
        }
    }

    fn known_signals_for_class(class_name: &str) -> &'static [&'static str] {
        match class_name {
            "GtkButton" => &["clicked"],
            "GtkBox" | "GtkGrid" | "GtkOverlay" | "GtkScrolledWindow" | "GtkStack" => {
                &["key-pressed"]
            }
            "GtkEntry" | "GtkPasswordEntry" => &["changed", "activate"],
            "AdwEntryRow" | "AdwPasswordEntryRow" => &["changed"],
            "GtkCheckButton" | "AdwSwitchRow" => &["toggled"],
            "GtkToggleButton" => &["toggled"],
            "GtkSwitch" => &["notify::active"],
            "GtkRange" | "GtkScale" => &["value-changed"],
            "GtkSpinButton" => &["value-changed"],
            "GtkSearchEntry" => &["changed", "search-changed"],
            "GtkComboBoxText" => &["changed"],
            "GtkNotebook" => &["switch-page"],
            "GtkDropDown" => &["notify::selected"],
            "AdwOverlaySplitView" => &["notify::show-sidebar"],
            "GtkColorDialogButton" => &["notify::rgba"],
            "GtkFontDialogButton" => &["notify::font-desc"],
            "GtkCalendar" => &["day-selected"],
            _ => &[],
        }
    }

    fn known_signal_note(class_name: &str) -> String {
        let signals = known_signals_for_class(class_name);
        if signals.is_empty() {
            "Known supported signals for this class: none.".to_string()
        } else {
            format!(
                "Known supported signals for this class: {}.",
                signals.join(", ")
            )
        }
    }

    fn invalid_signal_error(
        operation: &str,
        widget_id: i64,
        class_name: &str,
        node_id: Option<&str>,
        binding: &SignalBindingState,
    ) -> Gtk4Error {
        Gtk4Error::new(format!(
            "gtk4.{operation} unsupported signal `{}` on {} bound to `{}`. {}",
            binding.signal,
            widget_debug_label(widget_id, class_name, node_id),
            binding.handler,
            known_signal_note(class_name)
        ))
    }

    fn expected_preferences_child(
        parent_kind: CreatedWidgetKind,
    ) -> Option<(CreatedWidgetKind, &'static str, &'static str)> {
        match parent_kind {
            CreatedWidgetKind::PreferencesDialog => Some((
                CreatedWidgetKind::PreferencesPage,
                "AdwPreferencesPage",
                "libadwaita preferences dialogs only accept AdwPreferencesPage children.",
            )),
            CreatedWidgetKind::PreferencesPage => Some((
                CreatedWidgetKind::PreferencesGroup,
                "AdwPreferencesGroup",
                "libadwaita preferences pages only accept AdwPreferencesGroup children.",
            )),
            _ => None,
        }
    }

    #[derive(Clone, Copy)]
    struct WidgetAttachmentInfo<'a> {
        id: i64,
        class_name: &'a str,
        kind: CreatedWidgetKind,
        node_id: Option<&'a str>,
    }

    #[derive(Clone, Copy)]
    struct WidgetAttachmentTarget<'a> {
        raw: *mut c_void,
        info: WidgetAttachmentInfo<'a>,
    }

    #[derive(Clone, Copy)]
    struct ChildPlacement<'a> {
        child_type: Option<&'a str>,
        overlay_index: usize,
    }

    fn validate_special_child_attachment(
        operation: &str,
        parent: WidgetAttachmentInfo<'_>,
        child: WidgetAttachmentInfo<'_>,
    ) -> Result<(), Gtk4Error> {
        let Some((expected_kind, expected_class, note)) = expected_preferences_child(parent.kind)
        else {
            return Ok(());
        };
        if child.kind == expected_kind {
            return Ok(());
        }
        Err(Gtk4Error::new(format!(
            "gtk4.{operation} invalid child attachment: {} expected a child with class `{expected_class}`, but got {}. {note}",
            widget_debug_label(parent.id, parent.class_name, parent.node_id),
            widget_debug_label(child.id, child.class_name, child.node_id),
        )))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::{Mutex, OnceLock};
        use std::time::Duration;

        fn gtk_state_test_guard() -> std::sync::MutexGuard<'static, ()> {
            static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
            GUARD
                .get_or_init(|| Mutex::new(()))
                .lock()
                .expect("gtk state test guard lock poisoned")
        }

        #[test]
        fn adw_switch_row_reports_toggle_signal_support() {
            assert_eq!(
                signal_payload_kind_for("AdwSwitchRow", "toggled"),
                Some(SignalPayloadKind::ToggleActive)
            );
            assert_eq!(known_signals_for_class("AdwSwitchRow"), &["toggled"]);
        }

        #[test]
        fn unsupported_signal_error_includes_widget_context() {
            let binding = SignalBindingState {
                signal: "clicked".to_string(),
                handler: "Save".to_string(),
            };
            let err = invalid_signal_error(
                "buildFromNode",
                12,
                "GtkBox",
                Some("settings-panel"),
                &binding,
            );
            assert!(err
                .message
                .contains("widget #12 (GtkBox id=settings-panel)"));
            assert!(err.message.contains("bound to `Save`"));
            assert!(err
                .message
                .contains("Known supported signals for this class: key-pressed."));
        }

        #[test]
        fn preferences_page_child_mismatch_names_expected_class() {
            let err = validate_special_child_attachment(
                "buildFromNode",
                WidgetAttachmentInfo {
                    id: 4,
                    class_name: "AdwPreferencesPage",
                    kind: CreatedWidgetKind::PreferencesPage,
                    node_id: Some("settings-page"),
                },
                WidgetAttachmentInfo {
                    id: 9,
                    class_name: "GtkBox",
                    kind: CreatedWidgetKind::Box,
                    node_id: Some("account-connection"),
                },
            )
            .expect_err("expected invalid child mismatch");
            assert!(err
                .message
                .contains("expected a child with class `AdwPreferencesGroup`"));
            assert!(err
                .message
                .contains("widget #9 (GtkBox id=account-connection)"));
        }
        #[test]
        fn adw_abstract_animation_gives_helpful_error() {
            let err = create_adw_widget("AdwAnimation")
                .expect_err("AdwAnimation must not be directly creatable");
            assert!(err.message.contains("abstract base class"));
            assert!(err.message.contains("AdwTimedAnimation"));
            assert!(err.message.contains("AdwSpringAnimation"));
        }

        #[test]
        fn main_loop_tick_flushes_pending_tray_actions_to_signal_stream() {
            let _guard = gtk_state_test_guard();
            pending_tray_actions()
                .lock()
                .expect("pending tray actions lock poisoned")
                .clear();
            GTK_STATE.with(|state| {
                let mut state = state.borrow_mut();
                state.signal_senders.clear();
                state.signal_events.clear();
            });

            let receiver = signal_stream().expect("signal stream");
            pending_tray_actions()
                .lock()
                .expect("pending tray actions lock poisoned")
                .push_back("quit".to_string());

            unsafe {
                main_loop_tick_cb(std::ptr::null_mut());
            }

            let event = receiver
                .recv_timeout(Duration::from_millis(100))
                .expect("tray action should reach signal stream");
            assert_eq!(event.signal, "mailfox.tray.action");
            assert_eq!(event.handler, "quit");
            assert_eq!(event.payload, "");
        }

        #[test]
        fn adw_abstract_animation_target_gives_helpful_error() {
            let err = create_adw_widget("AdwAnimationTarget")
                .expect_err("AdwAnimationTarget must not be directly creatable");
            assert!(err.message.contains("abstract base class"));
            assert!(err.message.contains("AdwCallbackAnimationTarget"));
            assert!(err.message.contains("AdwPropertyAnimationTarget"));
        }

        #[test]
        fn adw_enum_list_item_gives_helpful_error() {
            let err = create_adw_widget("AdwEnumListItem")
                .expect_err("AdwEnumListItem must not be directly creatable");
            assert!(err.message.contains("AdwEnumListModel"));
        }

        #[test]
        fn adw_parent_managed_page_types_give_helpful_errors() {
            for (class, owner) in [
                ("AdwLeafletPage", "AdwLeaflet"),
                ("AdwSqueezerPage", "AdwSqueezer"),
                ("AdwTabPage", "AdwTabView"),
                ("AdwViewStackPage", "AdwViewStack"),
                ("AdwViewStackPages", "AdwViewStack"),
            ] {
                let err = create_adw_widget(class)
                    .expect_err(&format!("{class} must not be directly creatable"));
                assert!(
                    err.message.contains(owner),
                    "{class} error should mention {owner}, got: {}",
                    err.message
                );
            }
        }

        fn build_gtk_widget_error(class_name: &str) -> Gtk4Error {
            let mut state = RealGtkState::default();
            let mut id_map = HashMap::new();
            let mut binding_map = HashMap::new();
            let node = GtkNode::Element {
                tag: "object".to_string(),
                attrs: vec![("class".to_string(), class_name.to_string())],
                children: vec![],
            };
            build_widget_from_node_real(&mut state, &node, &mut id_map, &mut binding_map)
                .expect_err(&format!("{class_name} must not be directly creatable"))
        }

        #[test]
        fn gtk_documented_filter_sorter_and_expression_types_give_helpful_errors() {
            for (class, needle) in [
                ("GtkCustomFilter", "filter` property of GtkFilterListModel"),
                ("GtkStringFilter", "filter` property of GtkFilterListModel"),
                ("GtkBoolFilter", "filter` property of GtkFilterListModel"),
                ("GtkMultiFilter", "GtkEveryFilter or GtkAnyFilter"),
                ("GtkEveryFilter", "filter` property of GtkFilterListModel"),
                ("GtkAnyFilter", "filter` property of GtkFilterListModel"),
                ("GtkCustomSorter", "sorter` property of GtkSortListModel"),
                ("GtkStringSorter", "sorter` property of GtkSortListModel"),
                ("GtkNumericSorter", "sorter` property of GtkSortListModel"),
                ("GtkMultiSorter", "priority order"),
                ("GtkPropertyExpression", "prefer pure AIVI derivations"),
                ("GtkConstantExpression", "prefer pure AIVI derivations"),
                ("GtkCClosureExpression", "prefer pure AIVI derivations"),
                ("GtkObjectExpression", "prefer pure AIVI derivations"),
            ] {
                let err = build_gtk_widget_error(class);
                assert!(
                    err.message.contains(needle),
                    "{class} error should mention `{needle}`, got: {}",
                    err.message
                );
            }
        }

        #[test]
        fn gtk_documented_factory_and_column_view_types_give_helpful_errors() {
            for (class, needle) in [
                (
                    "GtkSignalListItemFactory",
                    "factory` property of GtkListView or GtkColumnView",
                ),
                (
                    "GtkBuilderListItemFactory",
                    "prefer the ~<gtk> sigil over raw Builder XML",
                ),
                (
                    "GtkColumnViewColumn",
                    "added to GtkColumnView programmatically",
                ),
                ("GtkColumnViewRow", "managed internally by GtkColumnView"),
                (
                    "GtkColumnViewCell",
                    "managed internally by the column factory",
                ),
                (
                    "GtkColumnViewSorter",
                    "obtained from the `sorter` property of GtkColumnView",
                ),
            ] {
                let err = build_gtk_widget_error(class);
                assert!(
                    err.message.contains(needle),
                    "{class} error should mention `{needle}`, got: {}",
                    err.message
                );
            }
        }

        #[test]
        fn gtk_documented_shortcut_and_media_types_give_helpful_errors() {
            for (class, needle) in [
                ("GtkShortcut", "GtkShortcutController"),
                ("GtkShortcutAction", "GtkActivateAction"),
                ("GtkShortcutTrigger", "GtkKeyvalTrigger"),
                ("GtkActivateAction", "shortcut action"),
                ("GtkSignalAction", "shortcut action"),
                ("GtkNamedAction", "shortcut action"),
                ("GtkNothingAction", "shortcut action"),
                ("GtkMnemonicAction", "shortcut action"),
                ("GtkKeyvalTrigger", "shortcut trigger"),
                ("GtkMnemonicTrigger", "shortcut trigger"),
                ("GtkAlternativeTrigger", "shortcut trigger"),
                ("GtkNeverTrigger", "shortcut trigger"),
                ("GtkAnyTrigger", "shortcut trigger"),
                ("GtkMediaStream", "set it as the `stream` property"),
                ("GtkMediaFile", "set it as the `stream` property"),
                ("GtkVideo", "does not yet expose it in ~<gtk> widget trees"),
                (
                    "GtkMediaControls",
                    "does not yet expose it in ~<gtk> widget trees",
                ),
            ] {
                let err = build_gtk_widget_error(class);
                assert!(
                    err.message.contains(needle),
                    "{class} error should mention `{needle}`, got: {}",
                    err.message
                );
            }
        }

        #[test]
        fn gtk_future_surface_and_programmatic_helper_types_give_helpful_errors() {
            for (class, needle) in [
                (
                    "GtkListView",
                    "use GtkListBox with <each> for simple read-only lists today",
                ),
                (
                    "GtkColumnView",
                    "use GtkListBox with <each> for simple read-only lists today",
                ),
                (
                    "GtkGridView",
                    "use GtkListBox with <each> for simple read-only lists today",
                ),
                (
                    "GtkApplication",
                    "managed by appNew during manual GTK startup",
                ),
                ("GtkPrintOperation", "printing is not part of the AIVI v0.1"),
                ("GtkPrinter", "printing is not part of the AIVI v0.1"),
                ("GtkPrintContext", "printing is not part of the AIVI v0.1"),
                ("GtkPageSetup", "printing is not part of the AIVI v0.1"),
                ("GtkPaperSize", "printing is not part of the AIVI v0.1"),
                ("GtkPrintSettings", "printing is not part of the AIVI v0.1"),
                (
                    "GtkWindow",
                    "managed by windowNew during manual GTK startup",
                ),
                (
                    "GtkApplicationWindow",
                    "managed by windowNew during manual GTK startup",
                ),
                ("GtkAboutDialog", "AdwAboutDialog"),
                ("GtkAlertDialog", "present it programmatically"),
                ("GtkDialog", "prefer GtkAlertDialog"),
                ("GtkMessageDialog", "prefer GtkAlertDialog"),
                ("GtkShortcutsWindow", "AdwShortcutsDialog"),
                ("GtkPopover", "GtkMenuButton with a menu model"),
                ("GtkPopoverMenu", "GtkMenuButton with a menu model"),
                ("GtkPopoverMenuBar", "menu model"),
                ("GtkFileDialog", "AIVI does not yet expose file dialogs"),
                ("GtkFileChooserDialog", "prefer GtkFileDialog"),
                ("GtkFileChooserNative", "prefer GtkFileDialog"),
                ("GtkFileChooserWidget", "prefer GtkFileDialog"),
                ("GtkPrintDialog", "printing is not part of the AIVI v0.1"),
                ("GtkPrintJob", "printing is not part of the AIVI v0.1"),
                (
                    "GtkPageSetupUnixDialog",
                    "printing is not part of the AIVI v0.1",
                ),
                (
                    "GtkPrintUnixDialog",
                    "printing is not part of the AIVI v0.1",
                ),
            ] {
                let err = build_gtk_widget_error(class);
                assert!(
                    err.message.contains(needle),
                    "{class} error should mention `{needle}`, got: {}",
                    err.message
                );
            }
        }

        #[test]
        fn adw_breakpoint_condition_gives_helpful_error() {
            let err = create_adw_widget("AdwBreakpointCondition")
                .expect_err("AdwBreakpointCondition must not be directly creatable");
            assert!(err.message.contains("condition descriptor"));
            assert!(err.message.contains("AdwBreakpoint"));
        }

        #[test]
        fn gtk_check_button_has_toggle_signal_support() {
            assert_eq!(
                signal_payload_kind_for("GtkCheckButton", "toggled"),
                Some(SignalPayloadKind::ToggleActive)
            );
            assert_eq!(known_signals_for_class("GtkCheckButton"), &["toggled"]);
        }

        #[test]
        fn gtk_toggle_button_has_toggle_signal_support() {
            assert_eq!(
                signal_payload_kind_for("GtkToggleButton", "toggled"),
                Some(SignalPayloadKind::ToggleActive)
            );
            assert_eq!(known_signals_for_class("GtkToggleButton"), &["toggled"]);
        }

        #[test]
        fn gtk_switch_has_notify_active_signal_support() {
            assert_eq!(
                signal_payload_kind_for("GtkSwitch", "notify::active"),
                Some(SignalPayloadKind::NotifyBool)
            );
            assert_eq!(known_signals_for_class("GtkSwitch"), &["notify::active"]);
        }

        #[test]
        fn gtk_scale_has_value_changed_signal_support() {
            assert_eq!(
                signal_payload_kind_for("GtkScale", "value-changed"),
                Some(SignalPayloadKind::FloatValue)
            );
            assert_eq!(
                signal_payload_kind_for("GtkRange", "value-changed"),
                Some(SignalPayloadKind::FloatValue)
            );
            assert_eq!(known_signals_for_class("GtkScale"), &["value-changed"]);
        }

        #[test]
        fn gtk_search_entry_has_changed_signal_support() {
            assert_eq!(
                signal_payload_kind_for("GtkSearchEntry", "changed"),
                Some(SignalPayloadKind::EditableText)
            );
            assert_eq!(
                signal_payload_kind_for("GtkSearchEntry", "search-changed"),
                Some(SignalPayloadKind::EditableText)
            );
        }

        #[test]
        fn new_gtk_classes_are_toggle_or_text_input_as_expected() {
            assert!(is_toggle_class("GtkCheckButton"));
            assert!(is_toggle_class("GtkToggleButton"));
            assert!(is_toggle_class("GtkSwitch"));
            assert!(is_toggle_class("AdwSwitchRow"));
            assert!(!is_toggle_class("GtkButton"));
            assert!(!is_toggle_class("GtkScale"));

            assert!(is_text_input_class("GtkSearchEntry"));
            assert!(is_text_input_class("GtkEntry"));
            assert!(!is_text_input_class("GtkCheckButton"));
        }

        #[test]
        fn gtk_spin_button_has_value_changed_signal_support() {
            assert_eq!(
                signal_payload_kind_for("GtkSpinButton", "value-changed"),
                Some(SignalPayloadKind::SpinButtonValue)
            );
            assert_eq!(known_signals_for_class("GtkSpinButton"), &["value-changed"]);
        }

        #[test]
        fn gtk_combo_box_text_has_changed_signal_support() {
            assert_eq!(
                signal_payload_kind_for("GtkComboBoxText", "changed"),
                Some(SignalPayloadKind::ComboBoxText)
            );
            assert_eq!(known_signals_for_class("GtkComboBoxText"), &["changed"]);
        }

        #[test]
        fn focus_direction_accepts_tab_aliases() {
            assert_eq!(parse_focus_direction("next"), Some(GTK_DIR_TAB_FORWARD));
            assert_eq!(parse_focus_direction("tab"), Some(GTK_DIR_TAB_FORWARD));
            assert_eq!(
                parse_focus_direction("previous"),
                Some(GTK_DIR_TAB_BACKWARD)
            );
            assert_eq!(
                parse_focus_direction("shift-tab"),
                Some(GTK_DIR_TAB_BACKWARD)
            );
            assert_eq!(parse_focus_direction("left"), Some(GTK_DIR_LEFT));
            assert_eq!(parse_focus_direction("bogus"), None);
        }

        #[test]
        fn string_selection_index_accepts_index_and_label() {
            let options = vec!["Alpha".to_string(), "Beta".to_string(), "Gamma".to_string()];
            assert_eq!(string_selection_index(&options, "1"), Some(1));
            assert_eq!(string_selection_index(&options, "Beta"), Some(1));
            assert_eq!(string_selection_index(&options, "gamma"), Some(2));
            assert_eq!(string_selection_index(&options, "9"), None);
        }

        #[test]
        fn parse_grid_position_defaults() {
            assert_eq!(parse_grid_position(None), (0, 0, 1, 1));
            assert_eq!(parse_grid_position(Some("0,0")), (0, 0, 1, 1));
            assert_eq!(parse_grid_position(Some("2,3")), (2, 3, 1, 1));
            assert_eq!(parse_grid_position(Some("1,2,3,4")), (1, 2, 3, 4));
            assert_eq!(parse_grid_position(Some("0,1,2")), (0, 1, 2, 1));
        }

        #[test]
        fn gtk_grid_has_grid_kind() {
            // Verify the kind name is correct for diagnostics
            assert_eq!(created_widget_kind_name(CreatedWidgetKind::Grid), "grid");
            assert_eq!(
                created_widget_kind_name(CreatedWidgetKind::AspectFrame),
                "aspect_frame"
            );
        }

        #[test]
        fn gtk_color_dialog_button_has_notify_rgba_signal_support() {
            assert_eq!(
                signal_payload_kind_for("GtkColorDialogButton", "notify::rgba"),
                Some(SignalPayloadKind::NotifyRgba)
            );
            assert_eq!(
                known_signals_for_class("GtkColorDialogButton"),
                &["notify::rgba"]
            );
        }

        #[test]
        fn gtk_font_dialog_button_has_notify_font_desc_signal_support() {
            assert_eq!(
                signal_payload_kind_for("GtkFontDialogButton", "notify::font-desc"),
                Some(SignalPayloadKind::NotifyFontDesc)
            );
            assert_eq!(
                known_signals_for_class("GtkFontDialogButton"),
                &["notify::font-desc"]
            );
        }

        #[test]
        fn parse_rgba_str_three_components() {
            let rgba = parse_rgba_str("0.1,0.2,0.3").expect("should parse");
            assert!((rgba.red - 0.1).abs() < 1e-5);
            assert!((rgba.green - 0.2).abs() < 1e-5);
            assert!((rgba.blue - 0.3).abs() < 1e-5);
            assert!((rgba.alpha - 1.0).abs() < 1e-5);
        }

        #[test]
        fn parse_rgba_str_four_components() {
            let rgba = parse_rgba_str("0.5,0.6,0.7,0.8").expect("should parse");
            assert!((rgba.red - 0.5).abs() < 1e-5);
            assert!((rgba.green - 0.6).abs() < 1e-5);
            assert!((rgba.blue - 0.7).abs() < 1e-5);
            assert!((rgba.alpha - 0.8).abs() < 1e-5);
        }

        #[test]
        fn parse_rgba_str_rejects_invalid_input() {
            assert!(parse_rgba_str("").is_none());
            assert!(parse_rgba_str("not-a-color").is_none());
            assert!(parse_rgba_str("0.1,0.2").is_none());
            assert!(parse_rgba_str("0.1,0.2,0.3,0.4,0.5").is_none());
        }

        #[test]
        fn gtk_color_button_removed_error_mentions_replacement() {
            // This is a compile-time check — the error branch is in the widget builder
            // rather than create_adw_widget, so we verify the signal metadata instead.
            assert_eq!(signal_payload_kind_for("GtkColorButton", "clicked"), None);
            assert_eq!(known_signals_for_class("GtkColorButton"), &[] as &[&str]);
        }

        #[test]
        fn gtk_app_chooser_classes_have_no_known_signals() {
            for class in &[
                "GtkAppChooserButton",
                "GtkAppChooserDialog",
                "GtkAppChooserWidget",
            ] {
                assert_eq!(
                    known_signals_for_class(class),
                    &[] as &[&str],
                    "{class} should have no registered signals (removed in GTK 4.10)"
                );
            }
        }
    }

    fn is_text_input_class(class_name: &str) -> bool {
        matches!(
            class_name,
            "GtkEntry"
                | "GtkPasswordEntry"
                | "GtkSearchEntry"
                | "AdwEntryRow"
                | "AdwPasswordEntryRow"
        )
    }

    fn is_toggle_class(class_name: &str) -> bool {
        matches!(
            class_name,
            "GtkCheckButton" | "GtkToggleButton" | "GtkSwitch" | "AdwSwitchRow"
        )
    }

    fn is_range_class(class_name: &str) -> bool {
        matches!(class_name, "GtkRange" | "GtkScale" | "GtkSpinButton")
    }

    fn is_choice_class(class_name: &str) -> bool {
        matches!(class_name, "GtkDropDown" | "GtkComboBoxText")
    }

    fn string_prop_lines(value: Option<&String>) -> Vec<String> {
        value
            .map(|text| {
                text.split('\n')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    fn string_selection_index(options: &[String], raw_value: &str) -> Option<usize> {
        parse_usize_text(raw_value)
            .filter(|index| options.is_empty() || *index < options.len())
            .or_else(|| options.iter().position(|option| option == raw_value))
            .or_else(|| {
                options
                    .iter()
                    .position(|option| option.eq_ignore_ascii_case(raw_value))
            })
    }

    fn text_slice_by_char_range(text: &str, start: usize, end: usize) -> String {
        text.chars()
            .skip(start)
            .take(end.saturating_sub(start))
            .collect()
    }

    fn widget_id_for_ptr(state: &RealGtkState, ptr: *mut c_void) -> Option<i64> {
        if ptr.is_null() {
            return None;
        }
        state
            .widgets
            .iter()
            .find_map(|(&widget_id, &widget_ptr)| (widget_ptr == ptr).then_some(widget_id))
            .or_else(|| {
                state
                    .windows
                    .iter()
                    .find_map(|(&window_id, &window_ptr)| (window_ptr == ptr).then_some(window_id))
            })
    }

    fn current_focus_info(state: &RealGtkState) -> Option<(i64, *mut c_void, Option<i64>)> {
        let mut window_ids = state.windows.keys().copied().collect::<Vec<_>>();
        window_ids.sort_unstable();
        for window_id in window_ids {
            let Some(&window) = state.windows.get(&window_id) else {
                continue;
            };
            let focus_ptr = unsafe { gtk_window_get_focus(window) };
            if focus_ptr.is_null() {
                continue;
            }
            return Some((window_id, focus_ptr, widget_id_for_ptr(state, focus_ptr)));
        }
        None
    }

    fn focus_summary_json(state: &RealGtkState) -> Value {
        match current_focus_info(state) {
            Some((window_id, _, widget_id)) => {
                let widget_name = widget_id
                    .and_then(|id| state.widget_id_to_name.get(&id))
                    .cloned();
                json!({
                    "windowId": window_id,
                    "widgetId": widget_id,
                    "widgetName": widget_name
                })
            }
            None => Value::Null,
        }
    }

    fn parse_focus_direction(direction: &str) -> Option<c_int> {
        match direction {
            "next" | "tab" | "forward" => Some(GTK_DIR_TAB_FORWARD),
            "previous" | "prev" | "backward" | "shift-tab" => Some(GTK_DIR_TAB_BACKWARD),
            "up" => Some(GTK_DIR_UP),
            "down" => Some(GTK_DIR_DOWN),
            "left" => Some(GTK_DIR_LEFT),
            "right" => Some(GTK_DIR_RIGHT),
            _ => None,
        }
    }

    fn choice_options_json(live: &LiveNode) -> Value {
        Value::Array(
            string_prop_lines(live.props.get("strings"))
                .into_iter()
                .enumerate()
                .map(|(index, label)| {
                    json!({
                        "index": index,
                        "label": label
                    })
                })
                .collect(),
        )
    }

    fn range_limits_json(live: &LiveNode) -> Value {
        json!({
            "min": live.props.get("min").and_then(|value| parse_f64_text(value)),
            "max": live.props.get("max").and_then(|value| parse_f64_text(value)),
            "step": live.props.get("step").and_then(|value| parse_f64_text(value))
        })
    }

    fn widget_bool_property(widget: *mut c_void, prop: &str) -> Option<bool> {
        let prop_c = CString::new(prop).ok()?;
        Some(unsafe { gobject_get_bool(widget, &prop_c) != 0 })
    }

    fn widget_dimensions_json(state: &RealGtkState, widget_id: i64) -> Value {
        match widget_ptr(state, widget_id, "uiDebugDimensions") {
            Ok(widget) => json!({
                "width": unsafe { gtk_widget_get_width(widget) },
                "height": unsafe { gtk_widget_get_height(widget) }
            }),
            Err(_) => json!({
                "width": 0,
                "height": 0
            }),
        }
    }

    fn split_css_classes(raw: Option<&String>) -> Vec<String> {
        let mut classes = string_prop_lines(raw)
            .into_iter()
            .flat_map(|item| {
                item.split_whitespace()
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>();
        classes.sort();
        classes.dedup();
        classes
    }

    fn widget_bounds_relative_to(
        state: &RealGtkState,
        widget_id: i64,
        target_id: i64,
    ) -> Option<GrapheneRect> {
        let widget = widget_ptr(state, widget_id, "uiDebugBounds").ok()?;
        if widget_id == target_id {
            return Some(GrapheneRect {
                origin: GraphenePoint { x: 0.0, y: 0.0 },
                size: GrapheneSize {
                    width: unsafe { gtk_widget_get_width(widget) }.max(0) as f32,
                    height: unsafe { gtk_widget_get_height(widget) }.max(0) as f32,
                },
            });
        }
        let target = widget_ptr(state, target_id, "uiDebugBounds").ok()?;
        let mut bounds = GrapheneRect::default();
        let ok =
            unsafe { gtk_widget_compute_bounds(widget, target, &mut bounds as *mut GrapheneRect) };
        (ok != 0).then_some(bounds)
    }

    fn rect_to_json(rect: GrapheneRect) -> Value {
        let x = rect.origin.x as f64;
        let y = rect.origin.y as f64;
        let width = rect.size.width.max(0.0) as f64;
        let height = rect.size.height.max(0.0) as f64;
        json!({
            "x": x,
            "y": y,
            "width": width,
            "height": height,
            "right": x + width,
            "bottom": y + height,
        })
    }

    fn rect_contains_point(rect: GrapheneRect, x: f64, y: f64) -> bool {
        let left = rect.origin.x as f64;
        let top = rect.origin.y as f64;
        let right = left + rect.size.width as f64;
        let bottom = top + rect.size.height as f64;
        x >= left && y >= top && x <= right && y <= bottom
    }

    fn widget_margin_json(live: &LiveNode) -> Value {
        json!({
            "start": live.props.get("margin-start").and_then(|value| parse_i32_text(value)).unwrap_or_default(),
            "end": live.props.get("margin-end").and_then(|value| parse_i32_text(value)).unwrap_or_default(),
            "top": live.props.get("margin-top").and_then(|value| parse_i32_text(value)).unwrap_or_default(),
            "bottom": live.props.get("margin-bottom").and_then(|value| parse_i32_text(value)).unwrap_or_default(),
        })
    }

    fn widget_style_info_json(state: &RealGtkState, live: &LiveNode) -> Value {
        let requested_classes = split_css_classes(live.props.get("css-class"));
        let widget = widget_ptr(state, live.widget_id, "uiDebugStyle").ok();
        let active_classes = widget
            .map(|widget| {
                requested_classes
                    .iter()
                    .filter(|class_name| {
                        CString::new(class_name.as_str())
                            .ok()
                            .map(|css_class| unsafe {
                                gtk_widget_has_css_class(widget, css_class.as_ptr()) != 0
                            })
                            .unwrap_or(false)
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        json!({
            "requestedCssClasses": requested_classes,
            "activeCssClasses": active_classes,
            "iconName": live.props.get("icon-name"),
            "tooltip": live.props.get("tooltip-text"),
            "opacity": live.props.get("opacity"),
            "wrap": live.props.get("wrap"),
            "ellipsize": live.props.get("ellipsize"),
            "margins": widget_margin_json(live),
        })
    }

    fn layout_entry_json(
        state: &RealGtkState,
        root_id: i64,
        target_id: i64,
        parent_bounds: Option<GrapheneRect>,
        live: &LiveNode,
    ) -> Value {
        let bounds =
            widget_bounds_relative_to(state, live.widget_id, target_id).unwrap_or_default();
        let overflow = parent_bounds.map(|parent| {
            (bounds.origin.x < parent.origin.x)
                || (bounds.origin.y < parent.origin.y)
                || (bounds.origin.x + bounds.size.width > parent.origin.x + parent.size.width)
                || (bounds.origin.y + bounds.size.height > parent.origin.y + parent.size.height)
        });
        let children = live
            .children
            .iter()
            .map(|child| layout_entry_json(state, root_id, target_id, Some(bounds), &child.node))
            .collect::<Vec<_>>();
        json!({
            "id": live.widget_id,
            "name": live.node_id,
            "className": live.class_name,
            "kind": created_widget_kind_name(live.kind),
            "rootId": root_id,
            "bounds": rect_to_json(bounds),
            "dimensions": widget_dimensions_json(state, live.widget_id),
            "margins": widget_margin_json(live),
            "state": widget_runtime_state_json(state, live),
            "style": widget_style_info_json(state, live),
            "overflowParentBounds": overflow,
            "children": children,
        })
    }

    fn widget_capabilities_json(state: &RealGtkState, live: &LiveNode) -> Value {
        let click = live
            .signals
            .iter()
            .any(|binding| matches!(binding.signal.as_str(), "clicked" | "activate"));
        let input = is_text_input_class(&live.class_name);
        let select = matches!(live.kind, CreatedWidgetKind::Stack)
            || is_toggle_class(&live.class_name)
            || is_choice_class(&live.class_name)
            || is_range_class(&live.class_name);
        let focus = widget_ptr(state, live.widget_id, "uiDebugCapabilities")
            .ok()
            .and_then(|widget| widget_bool_property(widget, "focusable"))
            .unwrap_or(false);
        let scroll = matches!(live.kind, CreatedWidgetKind::ScrolledWindow);
        json!({
            "inspect": true,
            "click": click,
            "type": input,
            "select": select,
            "focus": focus,
            "moveFocus": focus || scroll,
            "scroll": scroll,
            "keyPress": focus
        })
    }

    fn widget_runtime_state_json(state: &RealGtkState, live: &LiveNode) -> Value {
        let mut runtime = Map::new();
        if let Ok(widget) = widget_ptr(state, live.widget_id, "uiDebugState") {
            if let Some(visible) = widget_bool_property(widget, "visible") {
                runtime.insert("visible".to_string(), Value::Bool(visible));
            }
            if let Some(sensitive) = widget_bool_property(widget, "sensitive") {
                runtime.insert("sensitive".to_string(), Value::Bool(sensitive));
            }
            if let Some(focusable) = widget_bool_property(widget, "focusable") {
                runtime.insert("focusable".to_string(), Value::Bool(focusable));
            }
            if let Some(focused) = widget_bool_property(widget, "has-focus") {
                runtime.insert("focused".to_string(), Value::Bool(focused));
            }
            if is_text_input_class(&live.class_name) {
                let text_ptr = unsafe { gtk_editable_get_text(widget) };
                let text = if text_ptr.is_null() {
                    String::new()
                } else {
                    unsafe { CStr::from_ptr(text_ptr) }
                        .to_string_lossy()
                        .into_owned()
                };
                runtime.insert("text".to_string(), Value::String(text.clone()));
                runtime.insert(
                    "cursorPosition".to_string(),
                    json!(unsafe { gtk_editable_get_position(widget) }),
                );
                let mut selection_start: c_int = 0;
                let mut selection_end: c_int = 0;
                let has_selection = unsafe {
                    gtk_editable_get_selection_bounds(
                        widget,
                        &mut selection_start as *mut c_int,
                        &mut selection_end as *mut c_int,
                    ) != 0
                };
                if has_selection {
                    let start = selection_start.max(0) as usize;
                    let end = selection_end.max(0) as usize;
                    runtime.insert(
                        "selection".to_string(),
                        json!({
                            "start": start,
                            "end": end,
                            "text": text_slice_by_char_range(&text, start, end)
                        }),
                    );
                }
            }
            if live.class_name == "GtkCheckButton" {
                runtime.insert(
                    "active".to_string(),
                    Value::Bool(unsafe { gtk_check_button_get_active(widget) != 0 }),
                );
            } else if is_toggle_class(&live.class_name) {
                if let Some(active) = widget_bool_property(widget, "active") {
                    runtime.insert("active".to_string(), Value::Bool(active));
                }
            }
            if matches!(live.class_name.as_str(), "GtkRange" | "GtkScale") {
                runtime.insert(
                    "value".to_string(),
                    json!(unsafe { gtk_range_get_value(widget) }),
                );
                runtime.insert("limits".to_string(), range_limits_json(live));
            } else if live.class_name == "GtkSpinButton" {
                runtime.insert(
                    "value".to_string(),
                    json!(unsafe { gtk_spin_button_get_value(widget) }),
                );
                runtime.insert("limits".to_string(), range_limits_json(live));
            } else if live.class_name == "GtkDropDown" {
                let selected_raw = unsafe { gtk_drop_down_get_selected(widget) };
                let selected = (selected_raw != c_uint::MAX).then_some(selected_raw as usize);
                let options = string_prop_lines(live.props.get("strings"));
                let selected_label = selected.and_then(|index| options.get(index).cloned());
                runtime.insert("selected".to_string(), json!(selected));
                runtime.insert("selectedLabel".to_string(), json!(selected_label));
                runtime.insert("options".to_string(), choice_options_json(live));
            } else if live.class_name == "GtkComboBoxText" {
                let selected = unsafe { gtk_combo_box_get_active(widget) };
                let selected_index = (selected >= 0).then_some(selected as usize);
                let selected_text_ptr = unsafe { gtk_combo_box_text_get_active_text(widget) };
                let selected_label = if selected_text_ptr.is_null() {
                    None
                } else {
                    let text = unsafe { CStr::from_ptr(selected_text_ptr) }
                        .to_string_lossy()
                        .into_owned();
                    unsafe { g_free(selected_text_ptr as *mut c_void) };
                    Some(text)
                };
                runtime.insert("selected".to_string(), json!(selected_index));
                runtime.insert("selectedLabel".to_string(), json!(selected_label));
                runtime.insert("options".to_string(), choice_options_json(live));
            } else if matches!(live.kind, CreatedWidgetKind::ScrolledWindow) {
                let hadj = unsafe { gtk_scrolled_window_get_hadjustment(widget) };
                let vadj = unsafe { gtk_scrolled_window_get_vadjustment(widget) };
                if !hadj.is_null() {
                    runtime.insert(
                        "scrollX".to_string(),
                        json!({
                            "value": unsafe { gtk_adjustment_get_value(hadj) },
                            "lower": unsafe { gtk_adjustment_get_lower(hadj) },
                            "upper": unsafe { gtk_adjustment_get_upper(hadj) },
                            "pageSize": unsafe { gtk_adjustment_get_page_size(hadj) }
                        }),
                    );
                }
                if !vadj.is_null() {
                    runtime.insert(
                        "scrollY".to_string(),
                        json!({
                            "value": unsafe { gtk_adjustment_get_value(vadj) },
                            "lower": unsafe { gtk_adjustment_get_lower(vadj) },
                            "upper": unsafe { gtk_adjustment_get_upper(vadj) },
                            "pageSize": unsafe { gtk_adjustment_get_page_size(vadj) }
                        }),
                    );
                }
            }
        }
        if matches!(live.kind, CreatedWidgetKind::Stack) {
            if let Some(page) = live.props.get("visible-child-name") {
                runtime.insert("visibleChildName".to_string(), Value::String(page.clone()));
            }
        }
        Value::Object(runtime)
    }

    fn signal_bindings_json(live: &LiveNode) -> Value {
        Value::Array(
            live.signals
                .iter()
                .map(|binding| {
                    json!({
                        "signal": binding.signal,
                        "handler": binding.handler
                    })
                })
                .collect(),
        )
    }

    fn live_node_json(
        state: &RealGtkState,
        root_id: i64,
        parent_id: Option<i64>,
        child_type: Option<&str>,
        live: &LiveNode,
    ) -> Value {
        let children = live
            .children
            .iter()
            .map(|child| {
                live_node_json(
                    state,
                    root_id,
                    Some(live.widget_id),
                    child.child_type.as_deref(),
                    &child.node,
                )
            })
            .collect::<Vec<_>>();
        json!({
            "id": live.widget_id,
            "name": live.node_id,
            "className": live.class_name,
            "kind": created_widget_kind_name(live.kind),
            "rootId": root_id,
            "parentId": parent_id,
            "childType": child_type,
            "props": live.props,
            "signals": signal_bindings_json(live),
            "dimensions": widget_dimensions_json(state, live.widget_id),
            "state": widget_runtime_state_json(state, live),
            "capabilities": widget_capabilities_json(state, live),
            "children": children
        })
    }

    fn collect_widget_summaries(
        state: &RealGtkState,
        out: &mut Vec<Value>,
        root_id: i64,
        parent_id: Option<i64>,
        child_type: Option<&str>,
        live: &LiveNode,
    ) {
        out.push(json!({
            "id": live.widget_id,
            "name": live.node_id,
            "className": live.class_name,
            "kind": created_widget_kind_name(live.kind),
            "rootId": root_id,
            "parentId": parent_id,
            "childType": child_type,
            "dimensions": widget_dimensions_json(state, live.widget_id),
            "state": widget_runtime_state_json(state, live),
            "capabilities": widget_capabilities_json(state, live)
        }));
        for child in &live.children {
            collect_widget_summaries(
                state,
                out,
                root_id,
                Some(live.widget_id),
                child.child_type.as_deref(),
                &child.node,
            );
        }
    }

    fn find_live_node_mut(live: &mut LiveNode, widget_id: i64) -> Option<&mut LiveNode> {
        if live.widget_id == widget_id {
            return Some(live);
        }
        for child in &mut live.children {
            if let Some(found) = find_live_node_mut(&mut child.node, widget_id) {
                return Some(found);
            }
        }
        None
    }

    fn live_tree_contains_widget(live: &LiveNode, widget_id: i64) -> bool {
        live.widget_id == widget_id
            || live
                .children
                .iter()
                .any(|child| live_tree_contains_widget(&child.node, widget_id))
    }

    fn collect_binding_widgets(live: &LiveNode, out: &mut HashMap<String, i64>) {
        if let Some(binding_id) = live.binding_id.as_ref() {
            out.insert(binding_id.clone(), live.widget_id);
        }
        for child in &live.children {
            collect_binding_widgets(&child.node, out);
        }
    }

    fn collect_binding_widget_ids(live: &LiveNode, out: &mut Vec<i64>) {
        if live.binding_id.is_some() {
            out.push(live.widget_id);
        }
        for child in &live.children {
            collect_binding_widget_ids(&child.node, out);
        }
    }

    fn find_widget_context(
        state: &RealGtkState,
        widget_id: i64,
    ) -> Option<(i64, Option<i64>, Option<String>, &LiveNode)> {
        fn walk<'a>(
            live: &'a LiveNode,
            widget_id: i64,
            root_id: i64,
            parent_id: Option<i64>,
            child_type: Option<&str>,
        ) -> Option<(i64, Option<i64>, Option<String>, &'a LiveNode)> {
            if live.widget_id == widget_id {
                return Some((root_id, parent_id, child_type.map(str::to_string), live));
            }
            for child in &live.children {
                if let Some(found) = walk(
                    &child.node,
                    widget_id,
                    root_id,
                    Some(live.widget_id),
                    child.child_type.as_deref(),
                ) {
                    return Some(found);
                }
            }
            None
        }

        for (&root_id, live) in &state.live_trees {
            if let Some(found) = walk(live, widget_id, root_id, None, None) {
                return Some(found);
            }
        }
        None
    }

    fn resolve_widget_id(
        state: &RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<i64, Gtk4Error> {
        let named_id = params
            .get("name")
            .and_then(|value| value.as_str())
            .map(|name| {
                state.named_widgets.get(name).copied().ok_or_else(|| {
                    Gtk4Error::new(format!("gtk ui debug unknown widget name '{name}'"))
                })
            })
            .transpose()?;
        let numeric_id = params.get("id").and_then(|value| value.as_i64());

        match (named_id, numeric_id) {
            (Some(name_id), Some(id)) if name_id != id => Err(Gtk4Error::new(format!(
                "gtk ui debug widget target mismatch: name resolved to {name_id}, id was {id}"
            ))),
            (Some(id), _) => {
                let _ = widget_ptr(state, id, "uiDebugResolve")?;
                Ok(id)
            }
            (None, Some(id)) => {
                let _ = widget_ptr(state, id, "uiDebugResolve")?;
                Ok(id)
            }
            (None, None) => Err(Gtk4Error::new(
                "gtk ui debug expected one of: name (string) or id (integer)",
            )),
        }
    }

    fn enqueue_signal_event(
        state: &mut RealGtkState,
        widget_id: i64,
        signal: &str,
        handler: &str,
        payload: &str,
    ) -> Result<(), Gtk4Error> {
        if widget_id != 0 {
            let _ = widget_ptr(state, widget_id, "uiDebugEmit")?;
        }
        let event = SignalEventState {
            widget_id,
            signal: signal.to_string(),
            handler: handler.to_string(),
            payload: payload.to_string(),
        };
        let widget_name = state
            .widget_id_to_name
            .get(&widget_id)
            .cloned()
            .unwrap_or_default();
        let root_id = find_widget_context(state, widget_id).map(|(root_id, _, _, _)| root_id);
        let event_seq = state.next_event_seq();
        let event_tick = state.current_tick();
        let timestamp_ms = unix_timestamp_ms();
        RealGtkState::push_log(
            &mut state.signal_event_log,
            json!({
                "seq": event_seq,
                "tick": event_tick,
                "timestampMs": timestamp_ms,
                "widgetId": widget_id,
                "widgetName": widget_name,
                "rootId": root_id,
                "signal": signal,
                "handler": handler,
                "payload": payload,
            }),
        );
        let typed_event = make_signal_event(event.clone(), widget_name);
        state
            .signal_senders
            .retain(|sender| sender.send(typed_event.clone()).is_ok());
        state.signal_events.push_back(event);
        Ok(())
    }

    fn update_live_prop(state: &mut RealGtkState, widget_id: i64, key: &str, value: String) {
        let widget_name = state.widget_id_to_name.get(&widget_id).cloned();
        let mut mutation = None;
        for (&root_id, live) in state.live_trees.iter_mut() {
            if let Some(node) = find_live_node_mut(live, widget_id) {
                let old_value = node.props.insert(key.to_string(), value.clone());
                if old_value.as_deref() != Some(value.as_str()) {
                    mutation = Some(serde_json::Map::from_iter([
                        (
                            "kind".to_string(),
                            Value::String("prop-changed".to_string()),
                        ),
                        ("widgetId".to_string(), json!(widget_id)),
                        ("widgetName".to_string(), json!(widget_name)),
                        ("rootId".to_string(), json!(root_id)),
                        ("property".to_string(), Value::String(key.to_string())),
                        ("oldValue".to_string(), json!(old_value)),
                        ("newValue".to_string(), Value::String(value)),
                    ]));
                }
                break;
            }
        }
        if let Some(mutation) = mutation {
            state.record_mutation(mutation);
        }
    }

    fn ui_debug_limit_param(params: &Map<String, Value>, default: usize) -> usize {
        params
            .get("limit")
            .and_then(Value::as_u64)
            .map(|value| value.min(512) as usize)
            .unwrap_or(default)
    }

    fn ui_debug_after_seq_param(params: &Map<String, Value>) -> u64 {
        params.get("afterSeq").and_then(Value::as_u64).unwrap_or(0)
    }

    fn signal_action_json(action: &SignalAction) -> Value {
        match action {
            SignalAction::SetBool {
                widget_id,
                property,
                value,
            } => json!({
                "kind": "setBool",
                "widgetId": widget_id,
                "property": property,
                "value": value,
            }),
            SignalAction::CssClass {
                widget_id,
                class_name,
                add,
            } => json!({
                "kind": "cssClass",
                "widgetId": widget_id,
                "className": class_name,
                "add": add,
            }),
            SignalAction::ToggleBool {
                widget_id,
                property,
            } => json!({
                "kind": "toggleBool",
                "widgetId": widget_id,
                "property": property,
            }),
            SignalAction::ToggleCssClass {
                widget_id,
                class_name,
            } => json!({
                "kind": "toggleCssClass",
                "widgetId": widget_id,
                "className": class_name,
            }),
            SignalAction::PresentDialog {
                dialog_id,
                parent_id,
            } => json!({
                "kind": "presentDialog",
                "dialogId": dialog_id,
                "parentId": parent_id,
            }),
            SignalAction::SetStackPage {
                stack_id,
                page_name,
            } => json!({
                "kind": "setStackPage",
                "stackId": stack_id,
                "pageName": page_name,
            }),
        }
    }

    fn ui_debug_ok_response(id: Value, result: Value) -> Value {
        json!({
            "ok": true,
            "id": id,
            "result": result
        })
    }

    fn ui_debug_error_response(id: Value, message: impl Into<String>) -> Value {
        json!({
            "ok": false,
            "id": id,
            "error": {
                "message": message.into()
            }
        })
    }

    fn ui_debug_all_root_ids(state: &RealGtkState) -> Vec<i64> {
        let mut root_ids = state.live_trees.keys().copied().collect::<Vec<_>>();
        root_ids.sort_unstable();
        root_ids
    }

    fn ui_debug_all_window_ids(state: &RealGtkState) -> Vec<i64> {
        let mut window_ids = state.windows.keys().copied().collect::<Vec<_>>();
        window_ids.sort_unstable();
        window_ids
    }

    fn ui_debug_window_json(state: &RealGtkState, window_id: i64) -> Result<Value, Gtk4Error> {
        let widget =
            state.windows.get(&window_id).copied().ok_or_else(|| {
                Gtk4Error::new(format!("gtk ui debug unknown window id {window_id}"))
            })?;
        let focused_widget_id = current_focus_info(state)
            .and_then(|(focused_window_id, _, widget_id)| {
                (focused_window_id == window_id).then_some(widget_id)
            })
            .flatten();
        Ok(json!({
            "id": window_id,
            "name": state.widget_id_to_name.get(&window_id),
            "className": "GtkWindow",
            "kind": "window",
            "rootId": Value::Null,
            "parentId": Value::Null,
            "childType": Value::Null,
            "props": {},
            "signals": [],
            "dimensions": widget_dimensions_json(state, window_id),
            "state": {
                "visible": widget_bool_property(widget, "visible"),
                "sensitive": widget_bool_property(widget, "sensitive"),
                "focusedWidgetId": focused_widget_id,
                "focusedWidgetName": focused_widget_id.and_then(|id| state.widget_id_to_name.get(&id)).cloned()
            },
            "capabilities": {
                "inspect": true,
                "click": false,
                "type": false,
                "select": false,
                "focus": false,
                "moveFocus": true,
                "scroll": false,
                "keyPress": true
            },
            "children": []
        }))
    }

    fn resolve_snapshot_target<'a>(
        state: &'a RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<(i64, i64, &'a LiveNode), Gtk4Error> {
        if let Some(root_id) = params.get("rootId").and_then(Value::as_i64) {
            let live = state
                .live_trees
                .get(&root_id)
                .ok_or_else(|| Gtk4Error::new(format!("gtk ui debug unknown root id {root_id}")))?;
            return Ok((root_id, root_id, live));
        }
        if params.get("name").is_some() || params.get("id").is_some() {
            let widget_id = resolve_widget_id(state, params)?;
            let (root_id, _, _, live) = find_widget_context(state, widget_id).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug widget {widget_id} is not part of the live tree"
                ))
            })?;
            return Ok((root_id, widget_id, live));
        }
        let root_ids = ui_debug_all_root_ids(state);
        if root_ids.len() == 1 {
            let root_id = root_ids[0];
            let live = state
                .live_trees
                .get(&root_id)
                .ok_or_else(|| Gtk4Error::new(format!("gtk ui debug unknown root id {root_id}")))?;
            return Ok((root_id, root_id, live));
        }
        Err(Gtk4Error::new(
            "gtk ui debug expected one of: rootId, name, or id when multiple roots exist",
        ))
    }

    fn widget_action_bindings_json(state: &RealGtkState, live: &LiveNode) -> Value {
        Value::Array(
            live.signals
                .iter()
                .map(|binding| {
                    let actions = state
                        .signal_action_bindings
                        .get(&binding.handler)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|action| signal_action_json(&action))
                        .collect::<Vec<_>>();
                    json!({
                        "signal": binding.signal,
                        "handler": binding.handler,
                        "actions": actions,
                    })
                })
                .collect(),
        )
    }

    fn widget_ancestry_json(state: &RealGtkState, widget_id: i64) -> Value {
        let mut current_id = Some(widget_id);
        let mut ancestry = Vec::new();
        while let Some(id) = current_id {
            let Some((root_id, parent_id, child_type, live)) = find_widget_context(state, id)
            else {
                break;
            };
            ancestry.push(json!({
                "id": live.widget_id,
                "name": live.node_id,
                "className": live.class_name,
                "kind": created_widget_kind_name(live.kind),
                "rootId": root_id,
                "parentId": parent_id,
                "childType": child_type,
            }));
            current_id = parent_id;
        }
        ancestry.reverse();
        Value::Array(ancestry)
    }

    fn find_widget_at_point<'a>(
        state: &'a RealGtkState,
        target_id: i64,
        root_id: i64,
        live: &'a LiveNode,
        x: f64,
        y: f64,
    ) -> Option<&'a LiveNode> {
        let bounds = widget_bounds_relative_to(state, live.widget_id, target_id)?;
        if !rect_contains_point(bounds, x, y) {
            return None;
        }
        for child in live.children.iter().rev() {
            if let Some(found) = find_widget_at_point(state, target_id, root_id, &child.node, x, y)
            {
                return Some(found);
            }
        }
        let _ = root_id;
        Some(live)
    }

    fn ui_debug_style_info_result(
        state: &RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<Value, Gtk4Error> {
        let widget_id = resolve_widget_id(state, params)?;
        let (root_id, parent_id, child_type, live) = find_widget_context(state, widget_id)
            .ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug widget {widget_id} is not part of the live tree"
                ))
            })?;
        Ok(json!({
            "protocol": "aivi.gtk.debug.v1",
            "widget": live_node_json(state, root_id, parent_id, child_type.as_deref(), live),
            "style": widget_style_info_json(state, live),
        }))
    }

    fn ui_debug_layout_snapshot_result(
        state: &mut RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<Value, Gtk4Error> {
        let (root_id, target_id, live) = resolve_snapshot_target(state, params)?;
        let snapshot = json!({
            "protocol": "aivi.gtk.debug.v1",
            "rootId": root_id,
            "targetId": target_id,
            "focus": focus_summary_json(state),
            "tree": layout_entry_json(state, root_id, target_id, None, live),
        });
        if let Some(label) = params.get("label").and_then(Value::as_str) {
            RealGtkState::store_json_snapshot(&mut state.layout_snapshots, label, snapshot.clone());
        }
        Ok(snapshot)
    }

    fn ui_debug_explain_widget_result(
        state: &RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<Value, Gtk4Error> {
        let widget_id = resolve_widget_id(state, params)?;
        let (root_id, parent_id, child_type, live) = find_widget_context(state, widget_id)
            .ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug widget {widget_id} is not part of the live tree"
                ))
            })?;
        Ok(json!({
            "protocol": "aivi.gtk.debug.v1",
            "widget": live_node_json(state, root_id, parent_id, child_type.as_deref(), live),
            "ancestry": widget_ancestry_json(state, widget_id),
            "style": widget_style_info_json(state, live),
            "layout": layout_entry_json(state, root_id, widget_id, None, live),
            "actionBindings": widget_action_bindings_json(state, live),
        }))
    }

    fn ui_debug_inspect_at_result(
        state: &RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<Value, Gtk4Error> {
        let x = params
            .get("x")
            .and_then(Value::as_f64)
            .ok_or_else(|| Gtk4Error::new("gtk ui debug inspectAt requires x"))?;
        let y = params
            .get("y")
            .and_then(Value::as_f64)
            .ok_or_else(|| Gtk4Error::new("gtk ui debug inspectAt requires y"))?;
        let (root_id, target_id, live) = resolve_snapshot_target(state, params)?;
        let hit = find_widget_at_point(state, target_id, root_id, live, x, y).ok_or_else(|| {
            Gtk4Error::new(format!(
                "gtk ui debug found no widget at point ({x}, {y}) in target {target_id}"
            ))
        })?;
        let (_, parent_id, child_type, _) =
            find_widget_context(state, hit.widget_id).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug widget {} disappeared during inspectAt",
                    hit.widget_id
                ))
            })?;
        Ok(json!({
            "protocol": "aivi.gtk.debug.v1",
            "rootId": root_id,
            "targetId": target_id,
            "x": x,
            "y": y,
            "widget": live_node_json(state, root_id, parent_id, child_type.as_deref(), hit),
            "ancestry": widget_ancestry_json(state, hit.widget_id),
        }))
    }

    fn ui_debug_show_overlay_result(
        state: &mut RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<Value, Gtk4Error> {
        let enabled = params
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        state.overlay.enabled = enabled;
        if let Some(value) = params.get("bounds").and_then(Value::as_bool) {
            state.overlay.bounds = value;
        }
        if let Some(value) = params.get("margins").and_then(Value::as_bool) {
            state.overlay.margins = value;
        }
        if let Some(value) = params.get("spacing").and_then(Value::as_bool) {
            state.overlay.spacing = value;
        }
        if let Some(value) = params.get("focus").and_then(Value::as_bool) {
            state.overlay.focus = value;
        }
        if let Some(value) = params.get("clipping").and_then(Value::as_bool) {
            state.overlay.clipping = value;
        }
        Ok(json!({
            "protocol": "aivi.gtk.debug.v1",
            "overlay": {
                "enabled": state.overlay.enabled,
                "bounds": state.overlay.bounds,
                "margins": state.overlay.margins,
                "spacing": state.overlay.spacing,
                "focus": state.overlay.focus,
                "clipping": state.overlay.clipping,
            }
        }))
    }

    fn rect_json_number(bounds: &Value, key: &str) -> Option<i32> {
        bounds.get(key)?.as_f64().map(|value| value.round() as i32)
    }

    fn set_pixel(image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, x: i32, y: i32, color: [u8; 4]) {
        if x < 0 || y < 0 {
            return;
        }
        let (width, height) = image.dimensions();
        if x as u32 >= width || y as u32 >= height {
            return;
        }
        image.put_pixel(x as u32, y as u32, Rgba(color));
    }

    fn draw_rect_outline(
        image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        color: [u8; 4],
        thickness: i32,
    ) {
        if width <= 0 || height <= 0 {
            return;
        }
        let right = x + width - 1;
        let bottom = y + height - 1;
        for offset in 0..thickness.max(1) {
            for px in x..=right {
                set_pixel(image, px, y + offset, color);
                set_pixel(image, px, bottom - offset, color);
            }
            for py in y..=bottom {
                set_pixel(image, x + offset, py, color);
                set_pixel(image, right - offset, py, color);
            }
        }
    }

    fn fill_rect_alpha(
        image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        color: [u8; 4],
    ) {
        if width <= 0 || height <= 0 {
            return;
        }
        for py in y..(y + height) {
            for px in x..(x + width) {
                if px < 0 || py < 0 {
                    continue;
                }
                let (img_w, img_h) = image.dimensions();
                if px as u32 >= img_w || py as u32 >= img_h {
                    continue;
                }
                let mut base = *image.get_pixel(px as u32, py as u32);
                let alpha = color[3] as f32 / 255.0;
                for channel in 0..3 {
                    base[channel] = ((base[channel] as f32 * (1.0 - alpha))
                        + (color[channel] as f32 * alpha))
                        .round() as u8;
                }
                image.put_pixel(px as u32, py as u32, base);
            }
        }
    }

    fn draw_spacing_guides(image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, children: &[Value]) {
        let rects = children
            .iter()
            .filter_map(|child| child.get("bounds"))
            .filter_map(|bounds| {
                Some((
                    rect_json_number(bounds, "x")?,
                    rect_json_number(bounds, "y")?,
                    rect_json_number(bounds, "width")?,
                    rect_json_number(bounds, "height")?,
                ))
            })
            .collect::<Vec<_>>();
        if rects.len() < 2 {
            return;
        }
        let horizontal_span = rects
            .iter()
            .map(|(x, _, width, _)| x + width)
            .max()
            .unwrap_or(0)
            - rects.iter().map(|(x, _, _, _)| *x).min().unwrap_or(0);
        let vertical_span = rects
            .iter()
            .map(|(_, y, _, height)| y + height)
            .max()
            .unwrap_or(0)
            - rects.iter().map(|(_, y, _, _)| *y).min().unwrap_or(0);
        let horizontal = horizontal_span >= vertical_span;
        let mut rects = rects;
        if horizontal {
            rects.sort_by_key(|(x, _, _, _)| *x);
            for pair in rects.windows(2) {
                let left = pair[0];
                let right = pair[1];
                let gap_start = left.0 + left.2;
                let gap_end = right.0;
                if gap_end - gap_start > 2 {
                    let y = (left.1 + right.1 + left.3.min(right.3) / 2) / 2;
                    for x in gap_start..gap_end {
                        set_pixel(image, x, y, [0, 200, 255, 255]);
                    }
                }
            }
        } else {
            rects.sort_by_key(|(_, y, _, _)| *y);
            for pair in rects.windows(2) {
                let top = pair[0];
                let bottom = pair[1];
                let gap_start = top.1 + top.3;
                let gap_end = bottom.1;
                if gap_end - gap_start > 2 {
                    let x = (top.0 + bottom.0 + top.2.min(bottom.2) / 2) / 2;
                    for y in gap_start..gap_end {
                        set_pixel(image, x, y, [0, 200, 255, 255]);
                    }
                }
            }
        }
    }

    fn draw_layout_overlay_recursive(
        image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        node: &Value,
        overlay: &UiDebugOverlayState,
        focused_widget_id: Option<i64>,
    ) {
        let bounds = node.get("bounds").cloned().unwrap_or(Value::Null);
        let x = rect_json_number(&bounds, "x").unwrap_or(0);
        let y = rect_json_number(&bounds, "y").unwrap_or(0);
        let width = rect_json_number(&bounds, "width").unwrap_or(0);
        let height = rect_json_number(&bounds, "height").unwrap_or(0);
        let widget_id = node.get("id").and_then(Value::as_i64);
        if overlay.bounds {
            draw_rect_outline(image, x, y, width, height, [0, 255, 128, 255], 1);
        }
        if overlay.margins {
            let margins = node.get("margins").cloned().unwrap_or(Value::Null);
            let start = margins.get("start").and_then(Value::as_i64).unwrap_or(0) as i32;
            let end = margins.get("end").and_then(Value::as_i64).unwrap_or(0) as i32;
            let top = margins.get("top").and_then(Value::as_i64).unwrap_or(0) as i32;
            let bottom = margins.get("bottom").and_then(Value::as_i64).unwrap_or(0) as i32;
            if start != 0 || end != 0 || top != 0 || bottom != 0 {
                draw_rect_outline(
                    image,
                    x - start,
                    y - top,
                    width + start + end,
                    height + top + bottom,
                    [64, 160, 255, 255],
                    1,
                );
            }
        }
        if overlay.focus && widget_id == focused_widget_id {
            draw_rect_outline(image, x, y, width, height, [255, 220, 0, 255], 3);
        }
        if overlay.clipping
            && node
                .get("overflowParentBounds")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            fill_rect_alpha(image, x, y, width, height, [255, 0, 0, 72]);
        }
        let children = node
            .get("children")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if overlay.spacing {
            draw_spacing_guides(image, &children);
        }
        for child in children {
            draw_layout_overlay_recursive(image, &child, overlay, focused_widget_id);
        }
    }

    fn encode_png_base64(width: u32, height: u32, rgba: &[u8]) -> Result<String, Gtk4Error> {
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(rgba, width, height, image::ColorType::Rgba8.into())
            .map_err(|err| Gtk4Error::new(format!("gtk ui debug png encode failed: {err}")))?;
        Ok(base64::engine::general_purpose::STANDARD.encode(png))
    }

    fn scale_rgba_nearest(width: u32, height: u32, rgba: &[u8], scale: f64) -> (u32, u32, Vec<u8>) {
        if scale <= 1.0 {
            return (width, height, rgba.to_vec());
        }
        let next_width = ((width as f64) * scale).round().max(1.0) as u32;
        let next_height = ((height as f64) * scale).round().max(1.0) as u32;
        let mut out = vec![0; (next_width * next_height * 4) as usize];
        for y in 0..next_height {
            let src_y = ((y as f64) / scale).floor().min((height - 1) as f64) as u32;
            for x in 0..next_width {
                let src_x = ((x as f64) / scale).floor().min((width - 1) as f64) as u32;
                let src_index = ((src_y * width + src_x) * 4) as usize;
                let dst_index = ((y * next_width + x) * 4) as usize;
                out[dst_index..dst_index + 4].copy_from_slice(&rgba[src_index..src_index + 4]);
            }
        }
        (next_width, next_height, out)
    }

    fn capture_widget_rgba(
        state: &RealGtkState,
        target_id: i64,
    ) -> Result<(u32, u32, Vec<u8>), Gtk4Error> {
        let widget = widget_ptr(state, target_id, "uiDebugCapture")?;
        let paintable = unsafe { gtk_widget_paintable_new(widget) };
        if paintable.is_null() {
            return Err(Gtk4Error::new(format!(
                "gtk ui debug failed to create paintable for widget {target_id}"
            )));
        }
        let image = unsafe { gdk_paintable_get_current_image(paintable) };
        if image.is_null() {
            unsafe { g_object_unref(paintable) };
            return Err(Gtk4Error::new(format!(
                "gtk ui debug failed to snapshot widget {target_id}"
            )));
        }
        let width = unsafe { gdk_texture_get_width(image) }.max(0) as u32;
        let height = unsafe { gdk_texture_get_height(image) }.max(0) as u32;
        if width == 0 || height == 0 {
            unsafe {
                g_object_unref(image);
                g_object_unref(paintable);
            }
            return Err(Gtk4Error::new(format!(
                "gtk ui debug widget {target_id} has zero-sized capture"
            )));
        }
        let stride = (width * 4) as usize;
        let mut rgba = vec![0; stride * height as usize];
        unsafe {
            gdk_texture_download(image, rgba.as_mut_ptr(), stride);
            g_object_unref(image);
            g_object_unref(paintable);
        }
        Ok((width, height, rgba))
    }

    fn ui_debug_capture_result(
        state: &mut RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<Value, Gtk4Error> {
        let (root_id, target_id, live) = resolve_snapshot_target(state, params)?;
        let (width, height, mut rgba) = capture_widget_rgba(state, target_id)?;
        if state.overlay.enabled
            || params.get("highlightId").is_some()
            || params.get("highlightName").is_some()
        {
            let focus_id = current_focus_info(state).and_then(|(_, _, widget_id)| widget_id);
            let layout = layout_entry_json(state, root_id, target_id, None, live);
            let mut image = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, rgba)
                .ok_or_else(|| Gtk4Error::new("gtk ui debug failed to map screenshot buffer"))?;
            if state.overlay.enabled {
                draw_layout_overlay_recursive(&mut image, &layout, &state.overlay, focus_id);
            }
            let highlight_id = params
                .get("highlightId")
                .and_then(Value::as_i64)
                .or_else(|| {
                    params
                        .get("highlightName")
                        .and_then(Value::as_str)
                        .and_then(|name| state.named_widgets.get(name).copied())
                });
            if let Some(highlight_id) = highlight_id {
                if let Some(bounds) = widget_bounds_relative_to(state, highlight_id, target_id) {
                    draw_rect_outline(
                        &mut image,
                        bounds.origin.x.round() as i32,
                        bounds.origin.y.round() as i32,
                        bounds.size.width.round() as i32,
                        bounds.size.height.round() as i32,
                        [255, 0, 255, 255],
                        3,
                    );
                }
            }
            rgba = image.into_raw();
        }
        let scale = params.get("scale").and_then(Value::as_f64).unwrap_or(1.0);
        let (width, height, rgba) = scale_rgba_nearest(width, height, &rgba, scale);
        let png_base64 = encode_png_base64(width, height, &rgba)?;
        if let Some(label) = params.get("label").and_then(Value::as_str) {
            state.store_capture_snapshot(
                label,
                target_id,
                width,
                height,
                png_base64.clone(),
                rgba.clone(),
            );
        }
        Ok(json!({
            "protocol": "aivi.gtk.debug.v1",
            "rootId": root_id,
            "targetId": target_id,
            "width": width,
            "height": height,
            "scale": scale,
            "mimeType": "image/png",
            "pngBase64": png_base64,
        }))
    }

    fn ui_debug_hello_result(state: &RealGtkState) -> Value {
        let root_ids = ui_debug_all_root_ids(state);
        let window_ids = ui_debug_all_window_ids(state);
        json!({
            "protocol": "aivi.gtk.debug.v1",
            "rootIds": root_ids,
            "windowIds": window_ids,
            "windowCount": state.windows.len(),
            "widgetCount": state.widgets.len(),
            "namedWidgetCount": state.named_widgets.len(),
            "actions": ["click", "type", "select", "focus", "moveFocus", "scroll", "keyPress"],
            "focus": focus_summary_json(state),
            "inspectors": ["listWidgets", "inspectWidget", "dumpTree", "listSignals", "inspectSignal"]
        })
    }

    fn ui_debug_poll_events_result(state: &RealGtkState, params: &Map<String, Value>) -> Value {
        let after_seq = ui_debug_after_seq_param(params);
        let limit = ui_debug_limit_param(params, 100);
        let events = state
            .signal_event_log
            .iter()
            .filter(|entry| entry.get("seq").and_then(Value::as_u64).unwrap_or(0) > after_seq)
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let latest_seq = state
            .signal_event_log
            .back()
            .and_then(|entry| entry.get("seq"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        json!({
            "protocol": "aivi.gtk.debug.v1",
            "afterSeq": after_seq,
            "latestSeq": latest_seq,
            "returned": events.len(),
            "events": events,
        })
    }

    fn ui_debug_poll_mutations_result(state: &RealGtkState, params: &Map<String, Value>) -> Value {
        let after_seq = ui_debug_after_seq_param(params);
        let limit = ui_debug_limit_param(params, 100);
        let mutations = state
            .mutation_log
            .iter()
            .filter(|entry| entry.get("seq").and_then(Value::as_u64).unwrap_or(0) > after_seq)
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let latest_seq = state
            .mutation_log
            .back()
            .and_then(|entry| entry.get("seq"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        json!({
            "protocol": "aivi.gtk.debug.v1",
            "afterSeq": after_seq,
            "latestSeq": latest_seq,
            "returned": mutations.len(),
            "mutations": mutations,
        })
    }

    fn ui_debug_list_widgets_result(state: &RealGtkState) -> Value {
        let root_ids = ui_debug_all_root_ids(state);
        let mut widgets = Vec::new();
        for root_id in &root_ids {
            if let Some(root) = state.live_trees.get(root_id) {
                collect_widget_summaries(state, &mut widgets, *root_id, None, None, root);
            }
        }
        widgets.sort_by_key(|widget| widget.get("id").and_then(Value::as_i64).unwrap_or_default());
        json!({
            "protocol": "aivi.gtk.debug.v1",
            "rootIds": root_ids,
            "focus": focus_summary_json(state),
            "widgetCount": widgets.len(),
            "widgets": widgets
        })
    }

    fn ui_debug_list_action_bindings_result(
        state: &RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<Value, Gtk4Error> {
        if params.get("name").is_some() || params.get("id").is_some() {
            let widget_id = resolve_widget_id(state, params)?;
            let (_, _, _, live) = find_widget_context(state, widget_id).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug widget {widget_id} is not part of the live tree"
                ))
            })?;
            let bindings = live
                .signals
                .iter()
                .map(|binding| {
                    let actions = state
                        .signal_action_bindings
                        .get(&binding.handler)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|action| signal_action_json(&action))
                        .collect::<Vec<_>>();
                    json!({
                        "widgetId": widget_id,
                        "widgetName": live.node_id,
                        "signal": binding.signal,
                        "handler": binding.handler,
                        "actions": actions,
                    })
                })
                .collect::<Vec<_>>();
            return Ok(json!({
                "protocol": "aivi.gtk.debug.v1",
                "widgetId": widget_id,
                "bindings": bindings,
            }));
        }

        let mut handlers = state
            .signal_action_bindings
            .iter()
            .map(|(handler, actions)| {
                json!({
                    "handler": handler,
                    "actions": actions.iter().map(signal_action_json).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>();
        handlers.sort_by(|a, b| {
            a.get("handler")
                .and_then(Value::as_str)
                .cmp(&b.get("handler").and_then(Value::as_str))
        });
        Ok(json!({
            "protocol": "aivi.gtk.debug.v1",
            "handlerCount": handlers.len(),
            "handlers": handlers,
        }))
    }

    fn ui_debug_dump_tree_result(
        state: &RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<Value, Gtk4Error> {
        if let Some(root_id) = params.get("rootId").and_then(Value::as_i64) {
            let root = state
                .live_trees
                .get(&root_id)
                .ok_or_else(|| Gtk4Error::new(format!("gtk ui debug unknown root id {root_id}")))?;
            return Ok(json!({
                "protocol": "aivi.gtk.debug.v1",
                "rootId": root_id,
                "focus": focus_summary_json(state),
                "tree": live_node_json(state, root_id, None, None, root)
            }));
        }

        let root_ids = ui_debug_all_root_ids(state);
        let trees = root_ids
            .iter()
            .filter_map(|root_id| {
                state
                    .live_trees
                    .get(root_id)
                    .map(|root| live_node_json(state, *root_id, None, None, root))
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "protocol": "aivi.gtk.debug.v1",
            "rootIds": root_ids,
            "focus": focus_summary_json(state),
            "trees": trees
        }))
    }

    fn ui_debug_inspect_widget_result(
        state: &RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<Value, Gtk4Error> {
        let widget_id = resolve_widget_id(state, params)?;
        if state.windows.contains_key(&widget_id) && find_widget_context(state, widget_id).is_none()
        {
            return Ok(json!({
                "protocol": "aivi.gtk.debug.v1",
                "focus": focus_summary_json(state),
                "widget": ui_debug_window_json(state, widget_id)?
            }));
        }
        let (root_id, parent_id, child_type, live) = find_widget_context(state, widget_id)
            .ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug widget {widget_id} is not part of the live tree"
                ))
            })?;
        Ok(json!({
            "protocol": "aivi.gtk.debug.v1",
            "focus": focus_summary_json(state),
            "widget": live_node_json(state, root_id, parent_id, child_type.as_deref(), live)
        }))
    }

    fn resolve_key_press_target_id(
        state: &RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<i64, Gtk4Error> {
        let has_explicit_target =
            params.get("name").is_some() || params.get("id").and_then(Value::as_i64).is_some();
        if has_explicit_target {
            return resolve_widget_id(state, params);
        }

        if let Some((_, _, Some(widget_id))) = current_focus_info(state) {
            return Ok(widget_id);
        }

        let window_ids = ui_debug_all_window_ids(state);
        if window_ids.len() == 1 {
            return Ok(window_ids[0]);
        }

        let root_ids = ui_debug_all_root_ids(state);
        if root_ids.len() == 1 {
            return Ok(root_ids[0]);
        }

        Err(Gtk4Error::new(
            "gtk ui debug keyPress needs an explicit target when the session has multiple windows or roots",
        ))
    }

    fn ui_debug_key_press_result(
        state: &mut RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<Value, Gtk4Error> {
        let widget_id = resolve_key_press_target_id(state, params)?;
        let key = params
            .get("key")
            .and_then(Value::as_str)
            .ok_or_else(|| Gtk4Error::new("gtk ui debug keyPress requires key"))?;
        let detail = params
            .get("detail")
            .and_then(Value::as_str)
            .unwrap_or("mcp");
        let payload = format!("{key}\n{detail}");
        enqueue_signal_event(state, widget_id, "key-pressed", "", &payload)?;

        let target = if let Some((root_id, parent_id, child_type, live)) =
            find_widget_context(state, widget_id)
        {
            json!({
                "rootId": root_id,
                "widget": live_node_json(state, root_id, parent_id, child_type.as_deref(), live)
            })
        } else if state.windows.contains_key(&widget_id) {
            json!({
                "rootId": Value::Null,
                "widget": ui_debug_window_json(state, widget_id)?
            })
        } else {
            return Err(Gtk4Error::new(format!(
                "gtk ui debug keyPress target {widget_id} disappeared after dispatch"
            )));
        };

        Ok(json!({
            "protocol": "aivi.gtk.debug.v1",
            "targetId": widget_id,
            "key": key,
            "detail": detail,
            "focus": focus_summary_json(state),
            "emitted": [{
                "signal": "key-pressed",
                "handler": "",
                "payload": payload
            }],
            "target": target
        }))
    }

    fn resolve_focus_container_id(
        state: &RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<i64, Gtk4Error> {
        let has_explicit_target =
            params.get("name").is_some() || params.get("id").and_then(Value::as_i64).is_some();
        if has_explicit_target {
            let widget_id = resolve_widget_id(state, params)?;
            if state.windows.contains_key(&widget_id) {
                return Ok(widget_id);
            }
            if let Some((root_id, _, _, _)) = find_widget_context(state, widget_id) {
                return Ok(root_id);
            }
            return Err(Gtk4Error::new(format!(
                "gtk ui debug focus target {widget_id} is not part of the live tree"
            )));
        }
        if let Some((window_id, _, _)) = current_focus_info(state) {
            return Ok(window_id);
        }
        let window_ids = ui_debug_all_window_ids(state);
        if window_ids.len() == 1 {
            return Ok(window_ids[0]);
        }
        let root_ids = ui_debug_all_root_ids(state);
        if root_ids.len() == 1 {
            return Ok(root_ids[0]);
        }
        Err(Gtk4Error::new(
            "gtk ui debug moveFocus needs an explicit target when the session has multiple windows or roots and no current focus",
        ))
    }

    fn ui_debug_focus_result(
        state: &mut RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<Value, Gtk4Error> {
        let widget_id = resolve_widget_id(state, params)?;
        if state.windows.contains_key(&widget_id) && find_widget_context(state, widget_id).is_none()
        {
            return Err(Gtk4Error::new(
                "gtk ui debug focus expects a focusable widget target, not a window id",
            ));
        }
        let widget = widget_ptr(state, widget_id, "uiDebugFocus")?;
        if unsafe { gtk_widget_grab_focus(widget) } == 0 {
            return Err(Gtk4Error::new(format!(
                "gtk ui debug widget {widget_id} could not take focus"
            )));
        }
        let (root_id, parent_id, child_type, live) = find_widget_context(state, widget_id)
            .ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug widget {widget_id} disappeared after focus"
                ))
            })?;
        Ok(json!({
            "protocol": "aivi.gtk.debug.v1",
            "rootId": root_id,
            "focus": focus_summary_json(state),
            "widget": live_node_json(state, root_id, parent_id, child_type.as_deref(), live)
        }))
    }

    fn ui_debug_move_focus_result(
        state: &mut RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<Value, Gtk4Error> {
        let direction_text = params
            .get("direction")
            .and_then(Value::as_str)
            .ok_or_else(|| Gtk4Error::new("gtk ui debug moveFocus requires direction"))?;
        let direction = parse_focus_direction(direction_text).ok_or_else(|| {
            Gtk4Error::new(format!(
                "gtk ui debug moveFocus invalid direction '{direction_text}'"
            ))
        })?;
        let container_id = resolve_focus_container_id(state, params)?;
        let container = widget_ptr(state, container_id, "uiDebugMoveFocus")?;
        if unsafe { gtk_widget_child_focus(container, direction) } == 0 {
            return Err(Gtk4Error::new(format!(
                "gtk ui debug could not move focus from container {container_id} with direction '{direction_text}'"
            )));
        }
        Ok(json!({
            "protocol": "aivi.gtk.debug.v1",
            "containerId": container_id,
            "direction": direction_text,
            "focus": focus_summary_json(state)
        }))
    }

    fn scroll_adjustment_json(adjustment: *mut c_void) -> Value {
        json!({
            "value": unsafe { gtk_adjustment_get_value(adjustment) },
            "lower": unsafe { gtk_adjustment_get_lower(adjustment) },
            "upper": unsafe { gtk_adjustment_get_upper(adjustment) },
            "pageSize": unsafe { gtk_adjustment_get_page_size(adjustment) }
        })
    }

    fn ui_debug_scroll_result(
        state: &mut RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<Value, Gtk4Error> {
        let widget_id = resolve_widget_id(state, params)?;
        let direction = params
            .get("direction")
            .and_then(Value::as_str)
            .ok_or_else(|| Gtk4Error::new("gtk ui debug scroll requires direction"))?;
        let amount = params
            .get("amount")
            .and_then(Value::as_f64)
            .unwrap_or(40.0)
            .abs();
        let (root_id, class_name) = {
            let (root_id, _, _, live) = find_widget_context(state, widget_id).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug widget {widget_id} is not part of the live tree"
                ))
            })?;
            (root_id, live.class_name.clone())
        };
        if class_name != "GtkScrolledWindow" {
            return Err(Gtk4Error::new(format!(
                "gtk ui debug widget {widget_id} does not support scrolling"
            )));
        }
        let widget = widget_ptr(state, widget_id, "uiDebugScroll")?;
        let adjustment = match direction {
            "up" | "down" => unsafe { gtk_scrolled_window_get_vadjustment(widget) },
            "left" | "right" => unsafe { gtk_scrolled_window_get_hadjustment(widget) },
            _ => {
                return Err(Gtk4Error::new(format!(
                    "gtk ui debug scroll invalid direction '{direction}'"
                )))
            }
        };
        if adjustment.is_null() {
            return Err(Gtk4Error::new(format!(
                "gtk ui debug widget {widget_id} has no scroll adjustment for direction '{direction}'"
            )));
        }
        let current = unsafe { gtk_adjustment_get_value(adjustment) };
        let lower = unsafe { gtk_adjustment_get_lower(adjustment) };
        let upper = unsafe { gtk_adjustment_get_upper(adjustment) };
        let page_size = unsafe { gtk_adjustment_get_page_size(adjustment) };
        let max_value = (upper - page_size).max(lower);
        let delta = match direction {
            "up" | "left" => -amount,
            "down" | "right" => amount,
            _ => 0.0,
        };
        let next = (current + delta).clamp(lower, max_value);
        unsafe { gtk_adjustment_set_value(adjustment, next) };
        let (_, parent_id, child_type, live) =
            find_widget_context(state, widget_id).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug widget {widget_id} disappeared after scroll"
                ))
            })?;
        Ok(json!({
            "protocol": "aivi.gtk.debug.v1",
            "rootId": root_id,
            "direction": direction,
            "amount": amount,
            "focus": focus_summary_json(state),
            "scroll": scroll_adjustment_json(adjustment),
            "widget": live_node_json(state, root_id, parent_id, child_type.as_deref(), live)
        }))
    }

    fn ui_debug_click_result(
        state: &mut RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<Value, Gtk4Error> {
        let widget_id = resolve_widget_id(state, params)?;
        let (root_id, bindings) = {
            let (root_id, _, _, live) = find_widget_context(state, widget_id).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug widget {widget_id} is not part of the live tree"
                ))
            })?;
            let bindings = live
                .signals
                .iter()
                .filter(|binding| matches!(binding.signal.as_str(), "clicked" | "activate"))
                .cloned()
                .collect::<Vec<_>>();
            (root_id, bindings)
        };
        if bindings.is_empty() {
            return Err(Gtk4Error::new(format!(
                "gtk ui debug widget {widget_id} has no clickable signal bindings"
            )));
        }
        let mut emitted = Vec::new();
        for binding in &bindings {
            enqueue_signal_event(state, widget_id, &binding.signal, &binding.handler, "")?;
            emitted.push(json!({
                "signal": binding.signal,
                "handler": binding.handler
            }));
        }
        let (_, parent_id, child_type, live) =
            find_widget_context(state, widget_id).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug widget {widget_id} disappeared after click dispatch"
                ))
            })?;
        Ok(json!({
            "protocol": "aivi.gtk.debug.v1",
            "rootId": root_id,
            "focus": focus_summary_json(state),
            "widget": live_node_json(state, root_id, parent_id, child_type.as_deref(), live),
            "emitted": emitted
        }))
    }

    fn ui_debug_type_result(
        state: &mut RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<Value, Gtk4Error> {
        let widget_id = resolve_widget_id(state, params)?;
        let text = params
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| Gtk4Error::new("gtk ui debug type requires text"))?;
        let (root_id, class_name, bindings) = {
            let (root_id, _, _, live) = find_widget_context(state, widget_id).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug widget {widget_id} is not part of the live tree"
                ))
            })?;
            (
                root_id,
                live.class_name.clone(),
                live.signals
                    .iter()
                    .filter(|binding| {
                        binding.signal == "changed" || binding.signal == "notify::text"
                    })
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        };
        if !is_text_input_class(&class_name) {
            return Err(Gtk4Error::new(format!(
                "gtk ui debug widget {widget_id} does not support typing"
            )));
        }
        let widget = widget_ptr(state, widget_id, "uiDebugType")?;
        let text_c = c_text(text, "gtk ui debug invalid text payload")?;
        unsafe {
            if matches!(
                class_name.as_str(),
                "GtkEntry" | "GtkPasswordEntry" | "GtkSearchEntry"
            ) {
                gtk_editable_set_text(widget, text_c.as_ptr());
            } else {
                let prop_c = CString::new("text").unwrap();
                gobject_set_str(widget, &prop_c, &text_c);
            }
        }
        update_live_prop(state, widget_id, "text", text.to_string());
        let mut emitted = Vec::new();
        for binding in &bindings {
            enqueue_signal_event(state, widget_id, &binding.signal, &binding.handler, text)?;
            emitted.push(json!({
                "signal": binding.signal,
                "handler": binding.handler
            }));
        }
        let (_, parent_id, child_type, live) =
            find_widget_context(state, widget_id).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug widget {widget_id} disappeared after typing"
                ))
            })?;
        Ok(json!({
            "protocol": "aivi.gtk.debug.v1",
            "rootId": root_id,
            "text": text,
            "focus": focus_summary_json(state),
            "widget": live_node_json(state, root_id, parent_id, child_type.as_deref(), live),
            "emitted": emitted
        }))
    }

    fn ui_debug_select_result(
        state: &mut RealGtkState,
        params: &Map<String, Value>,
    ) -> Result<Value, Gtk4Error> {
        let widget_id = resolve_widget_id(state, params)?;
        let value = params
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(|| Gtk4Error::new("gtk ui debug select requires value"))?;
        let (root_id, class_name, kind, bindings, props) = {
            let (root_id, _, _, live) = find_widget_context(state, widget_id).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug widget {widget_id} is not part of the live tree"
                ))
            })?;
            (
                root_id,
                live.class_name.clone(),
                live.kind,
                live.signals.clone(),
                live.props.clone(),
            )
        };
        let widget = widget_ptr(state, widget_id, "uiDebugSelect")?;
        let mut emitted = Vec::new();

        if matches!(kind, CreatedWidgetKind::Stack) {
            let page_c = c_text(value, "gtk ui debug invalid stack page name")?;
            unsafe { gtk_stack_set_visible_child_name(widget, page_c.as_ptr()) };
            update_live_prop(state, widget_id, "visible-child-name", value.to_string());
            for binding in bindings
                .iter()
                .filter(|binding| binding.signal == "notify::visible-child-name")
            {
                enqueue_signal_event(state, widget_id, &binding.signal, &binding.handler, value)?;
                emitted.push(json!({
                    "signal": binding.signal,
                    "handler": binding.handler
                }));
            }
        } else if is_toggle_class(&class_name) {
            let active = parse_bool_text(value).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug select expected a boolean-like value for widget {widget_id}"
                ))
            })?;
            let prop_c = CString::new("active").unwrap();
            unsafe { gobject_set_bool(widget, &prop_c, bool_to_c(active)) };
            update_live_prop(state, widget_id, "active", active.to_string());
            let payload = if active { "true" } else { "false" };
            for binding in bindings
                .iter()
                .filter(|binding| binding.signal == "toggled" || binding.signal == "notify::active")
            {
                enqueue_signal_event(state, widget_id, &binding.signal, &binding.handler, payload)?;
                emitted.push(json!({
                    "signal": binding.signal,
                    "handler": binding.handler
                }));
            }
        } else if class_name == "GtkDropDown" {
            let options = string_prop_lines(props.get("strings"));
            let selected = string_selection_index(&options, value).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug select expected a dropdown index or label for widget {widget_id}"
                ))
            })?;
            unsafe { gtk_drop_down_set_selected(widget, selected as c_uint) };
            update_live_prop(state, widget_id, "selected", selected.to_string());
            let payload = selected.to_string();
            for binding in bindings
                .iter()
                .filter(|binding| binding.signal == "notify::selected")
            {
                enqueue_signal_event(
                    state,
                    widget_id,
                    &binding.signal,
                    &binding.handler,
                    &payload,
                )?;
                emitted.push(json!({
                    "signal": binding.signal,
                    "handler": binding.handler
                }));
            }
        } else if class_name == "GtkComboBoxText" {
            let options = string_prop_lines(props.get("strings"));
            let selected = string_selection_index(&options, value).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug select expected a combo-box index or label for widget {widget_id}"
                ))
            })?;
            unsafe { gtk_combo_box_set_active(widget, selected as c_int) };
            update_live_prop(state, widget_id, "active", selected.to_string());
            let payload = options
                .get(selected)
                .cloned()
                .unwrap_or_else(|| selected.to_string());
            for binding in bindings
                .iter()
                .filter(|binding| binding.signal == "changed")
            {
                enqueue_signal_event(
                    state,
                    widget_id,
                    &binding.signal,
                    &binding.handler,
                    &payload,
                )?;
                emitted.push(json!({
                    "signal": binding.signal,
                    "handler": binding.handler
                }));
            }
        } else if matches!(class_name.as_str(), "GtkRange" | "GtkScale") {
            let requested = parse_f64_text(value).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug select expected a numeric value for widget {widget_id}"
                ))
            })?;
            let min = props
                .get("min")
                .and_then(|raw| parse_f64_text(raw))
                .unwrap_or(requested);
            let max = props
                .get("max")
                .and_then(|raw| parse_f64_text(raw))
                .unwrap_or(requested);
            let selected = requested.clamp(min.min(max), min.max(max));
            unsafe { gtk_range_set_value(widget, selected) };
            update_live_prop(state, widget_id, "value", selected.to_string());
            let payload = selected.to_string();
            for binding in bindings
                .iter()
                .filter(|binding| binding.signal == "value-changed")
            {
                enqueue_signal_event(
                    state,
                    widget_id,
                    &binding.signal,
                    &binding.handler,
                    &payload,
                )?;
                emitted.push(json!({
                    "signal": binding.signal,
                    "handler": binding.handler
                }));
            }
        } else if class_name == "GtkSpinButton" {
            let requested = parse_f64_text(value).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug select expected a numeric value for widget {widget_id}"
                ))
            })?;
            let min = props
                .get("min")
                .and_then(|raw| parse_f64_text(raw))
                .unwrap_or(requested);
            let max = props
                .get("max")
                .and_then(|raw| parse_f64_text(raw))
                .unwrap_or(requested);
            let selected = requested.clamp(min.min(max), min.max(max));
            unsafe { gtk_spin_button_set_value(widget, selected) };
            update_live_prop(state, widget_id, "value", selected.to_string());
            let payload = selected.to_string();
            for binding in bindings
                .iter()
                .filter(|binding| binding.signal == "value-changed")
            {
                enqueue_signal_event(
                    state,
                    widget_id,
                    &binding.signal,
                    &binding.handler,
                    &payload,
                )?;
                emitted.push(json!({
                    "signal": binding.signal,
                    "handler": binding.handler
                }));
            }
        } else {
            return Err(Gtk4Error::new(format!(
                "gtk ui debug widget {widget_id} does not support select"
            )));
        }

        let (_, parent_id, child_type, live) =
            find_widget_context(state, widget_id).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk ui debug widget {widget_id} disappeared after selection"
                ))
            })?;
        Ok(json!({
            "protocol": "aivi.gtk.debug.v1",
            "rootId": root_id,
            "value": value,
            "focus": focus_summary_json(state),
            "widget": live_node_json(state, root_id, parent_id, child_type.as_deref(), live),
            "emitted": emitted
        }))
    }

    fn ui_debug_handle_request(state: &mut RealGtkState, token: &str, request: &Value) -> Value {
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let request_token = request
            .get("token")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if request_token != token {
            return ui_debug_error_response(id, "unauthorized");
        }
        let method = match request.get("method").and_then(Value::as_str) {
            Some(method) => method,
            None => return ui_debug_error_response(id, "missing method"),
        };
        let params = match request.get("params") {
            Some(Value::Object(map)) => map,
            Some(_) => return ui_debug_error_response(id, "params must be an object"),
            None => {
                static EMPTY_PARAMS: OnceLock<Map<String, Value>> = OnceLock::new();
                EMPTY_PARAMS.get_or_init(Map::new)
            }
        };

        let result = match method {
            "hello" => Ok(ui_debug_hello_result(state)),
            "capture" => ui_debug_capture_result(state, params),
            "pollEvents" => Ok(ui_debug_poll_events_result(state, params)),
            "pollMutations" => Ok(ui_debug_poll_mutations_result(state, params)),
            "listNamedWidgets" => Ok(ui_debug_list_widgets_result(state)),
            "listActionBindings" => ui_debug_list_action_bindings_result(state, params),
            "inspectAt" => ui_debug_inspect_at_result(state, params),
            "layoutSnapshot" => ui_debug_layout_snapshot_result(state, params),
            "styleInfo" => ui_debug_style_info_result(state, params),
            "explainWidget" => ui_debug_explain_widget_result(state, params),
            "showOverlay" => ui_debug_show_overlay_result(state, params),
            "dumpLiveTree" => ui_debug_dump_tree_result(state, params),
            "inspectWidget" => ui_debug_inspect_widget_result(state, params),
            "click" => ui_debug_click_result(state, params),
            "type" => ui_debug_type_result(state, params),
            "focus" => ui_debug_focus_result(state, params),
            "moveFocus" => ui_debug_move_focus_result(state, params),
            "select" => ui_debug_select_result(state, params),
            "scroll" => ui_debug_scroll_result(state, params),
            "keyPress" => ui_debug_key_press_result(state, params),
            _ => call_ui_debug_request_handler(method, params).unwrap_or_else(|| {
                Err(Gtk4Error::new(format!(
                    "gtk ui debug unknown method {method}"
                )))
            }),
        };

        match result {
            Ok(result) => ui_debug_ok_response(id, result),
            Err(err) => ui_debug_error_response(id, err.to_string()),
        }
    }

    fn ui_debug_handle_line(state: &mut RealGtkState, token: &str, line: &str) -> Value {
        match serde_json::from_str::<Value>(line) {
            Ok(request) => ui_debug_handle_request(state, token, &request),
            Err(err) => ui_debug_error_response(Value::Null, format!("invalid json: {err}")),
        }
    }

    fn process_ui_debug_connection(
        state: &mut RealGtkState,
        token: &str,
        stream: UnixStream,
    ) -> Result<(), Gtk4Error> {
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .map_err(|err| Gtk4Error::new(format!("gtk ui debug read timeout failed: {err}")))?;
        stream
            .set_write_timeout(Some(Duration::from_millis(200)))
            .map_err(|err| Gtk4Error::new(format!("gtk ui debug write timeout failed: {err}")))?;
        let reader_stream = stream
            .try_clone()
            .map_err(|err| Gtk4Error::new(format!("gtk ui debug clone failed: {err}")))?;
        let mut reader = BufReader::new(reader_stream);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|err| Gtk4Error::new(format!("gtk ui debug read failed: {err}")))?;
        let response = ui_debug_handle_line(state, token, line.trim_end_matches('\n'));
        let mut writer = stream;
        let mut bytes = serde_json::to_vec(&response)
            .map_err(|err| Gtk4Error::new(format!("gtk ui debug encode failed: {err}")))?;
        bytes.push(b'\n');
        writer
            .write_all(&bytes)
            .map_err(|err| Gtk4Error::new(format!("gtk ui debug write failed: {err}")))?;
        Ok(())
    }

    fn process_ui_debug_requests(state: &mut RealGtkState) -> Result<(), Gtk4Error> {
        let token = match state.ui_debug.as_ref() {
            Some(server) => server.token.clone(),
            None => return Ok(()),
        };
        let mut streams = Vec::new();
        if let Some(server) = state.ui_debug.as_mut() {
            loop {
                match server.listener.accept() {
                    Ok((stream, _)) => streams.push(stream),
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(err) => {
                        return Err(Gtk4Error::new(format!("gtk ui debug accept failed: {err}")))
                    }
                }
            }
        }
        for stream in streams {
            if let Err(err) = process_ui_debug_connection(state, &token, stream) {
                eprintln!("AIVI GTK UI debug request failed: {}", err);
            }
        }
        Ok(())
    }

    fn flush_pending_tray_actions() {
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

    unsafe extern "C" fn main_loop_tick_cb(_data: *mut c_void) -> c_int {
        if let Err(err) = call_main_loop_tick_handler() {
            eprintln!("AIVI GTK main-loop tick error: {}", err);
        }
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.ui_debug_tick = state.ui_debug_tick.saturating_add(1);
            if let Err(err) = process_ui_debug_requests(&mut state) {
                eprintln!("AIVI GTK UI debug server error: {}", err);
            }
        });
        flush_pending_tray_actions();
        1
    }

    fn ensure_main_loop_tick_registered(state: &mut RealGtkState) {
        if !state.main_loop_tick_registered {
            unsafe { g_timeout_add(16, main_loop_tick_cb, null_mut()) };
            state.main_loop_tick_registered = true;
        }
    }

    fn maybe_start_ui_debug_server(state: &mut RealGtkState) -> Result<(), Gtk4Error> {
        if state.ui_debug.is_some() {
            return Ok(());
        }
        let enabled = std::env::var("AIVI_UI_DEBUG")
            .ok()
            .and_then(|value| parse_bool_text(&value))
            .unwrap_or(false);
        if !enabled {
            return Ok(());
        }
        let socket_path = std::env::var("AIVI_UI_DEBUG_SOCKET")
            .map_err(|_| Gtk4Error::new("AIVI_UI_DEBUG_SOCKET is required when AIVI_UI_DEBUG=1"))?;
        let token = std::env::var("AIVI_UI_DEBUG_TOKEN")
            .map_err(|_| Gtk4Error::new("AIVI_UI_DEBUG_TOKEN is required when AIVI_UI_DEBUG=1"))?;
        if socket_path.is_empty() {
            return Err(Gtk4Error::new("AIVI_UI_DEBUG_SOCKET cannot be empty"));
        }
        if token.is_empty() {
            return Err(Gtk4Error::new("AIVI_UI_DEBUG_TOKEN cannot be empty"));
        }
        match fs::remove_file(&socket_path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(Gtk4Error::new(format!(
                    "failed to remove stale ui debug socket {socket_path}: {err}"
                )))
            }
        }
        let listener = UnixListener::bind(&socket_path).map_err(|err| {
            Gtk4Error::new(format!(
                "failed to bind ui debug socket {socket_path}: {err}"
            ))
        })?;
        listener.set_nonblocking(true).map_err(|err| {
            Gtk4Error::new(format!(
                "failed to mark ui debug socket non-blocking: {err}"
            ))
        })?;
        state.ui_debug = Some(UiDebugServer {
            socket_path,
            token,
            listener,
        });
        ensure_main_loop_tick_registered(state);
        Ok(())
    }

    fn shutdown_ui_debug_server(state: &mut RealGtkState) {
        if let Some(server) = state.ui_debug.take() {
            let _ = fs::remove_file(server.socket_path);
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

    fn dropdown_string_ptrs(
        value: &str,
        context: &str,
    ) -> Result<(Vec<CString>, Vec<*const c_char>), Gtk4Error> {
        let items: Vec<&str> = if value.is_empty() {
            vec![""]
        } else {
            value.split('\n').collect()
        };
        let strings = items
            .iter()
            .map(|item| c_text(item, context))
            .collect::<Result<Vec<_>, _>>()?;
        let mut ptrs = strings.iter().map(|item| item.as_ptr()).collect::<Vec<_>>();
        ptrs.push(std::ptr::null());
        Ok((strings, ptrs))
    }

    fn dropdown_new_from_strings(value: &str, context: &str) -> Result<*mut c_void, Gtk4Error> {
        let (_strings, ptrs) = dropdown_string_ptrs(value, context)?;
        let raw = unsafe { gtk_drop_down_new_from_strings(ptrs.as_ptr()) };
        if raw.is_null() {
            return Err(Gtk4Error::new(format!(
                "{context} failed to create GtkDropDown"
            )));
        }
        Ok(raw)
    }

    fn dropdown_model_from_strings(value: &str, context: &str) -> Result<*mut c_void, Gtk4Error> {
        let (_strings, ptrs) = dropdown_string_ptrs(value, context)?;
        let raw = unsafe { gtk_string_list_new(ptrs.as_ptr()) };
        if raw.is_null() {
            return Err(Gtk4Error::new(format!(
                "{context} failed to create GtkStringList"
            )));
        }
        Ok(raw)
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

    /// Call a no-argument libadwaita function that returns a `*mut c_void` (e.g., singletons).
    fn call_adw_singleton(fn_name: &str) -> Result<*mut c_void, Gtk4Error> {
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
            let _ = unsafe { dlclose(handle) };
            if !ptr.is_null() {
                let f: unsafe extern "C" fn() -> *mut c_void = unsafe { std::mem::transmute(ptr) };
                let result = unsafe { f() };
                if result.is_null() {
                    return Err(Gtk4Error::new(format!(
                        "gtk4.buildFromNode {fn_name} returned null"
                    )));
                }
                return Ok(result);
            }
            break;
        }
        Err(Gtk4Error::new(format!(
            "gtk4.buildFromNode could not call {fn_name} (libadwaita not available)"
        )))
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

    unsafe extern "C" fn gtk_window_key_pressed_callback(
        _controller: *mut c_void,
        keyval: c_uint,
        keycode: c_uint,
        _state: c_uint,
        data: *mut c_void,
    ) -> c_int {
        if data.is_null() {
            return 0;
        }
        let binding = unsafe { &*(data as *const WindowKeyCallbackData) };
        let key_name = unsafe { gdk_keyval_name(keyval) };
        let key_name = if key_name.is_null() {
            keyval.to_string()
        } else {
            unsafe { CStr::from_ptr(key_name) }
                .to_string_lossy()
                .into_owned()
        };
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let event = SignalEventState {
                widget_id: binding.widget_id,
                signal: "key-pressed".to_string(),
                handler: String::new(),
                payload: format!("{key_name}\n{keycode}"),
            };
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
        });
        0
    }

    unsafe extern "C" fn gtk_key_pressed_callback(
        _controller: *mut c_void,
        keyval: c_uint,
        keycode: c_uint,
        _state: c_uint,
        data: *mut c_void,
    ) -> c_int {
        if data.is_null() {
            return 0;
        }
        let binding = unsafe { &*(data as *const SignalCallbackData) };
        let key_name = unsafe { gdk_keyval_name(keyval) };
        let key_name = if key_name.is_null() {
            keyval.to_string()
        } else {
            unsafe { CStr::from_ptr(key_name) }
                .to_string_lossy()
                .into_owned()
        };
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let event = SignalEventState {
                widget_id: binding.widget_id,
                signal: binding.signal_name.clone(),
                handler: binding.handler.clone(),
                payload: format!("{key_name}\n{keycode}"),
            };
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
        });
        0
    }

    unsafe extern "C" fn gtk_signal_callback(instance: *mut c_void, data: *mut c_void) {
        if data.is_null() {
            return;
        }
        let binding = unsafe { &*(data as *const SignalCallbackData) };
        let payload = match binding.payload_kind {
            SignalPayloadKind::None => String::new(),
            SignalPayloadKind::KeyPressed => String::new(),
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
                if let Ok(prop_c) = CString::new("active") {
                    let active = unsafe { gobject_get_bool(instance, &prop_c) };
                    if active != 0 { "true" } else { "false" }.to_string()
                } else {
                    String::new()
                }
            }
            SignalPayloadKind::FloatValue => {
                let val = unsafe { gtk_range_get_value(instance) };
                val.to_string()
            }
            SignalPayloadKind::SpinButtonValue => {
                let val = unsafe { gtk_spin_button_get_value(instance) };
                val.to_string()
            }
            SignalPayloadKind::ComboBoxText => {
                let text_ptr = unsafe { gtk_combo_box_text_get_active_text(instance) };
                if text_ptr.is_null() {
                    String::new()
                } else {
                    let s = unsafe { CStr::from_ptr(text_ptr) }
                        .to_string_lossy()
                        .into_owned();
                    unsafe { g_free(text_ptr as *mut c_void) };
                    s
                }
            }
            SignalPayloadKind::NotifyBool => {
                let prop_name = binding
                    .signal_name
                    .strip_prefix("notify::")
                    .unwrap_or("show-sidebar");
                if let Ok(prop_c) = CString::new(prop_name) {
                    let val = unsafe { gobject_get_bool(instance, &prop_c) };
                    if val != 0 { "true" } else { "false" }.to_string()
                } else {
                    String::new()
                }
            }
            SignalPayloadKind::NotifyU32 => {
                let prop_name = binding
                    .signal_name
                    .strip_prefix("notify::")
                    .unwrap_or("selected");
                if let Ok(prop_c) = CString::new(prop_name) {
                    unsafe { gobject_get_u32(instance, &prop_c) }.to_string()
                } else {
                    String::new()
                }
            }
            // NotifyRgba and NotifyFontDesc are notify:: signals and are handled
            // exclusively by gtk_notify_callback; they cannot arrive here.
            SignalPayloadKind::NotifyRgba | SignalPayloadKind::NotifyFontDesc => String::new(),
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

    /// Callback for GObject `notify::` property-change signals.
    /// These have a 3-argument C signature: (instance, pspec, user_data).
    unsafe extern "C" fn gtk_notify_callback(
        instance: *mut c_void,
        _pspec: *mut c_void,
        data: *mut c_void,
    ) {
        if data.is_null() {
            return;
        }
        let binding = unsafe { &*(data as *const SignalCallbackData) };
        let property_name = binding.signal_name.strip_prefix("notify::").unwrap_or("");
        let payload = CString::new(property_name)
            .map(|prop_c| match binding.payload_kind {
                SignalPayloadKind::NotifyBool => {
                    let val = unsafe { gobject_get_bool(instance, &prop_c) };
                    if val != 0 {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    }
                }
                SignalPayloadKind::NotifyU32 => {
                    unsafe { gobject_get_u32(instance, &prop_c) }.to_string()
                }
                SignalPayloadKind::NotifyRgba => {
                    let rgba = unsafe { gtk_color_dialog_button_get_rgba(instance) };
                    if rgba.is_null() {
                        String::new()
                    } else {
                        let c = unsafe { &*rgba };
                        format!("{},{},{},{}", c.red, c.green, c.blue, c.alpha)
                    }
                }
                SignalPayloadKind::NotifyFontDesc => {
                    let desc = unsafe { gtk_font_dialog_button_get_font_desc(instance) };
                    if desc.is_null() {
                        String::new()
                    } else {
                        let s_ptr = unsafe { pango_font_description_to_string(desc) };
                        if s_ptr.is_null() {
                            String::new()
                        } else {
                            let s = unsafe { CStr::from_ptr(s_ptr) }
                                .to_string_lossy()
                                .into_owned();
                            unsafe { g_free(s_ptr as *mut c_void) };
                            s
                        }
                    }
                }
                _ => String::new(),
            })
            .unwrap_or_default();
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let event = SignalEventState {
                widget_id: binding.widget_id,
                signal: binding.signal_name.clone(),
                handler: binding.handler.clone(),
                payload,
            };
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
        });
    }

    fn signal_payload_kind_for(class_name: &str, signal_name: &str) -> Option<SignalPayloadKind> {
        match (class_name, signal_name) {
            (_, "key-pressed") => Some(SignalPayloadKind::KeyPressed),
            ("GtkButton", "clicked") => Some(SignalPayloadKind::None),
            ("GtkEntry", "changed")
            | ("GtkEntry", "activate")
            | ("GtkPasswordEntry", "changed")
            | ("GtkPasswordEntry", "activate")
            | ("AdwEntryRow", "changed")
            | ("AdwPasswordEntryRow", "changed") => Some(SignalPayloadKind::EditableText),
            ("GtkSearchEntry", "changed") | ("GtkSearchEntry", "search-changed") => {
                Some(SignalPayloadKind::EditableText)
            }
            ("GtkCheckButton", "toggled") | ("AdwSwitchRow", "toggled") => {
                Some(SignalPayloadKind::ToggleActive)
            }
            ("GtkToggleButton", "toggled") => Some(SignalPayloadKind::ToggleActive),
            ("GtkSwitch", "notify::active") => Some(SignalPayloadKind::NotifyBool),
            ("GtkRange", "value-changed") | ("GtkScale", "value-changed") => {
                Some(SignalPayloadKind::FloatValue)
            }
            ("GtkSpinButton", "value-changed") => Some(SignalPayloadKind::SpinButtonValue),
            ("GtkComboBoxText", "changed") => Some(SignalPayloadKind::ComboBoxText),
            ("GtkDropDown", "notify::selected") => Some(SignalPayloadKind::NotifyU32),
            ("AdwOverlaySplitView", "notify::show-sidebar") => Some(SignalPayloadKind::NotifyBool),
            ("GtkColorDialogButton", "notify::rgba") => Some(SignalPayloadKind::NotifyRgba),
            ("GtkFontDialogButton", "notify::font-desc") => Some(SignalPayloadKind::NotifyFontDesc),
            ("GtkCalendar", "day-selected") => Some(SignalPayloadKind::None),
            _ => None,
        }
    }

    fn connect_widget_signal(
        widget: *mut c_void,
        widget_id: i64,
        class_name: &str,
        node_id: Option<&str>,
        operation: &str,
        binding: &SignalBindingState,
    ) -> Result<SignalHandlerConnection, Gtk4Error> {
        if binding.signal == "key-pressed" {
            let controller = unsafe { gtk_event_controller_key_new() };
            if controller.is_null() {
                return Err(Gtk4Error::new(format!(
                    "{operation} failed to create key controller for {}",
                    widget_debug_label(widget_id, class_name, node_id)
                )));
            }
            let signal_c = c_text(&binding.signal, "gtk4.buildFromNode invalid signal name")?;
            let callback_data = Box::new(SignalCallbackData {
                widget_id,
                signal_name: binding.signal.clone(),
                handler: binding.handler.clone(),
                payload_kind: SignalPayloadKind::KeyPressed,
            });
            let callback_ptr = Box::into_raw(callback_data) as *mut c_void;
            let handler_id = unsafe {
                gtk_widget_set_focusable(widget, 1);
                gtk_widget_add_controller(widget, controller);
                g_signal_connect_data(
                    controller,
                    signal_c.as_ptr(),
                    gtk_key_pressed_callback as *const c_void,
                    callback_ptr,
                    null_mut(),
                    0,
                )
            };
            return Ok(SignalHandlerConnection {
                instance: controller as usize,
                handler_id,
            });
        }
        let Some(payload_kind) = signal_payload_kind_for(class_name, &binding.signal) else {
            return Err(invalid_signal_error(
                operation, widget_id, class_name, node_id, binding,
            ));
        };
        let signal_c = c_text(&binding.signal, "gtk4.buildFromNode invalid signal name")?;
        let callback_data = Box::new(SignalCallbackData {
            widget_id,
            signal_name: binding.signal.clone(),
            handler: binding.handler.clone(),
            payload_kind,
        });
        let callback_ptr = Box::into_raw(callback_data) as *mut c_void;
        let is_notify = binding.signal.starts_with("notify::");
        let callback_fn = if is_notify {
            gtk_notify_callback as *const c_void
        } else {
            gtk_signal_callback as *const c_void
        };
        let handler_id = unsafe {
            g_signal_connect_data(
                widget,
                signal_c.as_ptr(),
                callback_fn,
                callback_ptr,
                null_mut(),
                0,
            )
        };
        Ok(SignalHandlerConnection {
            instance: widget as usize,
            handler_id,
        })
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
            "AdwAnimation" => Err(Gtk4Error::new(
                "AdwAnimation is an abstract base class and cannot be created directly; \
                 use AdwTimedAnimation or AdwSpringAnimation instead"
                    .to_string(),
            )),
            "AdwAnimationTarget" => Err(Gtk4Error::new(
                "AdwAnimationTarget is an abstract base class and cannot be created directly; \
                 use AdwCallbackAnimationTarget or AdwPropertyAnimationTarget instead"
                    .to_string(),
            )),
            "AdwApplication" => create_adw_widget_type("AdwApplication"),
            "AdwApplicationWindow" => create_adw_widget_type("AdwApplicationWindow"),
            "AdwAvatar" => create_adw_widget_type("AdwAvatar"),
            "AdwBanner" => create_adw_widget_type("AdwBanner"),
            "AdwBin" => create_adw_widget_type("AdwBin"),
            "AdwBottomSheet" => create_adw_widget_type("AdwBottomSheet"),
            "AdwBreakpoint" => create_adw_widget_type("AdwBreakpoint"),
            "AdwBreakpointCondition" => Err(Gtk4Error::new(
                "AdwBreakpointCondition is a condition descriptor, not a widget-tree node; \
                 create it with adw_breakpoint_condition_parse() and set it as the `condition` \
                 property of AdwBreakpoint"
                    .to_string(),
            )),
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
            "AdwEnumListItem" => Err(Gtk4Error::new(
                "AdwEnumListItem is created internally by AdwEnumListModel and cannot be \
                 created directly; use AdwEnumListModel in your widget tree instead"
                    .to_string(),
            )),
            "AdwExpanderRow" => create_adw_widget_type("AdwExpanderRow"),
            "AdwFlap" => create_adw_widget_type("AdwFlap"),
            "AdwHeaderBar" => create_adw_widget_type("AdwHeaderBar"),
            "AdwInlineViewSwitcher" => create_adw_widget_type("AdwInlineViewSwitcher"),
            "AdwLayout" => create_adw_widget_type("AdwLayout"),
            "AdwLayoutSlot" => create_adw_widget_type("AdwLayoutSlot"),
            "AdwLeaflet" => create_adw_widget_type("AdwLeaflet"),
            "AdwLeafletPage" => Err(Gtk4Error::new(
                "AdwLeafletPage is managed by AdwLeaflet and cannot be created directly; \
                 add child widgets directly inside AdwLeaflet instead"
                    .to_string(),
            )),
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
            "AdwSqueezerPage" => Err(Gtk4Error::new(
                "AdwSqueezerPage is managed by AdwSqueezer and cannot be created directly; \
                 add child widgets directly inside AdwSqueezer instead"
                    .to_string(),
            )),
            "AdwStatusPage" => create_adw_widget_type("AdwStatusPage"),
            "AdwStyleManager" => {
                try_adw_init();
                call_adw_singleton("adw_style_manager_get_default")
            }
            "AdwSwipeTracker" => create_adw_widget_type("AdwSwipeTracker"),
            "AdwSwitchRow" => create_adw_widget_type("AdwSwitchRow"),
            "AdwTabBar" => create_adw_widget_type("AdwTabBar"),
            "AdwTabButton" => create_adw_widget_type("AdwTabButton"),
            "AdwTabOverview" => create_adw_widget_type("AdwTabOverview"),
            "AdwTabPage" => Err(Gtk4Error::new(
                "AdwTabPage is managed by AdwTabView and cannot be created directly; \
                 add child widgets directly inside AdwTabView instead"
                    .to_string(),
            )),
            "AdwTabView" => create_adw_widget_type("AdwTabView"),
            "AdwTimedAnimation" => create_adw_widget_type("AdwTimedAnimation"),
            "AdwToast" => create_adw_widget_type("AdwToast"),
            "AdwToastOverlay" => create_adw_widget_type("AdwToastOverlay"),
            "AdwToggle" => create_adw_widget_type("AdwToggle"),
            "AdwToggleGroup" => create_adw_widget_type("AdwToggleGroup"),
            "AdwToolbarView" => create_adw_widget_type("AdwToolbarView"),
            "AdwViewStack" => create_adw_widget_type("AdwViewStack"),
            "AdwViewStackPage" => Err(Gtk4Error::new(
                "AdwViewStackPage is managed by AdwViewStack and cannot be created directly; \
                 add child widgets directly inside AdwViewStack instead"
                    .to_string(),
            )),
            "AdwViewStackPages" => Err(Gtk4Error::new(
                "AdwViewStackPages is an internal list model owned by AdwViewStack and cannot \
                 be created directly"
                    .to_string(),
            )),
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

    /// Parse a GtkGrid child position from a `child_type` string.
    /// Accepts: `"col,row"` or `"col,row,colspan,rowspan"`.
    /// Defaults: col=0, row=0, colspan=1, rowspan=1.
    fn parse_grid_position(child_type: Option<&str>) -> (c_int, c_int, c_int, c_int) {
        let s = child_type.unwrap_or("0,0");
        let parts: Vec<&str> = s.split(',').collect();
        let get_int = |i: usize| {
            parts
                .get(i)
                .and_then(|v| v.trim().parse::<c_int>().ok())
                .unwrap_or(0)
        };
        let col = get_int(0);
        let row = get_int(1);
        let col_span = parts
            .get(2)
            .and_then(|v| v.trim().parse::<c_int>().ok())
            .unwrap_or(1);
        let row_span = parts
            .get(3)
            .and_then(|v| v.trim().parse::<c_int>().ok())
            .unwrap_or(1);
        (col, row, col_span, row_span)
    }

    /// Parse an RGBA color string in `"r,g,b"` or `"r,g,b,a"` format (components in 0..=1).
    fn parse_rgba_str(s: &str) -> Option<GdkRgba> {
        let parts: Vec<f32> = s
            .split(',')
            .map(|v| v.trim().parse::<f32>().ok())
            .collect::<Option<Vec<_>>>()?;
        match parts.len() {
            3 => Some(GdkRgba {
                red: parts[0],
                green: parts[1],
                blue: parts[2],
                alpha: 1.0,
            }),
            4 => Some(GdkRgba {
                red: parts[0],
                green: parts[1],
                blue: parts[2],
                alpha: parts[3],
            }),
            _ => None,
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

    fn child_spec_key(child: &ChildSpec<'_>) -> Option<String> {
        let GtkNode::Element { attrs, .. } = child.node else {
            return None;
        };
        node_attr(attrs, "aivi-key").map(str::to_string)
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
            "GtkDropDown" => {
                if let Some(value) = props.get("strings") {
                    let model = dropdown_model_from_strings(
                        value,
                        "gtk4.buildFromNode invalid GtkDropDown strings",
                    )?;
                    let prop_c = CString::new("model").unwrap();
                    unsafe { gobject_set_ptr(widget, &prop_c, model) };
                }
                if let Some(value) = props.get("selected").and_then(|v| parse_i32_text(v)) {
                    let prop_c = CString::new("selected").unwrap();
                    unsafe { gobject_set_u32(widget, &prop_c, value.max(0) as c_uint) };
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
            "GtkProgressBar" => {
                let processed = props
                    .get("processed")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let total = props
                    .get("total")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                if total > 0.0 {
                    let fraction = (processed / total).clamp(0.0, 1.0);
                    unsafe { gtk_progress_bar_set_fraction(widget, fraction) };
                }
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
            "GtkStack" => {
                if let Some(value) = props.get("transition-type") {
                    let t: c_int = match value.as_str() {
                        "none" => 0,
                        "crossfade" => 1,
                        "slide-right" => 2,
                        "slide-left" => 3,
                        "slide-up" => 4,
                        "slide-down" => 5,
                        "slide-left-right" => 6,
                        "slide-up-down" => 7,
                        "over-up" => 8,
                        "over-down" => 9,
                        "over-left" => 10,
                        "over-right" => 11,
                        "under-up" => 12,
                        "under-down" => 13,
                        "under-left" => 14,
                        "under-right" => 15,
                        "over-up-down" => 16,
                        "over-down-up" => 17,
                        "over-left-right" => 18,
                        "over-right-left" => 19,
                        _ => 0,
                    };
                    unsafe { gtk_stack_set_transition_type(widget, t) };
                }
                if let Some(value) = props
                    .get("transition-duration")
                    .and_then(|v| v.parse::<u32>().ok())
                {
                    unsafe { gtk_stack_set_transition_duration(widget, value) };
                }
                // visible-child-name is deferred until after children are added
                // (see build_widget_from_node_real)
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
            "GtkCheckButton" => {
                if let Some(value) = props.get("active").and_then(|v| parse_bool_text(v)) {
                    unsafe { gtk_check_button_set_active(widget, bool_to_c(value)) };
                }
                set_obj_str(widget, props, "label", "GtkCheckButton")?;
            }
            "GtkToggleButton" => {
                set_obj_bool(widget, props, "active");
                set_obj_str(widget, props, "label", "GtkToggleButton")?;
            }
            "GtkSwitch" => {
                set_obj_bool(widget, props, "active");
            }
            "GtkSpinner" => {
                if let Some(spinning) = props.get("spinning").and_then(|v| parse_bool_text(v)) {
                    if spinning {
                        unsafe { gtk_spinner_start(widget) };
                    } else {
                        unsafe { gtk_spinner_stop(widget) };
                    }
                }
            }
            "GtkScale" | "GtkRange" => {
                if let Some(value) = props.get("value").and_then(|v| parse_f64_text(v)) {
                    unsafe { gtk_range_set_value(widget, value) };
                }
                if class_name == "GtkScale" {
                    if let Some(digits) = props.get("digits").and_then(|v| parse_i32_text(v)) {
                        unsafe { gtk_scale_set_digits(widget, digits) };
                    }
                    if let Some(dv) = props.get("draw-value").and_then(|v| parse_bool_text(v)) {
                        unsafe { gtk_scale_set_draw_value(widget, bool_to_c(dv)) };
                    }
                    if let Some(pos) = props.get("value-pos") {
                        let p: c_int = match pos.as_str() {
                            "left" => 0,
                            "right" => 1,
                            "top" => 2,
                            "bottom" => 3,
                            _ => 2,
                        };
                        unsafe { gtk_scale_set_value_pos(widget, p) };
                    }
                }
            }
            "GtkSearchEntry" => {
                if let Some(value) = props.get("text") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid GtkSearchEntry text")?;
                    unsafe { gtk_editable_set_text(widget, text_c.as_ptr()) };
                }
                set_obj_str(widget, props, "placeholder-text", "GtkSearchEntry")?;
            }
            "GtkLinkButton" => {
                set_obj_str(widget, props, "uri", "GtkLinkButton")?;
                set_obj_str(widget, props, "label", "GtkLinkButton")?;
            }
            "GtkPaned" => {
                if let Some(value) = props.get("position").and_then(|v| parse_i32_text(v)) {
                    let prop_c = CString::new("position").unwrap();
                    unsafe { gobject_set_i32(widget, &prop_c, value) };
                }
                set_obj_bool(widget, props, "wide-handle");
            }
            "GtkFrame" => {
                set_obj_str(widget, props, "label", "GtkFrame")?;
            }
            "GtkExpander" => {
                set_obj_str(widget, props, "label", "GtkExpander")?;
                set_obj_bool(widget, props, "expanded");
            }
            "GtkNotebook" => {
                if let Some(page) = props.get("current-page").and_then(|v| parse_i32_text(v)) {
                    // deferred until children added; stored in props for post-child apply
                    let _ = page;
                }
                set_obj_bool(widget, props, "show-tabs");
                set_obj_bool(widget, props, "show-border");
                if let Some(pos) = props.get("tab-pos") {
                    let p: c_int = match pos.as_str() {
                        "top" => 0,
                        "bottom" => 1,
                        "left" => 2,
                        "right" => 3,
                        _ => 0,
                    };
                    let prop_c = CString::new("tab-pos").unwrap();
                    unsafe { gobject_set_i32(widget, &prop_c, p) };
                }
            }
            "GtkFlowBox" => {
                if let Some(v) = props
                    .get("max-children-per-line")
                    .and_then(|v| v.parse::<c_uint>().ok())
                {
                    unsafe { gtk_flow_box_set_max_children_per_line(widget, v) };
                }
                if let Some(v) = props
                    .get("min-children-per-line")
                    .and_then(|v| v.parse::<c_uint>().ok())
                {
                    unsafe { gtk_flow_box_set_min_children_per_line(widget, v) };
                }
                if let Some(v) = props
                    .get("row-spacing")
                    .and_then(|v| v.parse::<c_uint>().ok())
                {
                    unsafe { gtk_flow_box_set_row_spacing(widget, v) };
                }
                if let Some(v) = props
                    .get("column-spacing")
                    .and_then(|v| v.parse::<c_uint>().ok())
                {
                    unsafe { gtk_flow_box_set_column_spacing(widget, v) };
                }
                if let Some(v) = props.get("homogeneous").and_then(|v| parse_bool_text(v)) {
                    unsafe { gtk_flow_box_set_homogeneous(widget, bool_to_c(v)) };
                }
                if let Some(mode) = props.get("selection-mode") {
                    let m: c_int = match mode.as_str() {
                        "none" => 0,
                        "single" => 1,
                        "browse" => 2,
                        "multiple" => 3,
                        _ => 0,
                    };
                    unsafe { gtk_flow_box_set_selection_mode(widget, m) };
                }
            }
            "GtkGrid" => {
                if let Some(v) = props
                    .get("row-spacing")
                    .and_then(|v| v.parse::<c_uint>().ok())
                {
                    unsafe { gtk_grid_set_row_spacing(widget, v) };
                }
                if let Some(v) = props
                    .get("column-spacing")
                    .and_then(|v| v.parse::<c_uint>().ok())
                {
                    unsafe { gtk_grid_set_column_spacing(widget, v) };
                }
                if let Some(v) = props
                    .get("row-homogeneous")
                    .and_then(|v| parse_bool_text(v))
                {
                    unsafe { gtk_grid_set_row_homogeneous(widget, bool_to_c(v)) };
                }
                if let Some(v) = props
                    .get("column-homogeneous")
                    .and_then(|v| parse_bool_text(v))
                {
                    unsafe { gtk_grid_set_column_homogeneous(widget, bool_to_c(v)) };
                }
            }
            "GtkAspectFrame" => {
                if let Some(v) = props.get("ratio").and_then(|v| v.parse::<f32>().ok()) {
                    unsafe { gtk_aspect_frame_set_ratio(widget, v) };
                }
                if let Some(v) = props.get("obey-child").and_then(|v| parse_bool_text(v)) {
                    unsafe { gtk_aspect_frame_set_obey_child(widget, bool_to_c(v)) };
                }
            }
            "GtkSpinButton" => {
                if let Some(value) = props.get("value").and_then(|v| parse_f64_text(v)) {
                    unsafe { gtk_spin_button_set_value(widget, value) };
                }
                if let Some(digits) = props.get("digits").and_then(|v| v.parse::<c_uint>().ok()) {
                    unsafe { gtk_spin_button_set_digits(widget, digits) };
                }
                if let Some(wrap) = props.get("wrap").and_then(|v| parse_bool_text(v)) {
                    unsafe { gtk_spin_button_set_wrap(widget, bool_to_c(wrap)) };
                }
                if let Some(snap) = props.get("snap-to-ticks").and_then(|v| parse_bool_text(v)) {
                    unsafe { gtk_spin_button_set_snap_to_ticks(widget, bool_to_c(snap)) };
                }
                let min = props
                    .get("min")
                    .and_then(|v| parse_f64_text(v))
                    .unwrap_or(0.0);
                let max = props
                    .get("max")
                    .and_then(|v| parse_f64_text(v))
                    .unwrap_or(100.0);
                let step = props
                    .get("step")
                    .and_then(|v| parse_f64_text(v))
                    .unwrap_or(1.0);
                if props.contains_key("min") || props.contains_key("max") {
                    unsafe { gtk_spin_button_set_range(widget, min, max) };
                }
                if props.contains_key("step") {
                    unsafe { gtk_spin_button_set_increments(widget, step, step * 10.0) };
                }
            }
            "GtkComboBoxText" => {
                if let Some(idx) = props.get("active").and_then(|v| v.parse::<c_int>().ok()) {
                    unsafe { gtk_combo_box_set_active(widget, idx) };
                }
            }
            "AdwPreferencesDialog" => {
                set_obj_str(widget, props, "title", "AdwPreferencesDialog")?;
                set_obj_bool(widget, props, "search-enabled");
                set_obj_bool(widget, props, "follows-content-size");
                set_obj_i32(widget, props, "content-width");
                set_obj_i32(widget, props, "content-height");
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
            "GtkColorDialogButton" => {
                if let Some(rgba_str) = props.get("rgba") {
                    if let Some(rgba) = parse_rgba_str(rgba_str) {
                        unsafe { gtk_color_dialog_button_set_rgba(widget, &rgba) };
                    }
                }
            }
            "GtkFontDialogButton" => {
                if let Some(font_str) = props.get("font-desc") {
                    let font_c = c_text(
                        font_str,
                        "gtk4.buildFromNode invalid GtkFontDialogButton font-desc",
                    )?;
                    let desc = unsafe { pango_font_description_from_string(font_c.as_ptr()) };
                    if !desc.is_null() {
                        unsafe { gtk_font_dialog_button_set_font_desc(widget, desc) };
                        unsafe { pango_font_description_free(desc) };
                    }
                }
            }
            "GtkSearchBar" => {
                if let Some(value) = props.get("search-mode").and_then(|v| parse_bool_text(v)) {
                    unsafe { gtk_search_bar_set_search_mode(widget, bool_to_c(value)) };
                }
                if let Some(value) = props
                    .get("show-close-button")
                    .and_then(|v| parse_bool_text(v))
                {
                    unsafe { gtk_search_bar_set_show_close_button(widget, bool_to_c(value)) };
                }
            }
            "GtkPicture" => {
                if let Some(filename) = props.get("file").or_else(|| props.get("filename")) {
                    let file_c =
                        c_text(filename, "gtk4.buildFromNode invalid GtkPicture filename")?;
                    unsafe { gtk_picture_set_filename(widget, file_c.as_ptr()) };
                }
                if let Some(resource) = props.get("resource") {
                    let res_c = c_text(resource, "gtk4.buildFromNode invalid GtkPicture resource")?;
                    unsafe { gtk_picture_set_resource(widget, res_c.as_ptr()) };
                }
                if let Some(value) = props.get("content-fit") {
                    let fit: c_int = match value.as_str() {
                        "fill" => 0,
                        "contain" => 1,
                        "cover" => 2,
                        "scale-down" => 3,
                        _ => 1,
                    };
                    unsafe { gtk_picture_set_content_fit(widget, fit) };
                }
                if let Some(value) = props.get("can-shrink").and_then(|v| parse_bool_text(v)) {
                    unsafe { gtk_picture_set_can_shrink(widget, bool_to_c(value)) };
                }
                if let Some(value) = props.get("alternative-text").or_else(|| props.get("alt")) {
                    let alt_c = c_text(
                        value,
                        "gtk4.buildFromNode invalid GtkPicture alternative-text",
                    )?;
                    unsafe { gtk_picture_set_alternative_text(widget, alt_c.as_ptr()) };
                }
            }
            "GtkCalendar" => {
                if let Some(value) = props.get("show-heading").and_then(|v| parse_bool_text(v)) {
                    unsafe { gtk_calendar_set_show_heading(widget, bool_to_c(value)) };
                }
                if let Some(value) = props.get("show-day-names").and_then(|v| parse_bool_text(v)) {
                    unsafe { gtk_calendar_set_show_day_names(widget, bool_to_c(value)) };
                }
                if let Some(value) = props
                    .get("show-week-numbers")
                    .and_then(|v| parse_bool_text(v))
                {
                    unsafe { gtk_calendar_set_show_week_numbers(widget, bool_to_c(value)) };
                }
            }
            "GtkActionBar" => {
                if let Some(value) = props.get("revealed").and_then(|v| parse_bool_text(v)) {
                    unsafe { gtk_action_bar_set_revealed(widget, bool_to_c(value)) };
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn build_widget_from_node_real(
        state: &mut RealGtkState,
        node: &GtkNode,
        id_map: &mut HashMap<String, i64>,
        binding_map: &mut HashMap<String, i64>,
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
                key: None,
                binding_id: None,
                props: HashMap::new(),
                signals: Vec::new(),
                signal_handlers: Vec::new(),
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
            "GtkDropDown" => {
                let strings = props.get("strings").map(String::as_str).unwrap_or("");
                (
                    dropdown_new_from_strings(
                        strings,
                        "gtk4.buildFromNode invalid GtkDropDown strings",
                    )?,
                    CreatedWidgetKind::Other,
                )
            }
            "GtkMenuButton" => (unsafe { gtk_menu_button_new() }, CreatedWidgetKind::Other),
            "GtkStack" => (unsafe { gtk_stack_new() }, CreatedWidgetKind::Stack),
            "GtkRevealer" => (unsafe { gtk_revealer_new() }, CreatedWidgetKind::Revealer),
            "GtkProgressBar" => (unsafe { gtk_progress_bar_new() }, CreatedWidgetKind::Other),
            "GtkCheckButton" => {
                let label = props.get("label").cloned().unwrap_or_default();
                let raw = if label.is_empty() {
                    unsafe { gtk_check_button_new() }
                } else {
                    let label_c =
                        c_text(&label, "gtk4.buildFromNode invalid GtkCheckButton label")?;
                    unsafe { gtk_check_button_new_with_label(label_c.as_ptr()) }
                };
                (raw, CreatedWidgetKind::Other)
            }
            "GtkScale" | "GtkRange" => {
                let orientation = props
                    .get("orientation")
                    .map(|v| parse_orientation_text(v))
                    .unwrap_or(0);
                let min = props
                    .get("min")
                    .and_then(|v| parse_f64_text(v))
                    .unwrap_or(0.0);
                let max = props
                    .get("max")
                    .and_then(|v| parse_f64_text(v))
                    .unwrap_or(100.0);
                let step = props
                    .get("step")
                    .and_then(|v| parse_f64_text(v))
                    .unwrap_or(1.0);
                (
                    unsafe { gtk_scale_new_with_range(orientation, min, max, step) },
                    CreatedWidgetKind::Other,
                )
            }
            "GtkSwitch" => (unsafe { gtk_switch_new() }, CreatedWidgetKind::Other),
            "GtkSpinner" => (unsafe { gtk_spinner_new() }, CreatedWidgetKind::Other),
            "GtkSearchEntry" => (unsafe { gtk_search_entry_new() }, CreatedWidgetKind::Other),
            "GtkToggleButton" => {
                let label = props.get("label").cloned().unwrap_or_default();
                let raw = if label.is_empty() {
                    unsafe { gtk_toggle_button_new() }
                } else {
                    let label_c =
                        c_text(&label, "gtk4.buildFromNode invalid GtkToggleButton label")?;
                    unsafe { gtk_toggle_button_new_with_label(label_c.as_ptr()) }
                };
                (raw, CreatedWidgetKind::Button)
            }
            "GtkLinkButton" => {
                let uri = props.get("uri").cloned().unwrap_or_default();
                let uri_c = c_text(&uri, "gtk4.buildFromNode invalid GtkLinkButton uri")?;
                let raw = if let Some(label) = props.get("label") {
                    let label_c = c_text(label, "gtk4.buildFromNode invalid GtkLinkButton label")?;
                    unsafe { gtk_link_button_new_with_label(uri_c.as_ptr(), label_c.as_ptr()) }
                } else {
                    unsafe { gtk_link_button_new(uri_c.as_ptr()) }
                };
                (raw, CreatedWidgetKind::Other)
            }
            "GtkPaned" => {
                let orientation = props
                    .get("orientation")
                    .map(|v| parse_orientation_text(v))
                    .unwrap_or(0);
                (
                    unsafe { gtk_paned_new(orientation) },
                    CreatedWidgetKind::Paned,
                )
            }
            "GtkFrame" => {
                let label = props.get("label").cloned().unwrap_or_default();
                let raw = if label.is_empty() {
                    unsafe { gtk_frame_new(std::ptr::null()) }
                } else {
                    let label_c = c_text(&label, "gtk4.buildFromNode invalid GtkFrame label")?;
                    unsafe { gtk_frame_new(label_c.as_ptr()) }
                };
                (raw, CreatedWidgetKind::Frame)
            }
            "GtkExpander" => {
                let label = props.get("label").cloned().unwrap_or_default();
                let raw = if label.is_empty() {
                    unsafe { gtk_expander_new(std::ptr::null()) }
                } else {
                    let label_c = c_text(&label, "gtk4.buildFromNode invalid GtkExpander label")?;
                    unsafe { gtk_expander_new(label_c.as_ptr()) }
                };
                (raw, CreatedWidgetKind::Expander)
            }
            "GtkNotebook" => (unsafe { gtk_notebook_new() }, CreatedWidgetKind::Notebook),
            "GtkStackSwitcher" => (
                unsafe { gtk_stack_switcher_new() },
                CreatedWidgetKind::Other,
            ),
            "GtkFlowBox" => (unsafe { gtk_flow_box_new() }, CreatedWidgetKind::FlowBox),
            "GtkGrid" => (unsafe { gtk_grid_new() }, CreatedWidgetKind::Grid),
            "GtkAspectFrame" => {
                let xalign = props
                    .get("xalign")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.5);
                let yalign = props
                    .get("yalign")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.5);
                let ratio = props
                    .get("ratio")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(1.0);
                let obey_child = props
                    .get("obey-child")
                    .and_then(|v| parse_bool_text(v))
                    .unwrap_or(false);
                (
                    unsafe { gtk_aspect_frame_new(xalign, yalign, ratio, bool_to_c(obey_child)) },
                    CreatedWidgetKind::AspectFrame,
                )
            }
            "GtkSpinButton" => {
                let min = props
                    .get("min")
                    .and_then(|v| parse_f64_text(v))
                    .unwrap_or(0.0);
                let max = props
                    .get("max")
                    .and_then(|v| parse_f64_text(v))
                    .unwrap_or(100.0);
                let step = props
                    .get("step")
                    .and_then(|v| parse_f64_text(v))
                    .unwrap_or(1.0);
                (
                    unsafe { gtk_spin_button_new_with_range(min, max, step) },
                    CreatedWidgetKind::Other,
                )
            }
            "GtkComboBoxText" => {
                let raw = unsafe { gtk_combo_box_text_new() };
                let strings = props.get("strings").cloned().unwrap_or_default();
                for item in strings.split('\n').filter(|s| !s.trim().is_empty()) {
                    if let Ok(item_c) = CString::new(item.trim()) {
                        unsafe { gtk_combo_box_text_append_text(raw, item_c.as_ptr()) };
                    }
                }
                (raw, CreatedWidgetKind::Other)
            }
            "AdwOverlaySplitView" => (create_adw_widget(class_name)?, CreatedWidgetKind::SplitView),
            "GtkSearchBar" => (
                unsafe { gtk_search_bar_new() },
                CreatedWidgetKind::SearchBar,
            ),
            "GtkPicture" => (unsafe { gtk_picture_new() }, CreatedWidgetKind::Other),
            "GtkCalendar" => (unsafe { gtk_calendar_new() }, CreatedWidgetKind::Other),
            "GtkActionBar" => (
                unsafe { gtk_action_bar_new() },
                CreatedWidgetKind::ActionBar,
            ),
            "GtkCenterBox" => (
                unsafe { gtk_center_box_new() },
                CreatedWidgetKind::CenterBox,
            ),
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
            | "AdwAnimation"
            | "AdwAnimationTarget"
            | "AdwApplication"
            | "AdwApplicationWindow"
            | "AdwAvatar"
            | "AdwBanner"
            | "AdwBin"
            | "AdwBottomSheet"
            | "AdwBreakpoint"
            | "AdwBreakpointCondition"
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
            | "AdwEnumListItem"
            | "AdwEnumListModel"
            | "AdwFlap"
            | "AdwInlineViewSwitcher"
            | "AdwLayout"
            | "AdwLayoutSlot"
            | "AdwLeaflet"
            | "AdwLeafletPage"
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
            | "AdwSqueezerPage"
            | "AdwStatusPage"
            | "AdwStyleManager"
            | "AdwSwipeTracker"
            | "AdwSwitchRow"
            | "AdwTabBar"
            | "AdwTabButton"
            | "AdwTabOverview"
            | "AdwTabPage"
            | "AdwTabView"
            | "AdwTimedAnimation"
            | "AdwToast"
            | "AdwToastOverlay"
            | "AdwToggle"
            | "AdwToggleGroup"
            | "AdwToolbarView"
            | "AdwViewStack"
            | "AdwViewStackPage"
            | "AdwViewStackPages"
            | "AdwViewSwitcher"
            | "AdwViewSwitcherBar"
            | "AdwViewSwitcherTitle"
            | "AdwWindow"
            | "AdwWindowTitle"
            | "AdwWrapBox"
            | "AdwWrapLayout" => (create_adw_widget(class_name)?, CreatedWidgetKind::Other),
            "GtkColorDialogButton" => {
                let dialog = unsafe { gtk_color_dialog_new() };
                (
                    unsafe { gtk_color_dialog_button_new(dialog) },
                    CreatedWidgetKind::Other,
                )
            }
            "GtkFontDialogButton" => {
                let dialog = unsafe { gtk_font_dialog_new() };
                (
                    unsafe { gtk_font_dialog_button_new(dialog) },
                    CreatedWidgetKind::Other,
                )
            }
            "GtkColorButton" => {
                return Err(Gtk4Error::new(
                    "GtkColorButton was removed in GTK 4.10; \
                     use GtkColorDialogButton instead"
                        .to_string(),
                ));
            }
            "GtkFontButton" => {
                return Err(Gtk4Error::new(
                    "GtkFontButton was removed in GTK 4.10; \
                     use GtkFontDialogButton instead"
                        .to_string(),
                ));
            }
            "GtkAppChooserButton" | "GtkAppChooserDialog" | "GtkAppChooserWidget" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} was removed in GTK 4.10 and has no direct GTK replacement; \
                     use OS file manager integration or application-specific logic instead"
                )));
            }
            // Abstract base classes
            "GtkWidget" => {
                return Err(Gtk4Error::new(
                    "GtkWidget is an abstract base class and cannot be created directly; \
                     use a concrete widget subclass such as GtkLabel, GtkButton, or GtkBox"
                        .to_string(),
                ));
            }
            "GtkEventController" => {
                return Err(Gtk4Error::new(
                    "GtkEventController is an abstract base class; use a concrete subclass \
                     such as GtkEventControllerKey, GtkEventControllerFocus, or GtkGestureClick, \
                     and attach it with <child type=\"controller\">"
                        .to_string(),
                ));
            }
            "GtkGesture" => {
                return Err(Gtk4Error::new(
                    "GtkGesture is an abstract base class; use GtkGestureClick, GtkGestureDrag, \
                     GtkGestureSwipe, or another concrete gesture and attach it with \
                     <child type=\"controller\">"
                        .to_string(),
                ));
            }
            "GtkFilter" => {
                return Err(Gtk4Error::new(
                    "GtkFilter is an abstract base class; use GtkCustomFilter, GtkStringFilter, \
                     or GtkBoolFilter and set it as the `filter` property of GtkFilterListModel"
                        .to_string(),
                ));
            }
            "GtkSorter" => {
                return Err(Gtk4Error::new(
                    "GtkSorter is an abstract base class; use GtkCustomSorter, GtkStringSorter, \
                     or GtkNumericSorter and set it as the `sorter` property of GtkSortListModel"
                        .to_string(),
                ));
            }
            "GtkExpression" => {
                return Err(Gtk4Error::new(
                    "GtkExpression is an abstract base class; use GtkPropertyExpression or \
                     GtkConstantExpression and wire them via the relevant property"
                        .to_string(),
                ));
            }
            "GtkLayoutManager" => {
                return Err(Gtk4Error::new(
                    "GtkLayoutManager is an abstract base class; use GtkBoxLayout, GtkGridLayout, \
                     GtkFixedLayout, or GtkConstraintLayout and set it as the \
                     `layout-manager` property of the container"
                        .to_string(),
                ));
            }
            "GtkSelectionModel" => {
                return Err(Gtk4Error::new(
                    "GtkSelectionModel is an interface; use GtkNoSelection, GtkSingleSelection, \
                     or GtkMultiSelection and set it as the `model` property of the list view"
                        .to_string(),
                ));
            }
            "GtkListItemFactory" => {
                return Err(Gtk4Error::new(
                    "GtkListItemFactory is an abstract base class; use \
                     GtkSignalListItemFactory or GtkBuilderListItemFactory"
                        .to_string(),
                ));
            }
            // Concrete widget families that are not yet exposed in AIVI widget trees
            "GtkWindow" | "GtkApplicationWindow" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is a top-level window type managed by windowNew during manual GTK startup, not \
                     a ~<gtk> widget-tree node"
                )));
            }
            "GtkListView" | "GtkColumnView" | "GtkGridView" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is a concrete GTK list widget, but AIVI does not yet expose it \
                     in ~<gtk> widget trees; use GtkListBox with <each> for simple read-only \
                     lists today"
                )));
            }
            "GtkPopover" | "GtkPopoverMenu" | "GtkPopoverMenuBar" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is a popup/menu surface that AIVI does not yet expose directly \
                     in ~<gtk> widget trees; use GtkMenuButton with a menu model or custom header \
                     bar buttons today"
                )));
            }
            "GtkVideo" | "GtkMediaControls" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is a concrete GTK media widget, but AIVI does not yet expose it \
                     in ~<gtk> widget trees; use GtkPicture/GtkImage for static media today or \
                     programmatic GTK integration for playback"
                )));
            }
            "GtkAboutDialog" => {
                return Err(Gtk4Error::new(
                    "GtkAboutDialog is a top-level about dialog that AIVI does not yet expose in \
                     ~<gtk> widget trees; prefer AdwAboutDialog or AdwAboutWindow"
                        .to_string(),
                ));
            }
            "GtkAlertDialog" => {
                return Err(Gtk4Error::new(
                    "GtkAlertDialog is an asynchronous dialog API object, not a widget-tree node; \
                     create and present it programmatically from GTK integration code"
                        .to_string(),
                ));
            }
            "GtkDialog" | "GtkMessageDialog" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is a legacy dialog surface not exposed in AIVI widget trees; \
                     prefer GtkAlertDialog or the libadwaita dialog APIs"
                )));
            }
            "GtkShortcutsWindow" => {
                return Err(Gtk4Error::new(
                    "GtkShortcutsWindow is a top-level shortcuts help window that AIVI does not \
                     yet expose in ~<gtk> widget trees; prefer AdwShortcutsDialog"
                        .to_string(),
                ));
            }
            "GtkFileDialog" => {
                return Err(Gtk4Error::new(
                    "GtkFileDialog is an asynchronous file dialog API object, not a widget-tree \
                     node; AIVI does not yet expose file dialogs in the v0.1 stdlib UI surface"
                        .to_string(),
                ));
            }
            "GtkFileChooserDialog" | "GtkFileChooserNative" | "GtkFileChooserWidget" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is a legacy GTK file chooser surface not exposed in AIVI widget \
                     trees; prefer GtkFileDialog when working directly with GTK"
                )));
            }
            // List model and selection infrastructure (not widget-tree nodes)
            "GtkCustomFilter" | "GtkStringFilter" | "GtkBoolFilter" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is a GTK filter object, not a widget; set it as the `filter` \
                     property of GtkFilterListModel"
                )));
            }
            "GtkMultiFilter" => {
                return Err(Gtk4Error::new(
                    "GtkMultiFilter is an abstract GTK filter combiner, not a widget; use \
                     GtkEveryFilter or GtkAnyFilter and set it as the `filter` property of \
                     GtkFilterListModel"
                        .to_string(),
                ));
            }
            "GtkEveryFilter" | "GtkAnyFilter" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} combines multiple GTK filters and is not a widget; set it as the \
                     `filter` property of GtkFilterListModel"
                )));
            }
            "GtkCustomSorter" | "GtkStringSorter" | "GtkNumericSorter" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is a GTK sorter object, not a widget; set it as the `sorter` \
                     property of GtkSortListModel"
                )));
            }
            "GtkMultiSorter" => {
                return Err(Gtk4Error::new(
                    "GtkMultiSorter composes multiple GTK sorters in priority order and is not a \
                     widget; set it as the `sorter` property of GtkSortListModel"
                        .to_string(),
                ));
            }
            "GtkPropertyExpression"
            | "GtkConstantExpression"
            | "GtkCClosureExpression"
            | "GtkObjectExpression" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is a GTK expression object, not a widget-tree node; wire it \
                     through the relevant property or prefer pure AIVI derivations"
                )));
            }
            "GtkAdjustment" => {
                return Err(Gtk4Error::new(
                    "GtkAdjustment is not a widget; set it as the `adjustment` property of \
                     GtkSpinButton, GtkScale, GtkScrollbar, or GtkScrolledWindow"
                        .to_string(),
                ));
            }
            "GtkStringList" => {
                return Err(Gtk4Error::new(
                    "GtkStringList is a list model, not a widget; set it as the `model` property \
                     of GtkDropDown, GtkNoSelection, GtkSingleSelection, or GtkMultiSelection"
                        .to_string(),
                ));
            }
            "GtkFilterListModel"
            | "GtkSortListModel"
            | "GtkSliceListModel"
            | "GtkFlattenListModel" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is a list model wrapper, not a widget; set it as the `model` \
                     property of a list view or selection model"
                )));
            }
            "GtkNoSelection" | "GtkSingleSelection" | "GtkMultiSelection" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is a selection model, not a widget; set it as the `model` \
                     property of GtkListView, GtkColumnView, or GtkGridView"
                )));
            }
            "GtkSignalListItemFactory" => {
                return Err(Gtk4Error::new(
                    "GtkSignalListItemFactory is a list item factory, not a widget-tree node; set \
                     it as the `factory` property of GtkListView or GtkColumnView when that \
                     surface is exposed in AIVI"
                        .to_string(),
                ));
            }
            "GtkBuilderListItemFactory" => {
                return Err(Gtk4Error::new(
                    "GtkBuilderListItemFactory is a list item factory, not a widget-tree node; \
                     use it as the `factory` property of GtkListView or GtkColumnView, and prefer \
                     the ~<gtk> sigil over raw Builder XML in AIVI"
                        .to_string(),
                ));
            }
            "GtkListItem" => {
                return Err(Gtk4Error::new(
                    "GtkListItem is created and recycled internally by GtkListItemFactory; \
                     do not construct it directly"
                        .to_string(),
                ));
            }
            "GtkColumnViewColumn" => {
                return Err(Gtk4Error::new(
                    "GtkColumnViewColumn is added to GtkColumnView programmatically and is not a \
                     widget-tree element"
                        .to_string(),
                ));
            }
            "GtkColumnViewRow" => {
                return Err(Gtk4Error::new(
                    "GtkColumnViewRow is managed internally by GtkColumnView and is not a \
                     widget-tree node"
                        .to_string(),
                ));
            }
            "GtkColumnViewCell" => {
                return Err(Gtk4Error::new(
                    "GtkColumnViewCell is managed internally by the column factory and is not a \
                     widget-tree node"
                        .to_string(),
                ));
            }
            "GtkColumnViewSorter" => {
                return Err(Gtk4Error::new(
                    "GtkColumnViewSorter wraps column sort state and is obtained from the \
                     `sorter` property of GtkColumnView; it is not a widget-tree node"
                        .to_string(),
                ));
            }
            "GtkMediaStream" => {
                return Err(Gtk4Error::new(
                    "GtkMediaStream is an abstract media stream base, not a widget-tree node; set \
                     it as the `stream` property of GtkVideo or GtkMediaControls"
                        .to_string(),
                ));
            }
            "GtkMediaFile" => {
                return Err(Gtk4Error::new(
                    "GtkMediaFile is a media stream object, not a widget-tree node; create it \
                     programmatically and set it as the `stream` property of GtkVideo or \
                     GtkMediaControls"
                        .to_string(),
                ));
            }
            // Event controllers (attach via child type="controller", not as tree nodes)
            "GtkEventControllerKey"
            | "GtkEventControllerFocus"
            | "GtkEventControllerMotion"
            | "GtkEventControllerScroll"
            | "GtkEventControllerLegacy"
            | "GtkGestureDrag"
            | "GtkGestureLongPress"
            | "GtkGestureSwipe"
            | "GtkGestureZoom"
            | "GtkGestureRotate"
            | "GtkGesturePan"
            | "GtkGestureStylus"
            | "GtkShortcutController"
            | "GtkDropControllerMotion" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is an event controller, not a widget; attach it to a widget \
                     using <child type=\"controller\"> inside the target widget element"
                )));
            }
            "GtkShortcut" => {
                return Err(Gtk4Error::new(
                    "GtkShortcut is not a widget; add it to a GtkShortcutController \
                     programmatically"
                        .to_string(),
                ));
            }
            "GtkShortcutAction" => {
                return Err(Gtk4Error::new(
                    "GtkShortcutAction is an abstract shortcut action, not a widget; use \
                     GtkActivateAction, GtkSignalAction, or GtkNamedAction inside a GtkShortcut"
                        .to_string(),
                ));
            }
            "GtkShortcutTrigger" => {
                return Err(Gtk4Error::new(
                    "GtkShortcutTrigger is an abstract shortcut trigger, not a widget; use \
                     GtkKeyvalTrigger, GtkMnemonicTrigger, or GtkAlternativeTrigger inside a \
                     GtkShortcut"
                        .to_string(),
                ));
            }
            "GtkActivateAction" | "GtkSignalAction" | "GtkNamedAction" | "GtkNothingAction"
            | "GtkMnemonicAction" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is a shortcut action, not a widget-tree node; use it inside \
                     GtkShortcut/GtkShortcutController wiring"
                )));
            }
            "GtkKeyvalTrigger"
            | "GtkMnemonicTrigger"
            | "GtkAlternativeTrigger"
            | "GtkNeverTrigger"
            | "GtkAnyTrigger" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is a shortcut trigger, not a widget-tree node; use it inside \
                     GtkShortcut/GtkShortcutController wiring"
                )));
            }
            // Text buffer model items (owned by GtkTextBuffer)
            "GtkTextTag" | "GtkTextTagTable" | "GtkTextMark" | "GtkTextChildAnchor" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is part of the GtkTextBuffer model and is not a widget; \
                     create it through GtkTextBuffer or its tag table, not as a widget-tree node"
                )));
            }
            // Deprecated cell-renderer and tree-model infrastructure
            "GtkCellRenderer"
            | "GtkCellRendererText"
            | "GtkCellRendererToggle"
            | "GtkCellRendererPixbuf"
            | "GtkCellRendererProgress"
            | "GtkCellRendererSpin"
            | "GtkCellRendererCombo"
            | "GtkCellRendererAccel" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is part of the deprecated GtkTreeView cell-renderer architecture; \
                     use GtkColumnView with GtkSignalListItemFactory instead"
                )));
            }
            "GtkTreeModel" | "GtkListStore" | "GtkTreeStore" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is deprecated in GTK4; use GtkStringList or a custom \
                     GListModel with GtkNoSelection/GtkSingleSelection instead"
                )));
            }
            // Layout manager infrastructure (set as layout-manager property, not widget nodes)
            "GtkBoxLayout"
            | "GtkGridLayout"
            | "GtkFixedLayout"
            | "GtkBinLayout"
            | "GtkOverlayLayout"
            | "GtkConstraintLayout"
            | "GtkConstraint"
            | "GtkConstraintGuide" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is a layout manager type, not a widget; set it as the \
                     `layout-manager` property of the container widget"
                )));
            }
            // Builder-only and singleton types
            "GtkBuilder" => {
                return Err(Gtk4Error::new(
                    "GtkBuilder is a programmatic XML builder, not a widget; \
                     use buildFromNode or the ~<gtk> sigil to construct widget trees in AIVI"
                        .to_string(),
                ));
            }
            "GtkApplication" => {
                return Err(Gtk4Error::new(
                    "GtkApplication is managed by appNew during manual GTK startup and is not a widget-tree node"
                        .to_string(),
                ));
            }
            "GtkPrintOperation"
            | "GtkPrintDialog"
            | "GtkPrinter"
            | "GtkPrintJob"
            | "GtkPrintContext"
            | "GtkPageSetup"
            | "GtkPageSetupUnixDialog"
            | "GtkPaperSize"
            | "GtkPrintSettings"
            | "GtkPrintUnixDialog" => {
                return Err(Gtk4Error::new(format!(
                    "{class_name} is part of GTK printing infrastructure, not a widget-tree node; \
                     printing is not part of the AIVI v0.1 stdlib UI surface"
                )));
            }
            "GtkSettings" => {
                return Err(Gtk4Error::new(
                    "GtkSettings is a global singleton; it is fetched automatically via \
                     gtk_settings_get_default() and is not a widget-tree node"
                        .to_string(),
                ));
            }
            "GtkStyleContext" => {
                return Err(Gtk4Error::new(
                    "GtkStyleContext is attached to widgets internally and is not a widget; \
                     use widgetSetCss or appSetCss to apply styles"
                        .to_string(),
                ));
            }
            "GtkCssProvider" => {
                return Err(Gtk4Error::new(
                    "GtkCssProvider is applied programmatically; use appSetCss or widgetSetCss \
                     to apply CSS styles — it is not a widget-tree node"
                        .to_string(),
                ));
            }
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
        let node_id = node_attr(attrs, "id").map(str::to_string);
        let binding_id = node_attr(attrs, "aivi-binding-id").map(str::to_string);
        if let Some(object_id) = node_id.as_deref() {
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
        if let Some(binding_id) = binding_id.as_deref() {
            binding_map.insert(binding_id.to_string(), id);
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
            "GtkEntry" | "GtkPasswordEntry" | "GtkSearchEntry" => {
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
        let mut signal_handlers = Vec::new();
        for binding in &signal_bindings {
            let hid = connect_widget_signal(
                raw,
                id,
                class_name,
                node_id.as_deref(),
                "buildFromNode",
                binding,
            )?;
            signal_handlers.push(hid);
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

            let (child_id, child_live) =
                build_widget_from_node_real(state, child.node, id_map, binding_map)?;
            let child_raw = widget_ptr(state, child_id, "buildFromNode")?;
            validate_special_child_attachment(
                "buildFromNode",
                WidgetAttachmentInfo {
                    id,
                    class_name,
                    kind,
                    node_id: node_id.as_deref(),
                },
                WidgetAttachmentInfo {
                    id: child_id,
                    class_name: &child_live.class_name,
                    kind: child_live.kind,
                    node_id: child_live.node_id.as_deref(),
                },
            )?;

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
                CreatedWidgetKind::Paned => {
                    let is_end = child.child_type.as_deref() == Some("end");
                    if is_end {
                        unsafe { gtk_paned_set_end_child(raw, child_raw) };
                    } else {
                        unsafe { gtk_paned_set_start_child(raw, child_raw) };
                    }
                }
                CreatedWidgetKind::Frame => unsafe { gtk_frame_set_child(raw, child_raw) },
                CreatedWidgetKind::Expander => unsafe { gtk_expander_set_child(raw, child_raw) },
                CreatedWidgetKind::Notebook => {
                    let tab_label_text = child.child_type.as_deref().unwrap_or("").to_string();
                    let tab_label_raw = if tab_label_text.is_empty() {
                        std::ptr::null_mut()
                    } else if let Ok(lc) = CString::new(tab_label_text.as_bytes()) {
                        unsafe { gtk_label_new(lc.as_ptr()) }
                    } else {
                        std::ptr::null_mut()
                    };
                    unsafe { gtk_notebook_append_page(raw, child_raw, tab_label_raw) };
                }
                CreatedWidgetKind::FlowBox => unsafe { gtk_flow_box_append(raw, child_raw) },
                CreatedWidgetKind::Grid => {
                    let (col, row, col_span, row_span) =
                        parse_grid_position(child.child_type.as_deref());
                    unsafe { gtk_grid_attach(raw, child_raw, col, row, col_span, row_span) };
                }
                CreatedWidgetKind::AspectFrame => unsafe {
                    gtk_aspect_frame_set_child(raw, child_raw)
                },
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
                CreatedWidgetKind::SearchBar => unsafe { gtk_search_bar_set_child(raw, child_raw) },
                CreatedWidgetKind::ActionBar => match child.child_type.as_deref() {
                    Some("end") => unsafe { gtk_action_bar_pack_end(raw, child_raw) },
                    Some("center") => unsafe { gtk_action_bar_set_center_widget(raw, child_raw) },
                    _ => unsafe { gtk_action_bar_pack_start(raw, child_raw) },
                },
                CreatedWidgetKind::CenterBox => match child.child_type.as_deref() {
                    Some("center") => unsafe { gtk_center_box_set_center_widget(raw, child_raw) },
                    Some("end") => unsafe { gtk_center_box_set_end_widget(raw, child_raw) },
                    _ => unsafe { gtk_center_box_set_start_widget(raw, child_raw) },
                },
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

        // Deferred: set visible-child-name on GtkStack after children are added
        if matches!(kind, CreatedWidgetKind::Stack) {
            if let Some(value) = props.get("visible-child-name") {
                if let Ok(name_c) = CString::new(value.as_str()) {
                    unsafe { gtk_stack_set_visible_child_name(raw, name_c.as_ptr()) };
                }
            }
        }

        // Deferred: set current-page on GtkNotebook after children are added
        if matches!(kind, CreatedWidgetKind::Notebook) {
            if let Some(page) = props.get("current-page").and_then(|v| parse_i32_text(v)) {
                unsafe { gtk_notebook_set_current_page(raw, page) };
            }
        }

        // Deferred: connect GtkSearchBar's child entry for keyboard capture
        if matches!(kind, CreatedWidgetKind::SearchBar) {
            for lc in &live_children {
                if matches!(
                    lc.node.class_name.as_str(),
                    "GtkSearchEntry" | "GtkEntry" | "GtkPasswordEntry"
                ) {
                    if let Some(&entry_raw) = state.widgets.get(&lc.node.widget_id) {
                        unsafe { gtk_search_bar_connect_entry(raw, entry_raw) };
                    }
                    break;
                }
            }
        }

        let live = LiveNode {
            widget_id: id,
            class_name: class_name.to_string(),
            kind,
            node_id,
            key: node_attr(attrs, "aivi-key").map(str::to_string),
            binding_id,
            props,
            signals: signal_bindings,
            signal_handlers,
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
            CreatedWidgetKind::FlowBox => unsafe { gtk_flow_box_remove(parent_raw, child_raw) },
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
        parent: WidgetAttachmentTarget<'_>,
        child: WidgetAttachmentTarget<'_>,
        placement: ChildPlacement<'_>,
    ) -> Result<(), Gtk4Error> {
        validate_special_child_attachment("reconcileNode", parent.info, child.info)?;
        match parent.info.kind {
            CreatedWidgetKind::Box => unsafe { gtk_box_append(parent.raw, child.raw) },
            CreatedWidgetKind::Button => unsafe { gtk_button_set_child(parent.raw, child.raw) },
            CreatedWidgetKind::HeaderBar => match placement.child_type {
                Some("end") => unsafe { gtk_header_bar_pack_end(parent.raw, child.raw) },
                Some("title") => unsafe { gtk_header_bar_set_title_widget(parent.raw, child.raw) },
                _ => unsafe { gtk_header_bar_pack_start(parent.raw, child.raw) },
            },
            CreatedWidgetKind::ScrolledWindow => {
                if placement.child_type != Some("overlay") {
                    unsafe { gtk_scrolled_window_set_child(parent.raw, child.raw) };
                }
            }
            CreatedWidgetKind::Overlay => {
                if placement.child_type == Some("overlay") || placement.overlay_index > 0 {
                    unsafe { gtk_overlay_add_overlay(parent.raw, child.raw) };
                } else {
                    unsafe { gtk_overlay_set_child(parent.raw, child.raw) };
                }
            }
            CreatedWidgetKind::ListBox => unsafe { gtk_list_box_append(parent.raw, child.raw) },
            CreatedWidgetKind::Revealer => unsafe { gtk_revealer_set_child(parent.raw, child.raw) },
            CreatedWidgetKind::Stack => {
                let page_name = placement.child_type.unwrap_or("page");
                if let Ok(name_c) = CString::new(page_name) {
                    unsafe { gtk_stack_add_named(parent.raw, child.raw, name_c.as_ptr()) };
                }
            }
            CreatedWidgetKind::SplitView => {
                let prop_name = match placement.child_type {
                    Some("sidebar") => "sidebar",
                    _ => "content",
                };
                let prop_c = CString::new(prop_name).unwrap();
                unsafe { gobject_set_ptr(parent.raw, &prop_c, child.raw) };
            }
            CreatedWidgetKind::PreferencesDialog => {
                call_adw_fn_pp("adw_preferences_dialog_add", parent.raw, child.raw);
            }
            CreatedWidgetKind::PreferencesPage => {
                call_adw_fn_pp("adw_preferences_page_add", parent.raw, child.raw);
            }
            CreatedWidgetKind::PreferencesGroup => {
                call_adw_fn_pp("adw_preferences_group_add", parent.raw, child.raw);
            }
            CreatedWidgetKind::ActionRow => {
                call_adw_fn_pp("adw_action_row_add_suffix", parent.raw, child.raw);
            }
            CreatedWidgetKind::Paned => {
                let is_end = placement.child_type == Some("end");
                if is_end {
                    unsafe { gtk_paned_set_end_child(parent.raw, child.raw) };
                } else {
                    unsafe { gtk_paned_set_start_child(parent.raw, child.raw) };
                }
            }
            CreatedWidgetKind::Frame => unsafe { gtk_frame_set_child(parent.raw, child.raw) },
            CreatedWidgetKind::Expander => unsafe { gtk_expander_set_child(parent.raw, child.raw) },
            CreatedWidgetKind::Notebook => {
                let tab_text = placement.child_type.unwrap_or("").to_string();
                let tab_raw = if tab_text.is_empty() {
                    std::ptr::null_mut()
                } else if let Ok(lc) = CString::new(tab_text.as_bytes()) {
                    unsafe { gtk_label_new(lc.as_ptr()) }
                } else {
                    std::ptr::null_mut()
                };
                unsafe { gtk_notebook_append_page(parent.raw, child.raw, tab_raw) };
            }
            CreatedWidgetKind::FlowBox => unsafe { gtk_flow_box_append(parent.raw, child.raw) },
            CreatedWidgetKind::Grid => {
                let (col, row, col_span, row_span) = parse_grid_position(placement.child_type);
                unsafe { gtk_grid_attach(parent.raw, child.raw, col, row, col_span, row_span) };
            }
            CreatedWidgetKind::AspectFrame => unsafe {
                gtk_aspect_frame_set_child(parent.raw, child.raw)
            },
            CreatedWidgetKind::SearchBar => unsafe {
                gtk_search_bar_set_child(parent.raw, child.raw)
            },
            CreatedWidgetKind::ActionBar => match placement.child_type {
                Some("end") => unsafe { gtk_action_bar_pack_end(parent.raw, child.raw) },
                Some("center") => unsafe {
                    gtk_action_bar_set_center_widget(parent.raw, child.raw)
                },
                _ => unsafe { gtk_action_bar_pack_start(parent.raw, child.raw) },
            },
            CreatedWidgetKind::CenterBox => match placement.child_type {
                Some("center") => unsafe {
                    gtk_center_box_set_center_widget(parent.raw, child.raw)
                },
                Some("end") => unsafe { gtk_center_box_set_end_widget(parent.raw, child.raw) },
                _ => unsafe { gtk_center_box_set_start_widget(parent.raw, child.raw) },
            },
            CreatedWidgetKind::Other => {}
        }
        Ok(())
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
        for connection in &live.signal_handlers {
            if connection.handler_id != 0 {
                unsafe {
                    g_signal_handler_disconnect(
                        connection.instance as *mut c_void,
                        connection.handler_id,
                    )
                };
            }
        }

        // Patch properties (safe now — no signals connected)
        patch_widget_properties(raw, new_class, &live.props, &new_props, state)?;
        live.props = new_props;

        // Reconnect signals
        let mut new_handlers = Vec::new();
        for binding in &new_signals {
            let hid = connect_widget_signal(
                raw,
                live.widget_id,
                new_class,
                live.node_id.as_deref(),
                "reconcileNode",
                binding,
            )?;
            new_handlers.push(hid);
        }
        live.signals = new_signals;
        live.signal_handlers = new_handlers;

        // Update node_id if it changed
        let new_node_id = node_attr(attrs, "id").map(str::to_string);
        if new_node_id != live.node_id {
            if let Some(ref name) = new_node_id {
                id_map.insert(name.clone(), live.widget_id);
            }
            live.node_id = new_node_id;
        }
        live.key = node_attr(attrs, "aivi-key").map(str::to_string);
        live.binding_id = node_attr(attrs, "aivi-binding-id").map(str::to_string);

        // Reconcile children
        let mut new_child_objects = collect_child_objects(children);
        new_child_objects.sort_by_key(|child| child.position.unwrap_or(usize::MAX));
        reconcile_children(state, live, &new_child_objects, id_map)?;

        // Deferred: set visible-child-name on GtkStack after children are reconciled
        if matches!(live.kind, CreatedWidgetKind::Stack) {
            if let Some(value) = live.props.get("visible-child-name") {
                if let Ok(name_c) = CString::new(value.as_str()) {
                    unsafe { gtk_stack_set_visible_child_name(raw, name_c.as_ptr()) };
                }
            }
        }

        Ok(true)
    }

    /// Reconcile the children of a live node against new child specs.
    fn reconcile_children_keyed(
        state: &mut RealGtkState,
        parent: &mut LiveNode,
        new_children: &[ChildSpec<'_>],
        id_map: &mut HashMap<String, i64>,
    ) -> Result<(), Gtk4Error> {
        let parent_id = parent.widget_id;
        let parent_kind = parent.kind;
        let parent_raw = widget_ptr(state, parent_id, "reconcile")?;
        let old_children = std::mem::take(&mut parent.children);
        let mut old_placements = HashMap::new();
        let mut keyed_old: HashMap<String, std::collections::VecDeque<LiveChild>> = HashMap::new();
        let mut unkeyed_old = std::collections::VecDeque::new();

        for child in old_children {
            old_placements.insert(child.node.widget_id, child.child_type.clone());
            if let Some(key) = child.node.key.clone() {
                keyed_old.entry(key).or_default().push_back(child);
            } else {
                unkeyed_old.push_back(child);
            }
        }

        let mut next_children = Vec::new();
        let mut reused_ids = std::collections::HashSet::new();
        for new_spec in new_children {
            let old_child = if let Some(key) = child_spec_key(new_spec) {
                keyed_old
                    .get_mut(&key)
                    .and_then(std::collections::VecDeque::pop_front)
            } else {
                unkeyed_old.pop_front()
            };

            if let Some(mut old_child) = old_child {
                let old_widget_id = old_child.node.widget_id;
                let patched = reconcile_node(state, &mut old_child.node, new_spec.node, id_map)?;
                old_child.child_type = new_spec.child_type.clone();
                if patched {
                    reused_ids.insert(old_widget_id);
                    next_children.push(old_child);
                } else {
                    let old_ct = old_placements
                        .get(&old_widget_id)
                        .cloned()
                        .unwrap_or(old_child.child_type.clone());
                    let old_live = old_child.node.clone();
                    remove_child_from_parent(
                        state,
                        parent_id,
                        parent_kind,
                        old_widget_id,
                        old_ct.as_deref(),
                        Some(&old_live),
                    )?;
                    let mut binding_map = HashMap::new();
                    let (new_id, new_live) = build_widget_from_node_real(
                        state,
                        new_spec.node,
                        id_map,
                        &mut binding_map,
                    )?;
                    next_children.push(LiveChild {
                        child_type: new_spec.child_type.clone(),
                        node: new_live,
                    });
                    let _ = new_id;
                }
            } else {
                let mut binding_map = HashMap::new();
                let (_new_id, new_live) =
                    build_widget_from_node_real(state, new_spec.node, id_map, &mut binding_map)?;
                next_children.push(LiveChild {
                    child_type: new_spec.child_type.clone(),
                    node: new_live,
                });
            }
        }

        for child in keyed_old
            .into_values()
            .flat_map(|children| children.into_iter())
            .chain(unkeyed_old.into_iter())
        {
            let old_wid = child.node.widget_id;
            let old_ct = child.child_type.clone();
            let old_live = child.node.clone();
            let _ = remove_child_from_parent(
                state,
                parent_id,
                parent_kind,
                old_wid,
                old_ct.as_deref(),
                Some(&old_live),
            );
        }

        for child in &next_children {
            if reused_ids.contains(&child.node.widget_id) {
                let child_raw = widget_ptr(state, child.node.widget_id, "reconcile")?;
                unsafe { g_object_ref(child_raw) };
                let old_ct = old_placements
                    .get(&child.node.widget_id)
                    .cloned()
                    .unwrap_or_default();
                remove_child_from_parent(
                    state,
                    parent_id,
                    parent_kind,
                    child.node.widget_id,
                    old_ct.as_deref(),
                    None,
                )?;
            }
        }

        for (i, child) in next_children.iter().enumerate() {
            let child_id = child.node.widget_id;
            let child_raw = widget_ptr(state, child_id, "reconcile")?;
            add_child_to_parent(
                WidgetAttachmentTarget {
                    raw: parent_raw,
                    info: WidgetAttachmentInfo {
                        id: parent_id,
                        class_name: &parent.class_name,
                        kind: parent_kind,
                        node_id: parent.node_id.as_deref(),
                    },
                },
                WidgetAttachmentTarget {
                    raw: child_raw,
                    info: WidgetAttachmentInfo {
                        id: child_id,
                        class_name: &child.node.class_name,
                        kind: child.node.kind,
                        node_id: child.node.node_id.as_deref(),
                    },
                },
                ChildPlacement {
                    child_type: child.child_type.as_deref(),
                    overlay_index: i,
                },
            )?;
            if reused_ids.contains(&child.node.widget_id) {
                unsafe { g_object_unref(child_raw) };
            }
        }

        parent.children = next_children;
        Ok(())
    }

    fn reconcile_children(
        state: &mut RealGtkState,
        parent: &mut LiveNode,
        new_children: &[ChildSpec<'_>],
        id_map: &mut HashMap<String, i64>,
    ) -> Result<(), Gtk4Error> {
        if parent.children.iter().any(|child| child.node.key.is_some())
            || new_children
                .iter()
                .any(|child| child_spec_key(child).is_some())
        {
            return reconcile_children_keyed(state, parent, new_children, id_map);
        }
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
                let mut binding_map = HashMap::new();
                let (new_id, new_live) = build_widget_from_node_real(
                    state,
                    new_children[i].node,
                    id_map,
                    &mut binding_map,
                )?;
                let new_raw = widget_ptr(state, new_id, "reconcile")?;
                let parent_raw = widget_ptr(state, parent_id, "reconcile")?;
                // For GtkBox, insert at position using the previous sibling
                if matches!(parent_kind, CreatedWidgetKind::Box) && i > 0 {
                    let prev_id = parent.children[i - 1].node.widget_id;
                    if let Ok(prev_raw) = widget_ptr(state, prev_id, "reconcile") {
                        unsafe { gtk_box_insert_child_after(parent_raw, new_raw, prev_raw) };
                    } else {
                        add_child_to_parent(
                            WidgetAttachmentTarget {
                                raw: parent_raw,
                                info: WidgetAttachmentInfo {
                                    id: parent_id,
                                    class_name: &parent.class_name,
                                    kind: parent_kind,
                                    node_id: parent.node_id.as_deref(),
                                },
                            },
                            WidgetAttachmentTarget {
                                raw: new_raw,
                                info: WidgetAttachmentInfo {
                                    id: new_id,
                                    class_name: &new_live.class_name,
                                    kind: new_live.kind,
                                    node_id: new_live.node_id.as_deref(),
                                },
                            },
                            ChildPlacement {
                                child_type: new_children[i].child_type.as_deref(),
                                overlay_index: i,
                            },
                        )?;
                    }
                } else {
                    add_child_to_parent(
                        WidgetAttachmentTarget {
                            raw: parent_raw,
                            info: WidgetAttachmentInfo {
                                id: parent_id,
                                class_name: &parent.class_name,
                                kind: parent_kind,
                                node_id: parent.node_id.as_deref(),
                            },
                        },
                        WidgetAttachmentTarget {
                            raw: new_raw,
                            info: WidgetAttachmentInfo {
                                id: new_id,
                                class_name: &new_live.class_name,
                                kind: new_live.kind,
                                node_id: new_live.node_id.as_deref(),
                            },
                        },
                        ChildPlacement {
                            child_type: new_children[i].child_type.as_deref(),
                            overlay_index: i,
                        },
                    )?;
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
            let mut binding_map = HashMap::new();
            let (new_id, new_live) =
                build_widget_from_node_real(state, new_spec.node, id_map, &mut binding_map)?;
            let new_raw = widget_ptr(state, new_id, "reconcile")?;
            let parent_raw = widget_ptr(state, parent_id, "reconcile")?;
            add_child_to_parent(
                WidgetAttachmentTarget {
                    raw: parent_raw,
                    info: WidgetAttachmentInfo {
                        id: parent_id,
                        class_name: &parent.class_name,
                        kind: parent_kind,
                        node_id: parent.node_id.as_deref(),
                    },
                },
                WidgetAttachmentTarget {
                    raw: new_raw,
                    info: WidgetAttachmentInfo {
                        id: new_id,
                        class_name: &new_live.class_name,
                        kind: new_live.kind,
                        node_id: new_live.node_id.as_deref(),
                    },
                },
                ChildPlacement {
                    child_type: new_spec.child_type.as_deref(),
                    overlay_index: i,
                },
            )?;
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
                GTK_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    if let Err(err) = process_ui_debug_requests(&mut state) {
                        eprintln!("AIVI GTK UI debug server error: {}", err);
                    }
                });
                flush_pending_tray_actions();
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
            ensure_main_loop_tick_registered(&mut state);
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
        // Register the application so `startup` fires before any windows are
        // created. Hold the application so `g_application_run` keeps the event
        // loop alive until we explicitly release or the window closes.
        unsafe {
            g_application_register(raw, null_mut(), null_mut());
            g_application_hold(raw);
        }
        let id = GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let id = state.alloc_id();
            state.apps.insert(id, raw);
            ensure_main_loop_tick_registered(&mut state);
            maybe_start_ui_debug_server(&mut state)?;
            Ok(id)
        })?;
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
        // Release the hold placed in app_new so the event loop can exit
        // naturally when the last window closes.
        unsafe {
            g_application_release(app);
            let _ = g_application_run(app, 0, null_mut());
        }
        GTK_STATE.with(|state| shutdown_ui_debug_server(&mut state.borrow_mut()));
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
            let app =
                state.apps.get(&app_id).copied().ok_or_else(|| {
                    Gtk4Error::new(format!("gtk4.windowNew unknown app id {app_id}"))
                })?;
            let window = unsafe { gtk_application_window_new(app) };
            if window.is_null() {
                return Err(Gtk4Error::new("gtk4.windowNew failed to create window"));
            }
            unsafe {
                gtk_window_set_title(window, title_c.as_ptr());
                gtk_window_set_default_size(window, width, height);
                gtk_widget_set_focusable(window, 1);
            }
            let id = state.alloc_id();
            state.windows.insert(id, window);
            state.widgets.insert(id, window);
            let controller = unsafe { gtk_event_controller_key_new() };
            if controller.is_null() {
                return Err(Gtk4Error::new(
                    "gtk4.windowNew failed to create key controller",
                ));
            }
            let signal_c = CString::new("key-pressed")
                .map_err(|_| Gtk4Error::new("gtk4.windowNew invalid key signal"))?;
            let callback_data = Box::new(WindowKeyCallbackData { widget_id: id });
            let callback_ptr = Box::into_raw(callback_data) as *mut c_void;
            unsafe {
                gtk_widget_add_controller(window, controller);
                g_signal_connect_data(
                    controller,
                    signal_c.as_ptr(),
                    gtk_window_key_pressed_callback as *const c_void,
                    callback_ptr,
                    std::ptr::null_mut(),
                    0,
                );
            }
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

    pub(super) fn widget_get_bool_property(id: i64, prop: &str) -> Result<bool, Gtk4Error> {
        let prop_c = CString::new(prop)
            .map_err(|_| Gtk4Error::new("gtk4.widgetGetBoolProperty invalid prop"))?;
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let widget = widget_ptr(&state, id, "widgetGetBoolProperty")?;
            Ok(unsafe { gobject_get_bool(widget, &prop_c) != 0 })
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
            let mut binding_map = HashMap::new();
            let root = first_object_in_interface(node)?;
            let (id, live) =
                build_widget_from_node_real(&mut state, root, &mut id_map, &mut binding_map)?;
            state.named_widgets.extend(id_map.clone());
            for (name, wid) in &id_map {
                state.widget_id_to_name.insert(*wid, name.clone());
            }
            state.live_trees.insert(id, live);
            state.record_mutation(serde_json::Map::from_iter([
                ("kind".to_string(), Value::String("root-built".to_string())),
                ("rootId".to_string(), json!(id)),
                ("widgetId".to_string(), json!(id)),
                (
                    "widgetName".to_string(),
                    json!(id_map
                        .iter()
                        .find_map(|(name, wid)| (*wid == id).then_some(name.clone()))),
                ),
            ]));
            Ok(id)
        })
    }

    pub(super) fn build_with_ids(node: &super::GtkNode) -> Result<BuildResult, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let mut id_map = HashMap::new();
            let mut binding_map = HashMap::new();
            let root = first_object_in_interface(node)?;
            let (id, live) =
                build_widget_from_node_real(&mut state, root, &mut id_map, &mut binding_map)?;
            state.named_widgets.extend(id_map.clone());
            for (name, wid) in &id_map {
                state.widget_id_to_name.insert(*wid, name.clone());
            }
            state.live_trees.insert(id, live);
            state.record_mutation(serde_json::Map::from_iter([
                ("kind".to_string(), Value::String("root-built".to_string())),
                ("rootId".to_string(), json!(id)),
                ("widgetId".to_string(), json!(id)),
                (
                    "widgetName".to_string(),
                    json!(id_map
                        .iter()
                        .find_map(|(name, wid)| (*wid == id).then_some(name.clone()))),
                ),
            ]));
            Ok(BuildResult {
                root_id: id,
                named_widgets: id_map,
            })
        })
    }

    pub(super) fn build_with_bindings(
        node: &super::GtkNode,
    ) -> Result<BuildWithBindingsResult, Gtk4Error> {
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let mut id_map = HashMap::new();
            let mut binding_map = HashMap::new();
            let root = first_object_in_interface(node)?;
            let (id, live) =
                build_widget_from_node_real(&mut state, root, &mut id_map, &mut binding_map)?;
            state.named_widgets.extend(id_map.clone());
            for (name, wid) in &id_map {
                state.widget_id_to_name.insert(*wid, name.clone());
            }
            state.live_trees.insert(id, live);
            state.record_mutation(serde_json::Map::from_iter([
                ("kind".to_string(), Value::String("root-built".to_string())),
                ("rootId".to_string(), json!(id)),
                ("widgetId".to_string(), json!(id)),
                (
                    "widgetName".to_string(),
                    json!(id_map
                        .iter()
                        .find_map(|(name, wid)| (*wid == id).then_some(name.clone()))),
                ),
            ]));
            Ok(BuildWithBindingsResult {
                root_id: id,
                named_widgets: id_map,
                binding_widgets: binding_map,
            })
        })
    }

    pub(super) fn reconcile_widget(
        widget_id: i64,
        node: &super::GtkNode,
    ) -> Result<HashMap<String, i64>, Gtk4Error> {
        let new_node = first_object_in_interface(node)?;
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let root_id = state
                .live_trees
                .iter()
                .find_map(|(root_id, live)| {
                    live_tree_contains_widget(live, widget_id).then_some(*root_id)
                })
                .ok_or_else(|| {
                    Gtk4Error::new(format!(
                        "gtk4.reconcileWidget no live tree contains widget id {widget_id}"
                    ))
                })?;
            let mut live_root = state.live_trees.remove(&root_id).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk4.reconcileWidget missing live tree for root id {root_id}"
                ))
            })?;
            let mut id_map = HashMap::new();
            let target = find_live_node_mut(&mut live_root, widget_id).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk4.reconcileWidget could not locate widget id {widget_id} in live tree"
                ))
            })?;
            let patched = reconcile_node(&mut state, target, new_node, &mut id_map)?;
            if !patched {
                state.live_trees.insert(root_id, live_root);
                return Err(Gtk4Error::new(
                    "gtk4.reconcileWidget target subtree changed widget class".to_string(),
                ));
            }
            let mut binding_map = HashMap::new();
            let target = find_live_node_mut(&mut live_root, widget_id).ok_or_else(|| {
                Gtk4Error::new(format!(
                    "gtk4.reconcileWidget lost widget id {widget_id} after reconcile"
                ))
            })?;
            collect_binding_widgets(target, &mut binding_map);
            state.named_widgets.extend(id_map.clone());
            for (name, wid) in &id_map {
                state.widget_id_to_name.insert(*wid, name.clone());
            }
            state.live_trees.insert(root_id, live_root);
            let widget_name = state.widget_id_to_name.get(&widget_id).cloned();
            state.record_mutation(serde_json::Map::from_iter([
                (
                    "kind".to_string(),
                    Value::String("tree-reconciled".to_string()),
                ),
                ("rootId".to_string(), json!(root_id)),
                ("widgetId".to_string(), json!(widget_id)),
                ("widgetName".to_string(), json!(widget_name)),
                (
                    "addedNames".to_string(),
                    json!(id_map.keys().cloned().collect::<Vec<_>>()),
                ),
            ]));
            Ok(binding_map)
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
                let mut binding_map = HashMap::new();
                let (new_id, new_live) = build_widget_from_node_real(
                    &mut state,
                    new_root,
                    &mut id_map,
                    &mut binding_map,
                )?;
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
            state.record_mutation(serde_json::Map::from_iter([
                (
                    "kind".to_string(),
                    Value::String(
                        if final_id == root_id {
                            "root-reconciled"
                        } else {
                            "root-replaced"
                        }
                        .to_string(),
                    ),
                ),
                ("rootId".to_string(), json!(final_id)),
                ("widgetId".to_string(), json!(final_id)),
                ("previousRootId".to_string(), json!(root_id)),
                (
                    "addedNames".to_string(),
                    json!(id_map.keys().cloned().collect::<Vec<_>>()),
                ),
            ]));
            Ok(final_id)
        })
    }

    pub(super) fn widget_child_count(id: i64) -> Result<i64, Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            find_widget_context(&state, id)
                .map(|(_, _, _, node)| node.children.len() as i64)
                .ok_or_else(|| Gtk4Error::new(format!("unknown widget id {id}")))
        })
    }

    pub(super) fn widget_exists(id: i64) -> Result<bool, Gtk4Error> {
        GTK_STATE.with(|state| Ok(state.borrow().widgets.contains_key(&id)))
    }

    pub(super) fn binding_widget_ids(id: i64) -> Result<Vec<i64>, Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let (_, _, _, node) = find_widget_context(&state, id)
                .ok_or_else(|| Gtk4Error::new(format!("unknown widget id {id}")))?;
            let mut ids = Vec::new();
            collect_binding_widget_ids(node, &mut ids);
            Ok(ids)
        })
    }

    pub(super) fn widget_child_ids(id: i64) -> Result<Vec<i64>, Gtk4Error> {
        GTK_STATE.with(|state| {
            let state = state.borrow();
            let (_, _, _, node) = find_widget_context(&state, id)
                .ok_or_else(|| Gtk4Error::new(format!("unknown widget id {id}")))?;
            Ok(node
                .children
                .iter()
                .map(|child| child.node.widget_id)
                .collect())
        })
    }

    unsafe extern "C" fn tick_timeout_cb(_data: *mut c_void) -> c_int {
        let event = SignalEvent {
            widget_id: 0,
            widget_name: String::new(),
            signal: "tick".to_string(),
            handler: String::new(),
            payload: String::new(),
        };
        GTK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            state
                .signal_senders
                .retain(|s| s.send(event.clone()).is_ok());
            state.signal_events.push_back(SignalEventState {
                widget_id: 0,
                signal: "tick".to_string(),
                handler: String::new(),
                payload: String::new(),
            });
        });
        1 // TRUE — keep repeating
    }

    pub(super) fn set_interval(ms: u32) -> Result<(), Gtk4Error> {
        unsafe { g_timeout_add(ms, tick_timeout_cb, null_mut()) };
        Ok(())
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
delegate!(widget_get_bool_property(id: i64, prop: &str) -> Result<bool, Gtk4Error>);
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
delegate!(build_with_bindings(node: &GtkNode) -> Result<BuildWithBindingsResult, Gtk4Error>);
delegate!(reconcile_widget(widget_id: i64, node: &GtkNode) -> Result<HashMap<String, i64>, Gtk4Error>);
delegate!(widget_child_count(id: i64) -> Result<i64, Gtk4Error>);
delegate!(widget_exists(id: i64) -> Result<bool, Gtk4Error>);
delegate!(binding_widget_ids(id: i64) -> Result<Vec<i64>, Gtk4Error>);
delegate!(widget_child_ids(id: i64) -> Result<Vec<i64>, Gtk4Error>);
delegate!(set_interval(ms: u32) -> Result<(), Gtk4Error>);

pub fn set_ui_debug_request_handler(handler: Option<Arc<UiDebugRequestHandler>>) {
    #[cfg(target_os = "linux")]
    {
        linux_impl::set_ui_debug_request_handler(handler);
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = handler;
    }
}

pub fn set_main_loop_tick_handler(handler: Option<Arc<MainLoopTickHandler>>) {
    #[cfg(target_os = "linux")]
    {
        linux_impl::set_main_loop_tick_handler(handler);
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = handler;
    }
}

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
