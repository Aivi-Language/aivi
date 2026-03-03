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
        state: &RealGtkState,
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
        if let Some(value) = props.get("sensitive").and_then(|v| parse_bool_text(v)) {
            unsafe { gtk_widget_set_sensitive(widget, if value { 1 } else { 0 }) };
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
            "GtkEntry" | "GtkPasswordEntry" => {
                if let Some(value) = props.get("text") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid GtkEntry text")?;
                    unsafe { gtk_editable_set_text(widget, text_c.as_ptr()) };
                }
                if let Some(value) = props.get("placeholder-text") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid placeholder-text")?;
                    let prop_c = CString::new("placeholder-text").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), text_c.as_ptr(), std::ptr::null::<c_char>()) };
                }
                if class_name == "GtkPasswordEntry" {
                    if let Some(value) = props.get("show-peek-icon").and_then(|v| parse_bool_text(v)) {
                        unsafe { gtk_password_entry_set_show_peek_icon(widget, if value { 1 } else { 0 }) };
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
                    unsafe { gtk_text_view_set_editable(widget, if value { 1 } else { 0 }) };
                }
                if let Some(value) = props.get("cursor-visible").and_then(|v| parse_bool_text(v)) {
                    unsafe { gtk_text_view_set_cursor_visible(widget, if value { 1 } else { 0 }) };
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
            "AdwOverlaySplitView" => {
                if let Some(value) = props.get("sidebar-position") {
                    let pos: c_int = if value == "end" { 1 } else { 0 };
                    let prop_c = CString::new("sidebar-position").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), pos, std::ptr::null::<c_char>()) };
                }
                if let Some(value) = props.get("collapsed").and_then(|v| parse_bool_text(v)) {
                    let prop_c = CString::new("collapsed").unwrap();
                    let v: c_int = if value { 1 } else { 0 };
                    unsafe { g_object_set(widget, prop_c.as_ptr(), v, std::ptr::null::<c_char>()) };
                }
                if let Some(value) = props.get("show-sidebar").and_then(|v| parse_bool_text(v)) {
                    let prop_c = CString::new("show-sidebar").unwrap();
                    let v: c_int = if value { 1 } else { 0 };
                    unsafe { g_object_set(widget, prop_c.as_ptr(), v, std::ptr::null::<c_char>()) };
                }
                if let Some(value) = props.get("max-sidebar-width").and_then(|v| parse_f64_text(v)) {
                    let prop_c = CString::new("max-sidebar-width").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), value, std::ptr::null::<c_char>()) };
                }
                if let Some(value) = props.get("min-sidebar-width").and_then(|v| parse_f64_text(v)) {
                    let prop_c = CString::new("min-sidebar-width").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), value, std::ptr::null::<c_char>()) };
                }
                if let Some(value) = props.get("sidebar-width-fraction").and_then(|v| parse_f64_text(v)) {
                    let prop_c = CString::new("sidebar-width-fraction").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), value, std::ptr::null::<c_char>()) };
                }
            }
            "AdwButtonContent" => {
                if let Some(value) = props.get("label") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid AdwButtonContent label")?;
                    let prop_c = CString::new("label").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), text_c.as_ptr(), std::ptr::null::<c_char>()) };
                }
                if let Some(value) = props.get("icon-name") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid AdwButtonContent icon-name")?;
                    let prop_c = CString::new("icon-name").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), text_c.as_ptr(), std::ptr::null::<c_char>()) };
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
                if let Some(value) = props.get("transition-duration").and_then(|v| v.parse::<u32>().ok()) {
                    unsafe { gtk_revealer_set_transition_duration(widget, value) };
                }
                if let Some(value) = props.get("reveal-child").and_then(|v| parse_bool_text(v)) {
                    unsafe { gtk_revealer_set_reveal_child(widget, if value { 1 } else { 0 }) };
                }
            }
            "GtkMenuButton" => {                if let Some(value) = props.get("label") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid GtkMenuButton label")?;
                    let prop_c = CString::new("label").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), text_c.as_ptr(), std::ptr::null::<c_char>()) };
                }
                if let Some(value) = props.get("icon-name") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid GtkMenuButton icon-name")?;
                    let prop_c = CString::new("icon-name").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), text_c.as_ptr(), std::ptr::null::<c_char>()) };
                }
                if let Some(id_str) = props.get("menu-model") {
                    if let Ok(id) = id_str.parse::<i64>() {
                        if let Some(&menu_raw) = state.widgets.get(&id) {
                            let prop_c = CString::new("menu-model").unwrap();
                            unsafe { g_object_set(widget, prop_c.as_ptr(), menu_raw, std::ptr::null::<c_char>()) };
                        }
                    }
                }
            }
            "AdwPreferencesDialog" => {
                if let Some(value) = props.get("title") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid AdwPreferencesDialog title")?;
                    let prop_c = CString::new("title").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), text_c.as_ptr(), std::ptr::null::<c_char>()) };
                }
            }
            "AdwPreferencesPage" => {
                if let Some(value) = props.get("title") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid AdwPreferencesPage title")?;
                    let prop_c = CString::new("title").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), text_c.as_ptr(), std::ptr::null::<c_char>()) };
                }
                if let Some(value) = props.get("icon-name") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid AdwPreferencesPage icon-name")?;
                    let prop_c = CString::new("icon-name").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), text_c.as_ptr(), std::ptr::null::<c_char>()) };
                }
                if let Some(value) = props.get("name") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid AdwPreferencesPage name")?;
                    let prop_c = CString::new("name").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), text_c.as_ptr(), std::ptr::null::<c_char>()) };
                }
            }
            "AdwPreferencesGroup" => {
                if let Some(value) = props.get("title") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid AdwPreferencesGroup title")?;
                    let prop_c = CString::new("title").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), text_c.as_ptr(), std::ptr::null::<c_char>()) };
                }
                if let Some(value) = props.get("description") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid AdwPreferencesGroup description")?;
                    let prop_c = CString::new("description").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), text_c.as_ptr(), std::ptr::null::<c_char>()) };
                }
            }
            "AdwActionRow" | "AdwExpanderRow" | "AdwPreferencesRow" | "AdwSpinRow" => {
                if let Some(value) = props.get("title") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid AdwActionRow title")?;
                    let prop_c = CString::new("title").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), text_c.as_ptr(), std::ptr::null::<c_char>()) };
                }
                if let Some(value) = props.get("subtitle") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid AdwActionRow subtitle")?;
                    let prop_c = CString::new("subtitle").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), text_c.as_ptr(), std::ptr::null::<c_char>()) };
                }
            }
            "AdwEntryRow" | "AdwPasswordEntryRow" => {
                if let Some(value) = props.get("title") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid AdwEntryRow title")?;
                    let prop_c = CString::new("title").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), text_c.as_ptr(), std::ptr::null::<c_char>()) };
                }
                if let Some(value) = props.get("text") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid AdwEntryRow text")?;
                    unsafe { gtk_editable_set_text(widget, text_c.as_ptr()) };
                }
            }
            "AdwSwitchRow" => {
                if let Some(value) = props.get("title") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid AdwSwitchRow title")?;
                    let prop_c = CString::new("title").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), text_c.as_ptr(), std::ptr::null::<c_char>()) };
                }
                if let Some(value) = props.get("subtitle") {
                    let text_c = c_text(value, "gtk4.buildFromNode invalid AdwSwitchRow subtitle")?;
                    let prop_c = CString::new("subtitle").unwrap();
                    unsafe { g_object_set(widget, prop_c.as_ptr(), text_c.as_ptr(), std::ptr::null::<c_char>()) };
                }
                if let Some(value) = props.get("active").and_then(|v| parse_bool_text(v)) {
                    let prop_c = CString::new("active").unwrap();
                    let v: c_int = if value { 1 } else { 0 };
                    unsafe { g_object_set(widget, prop_c.as_ptr(), v, std::ptr::null::<c_char>()) };
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
    ) -> Result<(i64, LiveNode), RuntimeError> {
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
            let wid = id_map.get(ref_id).copied().ok_or_else(|| {
                RuntimeError::Error(Value::Text(format!(
                    "gtk4.buildFromNode unresolved object reference id '{ref_id}'"
                )))
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
            "GtkPasswordEntry" => (unsafe { gtk_password_entry_new() }, CreatedWidgetKind::Other),
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
            "AdwPreferencesDialog" => (create_adw_widget(class_name)?, CreatedWidgetKind::PreferencesDialog),
            "AdwPreferencesPage" => (create_adw_widget(class_name)?, CreatedWidgetKind::PreferencesPage),
            "AdwPreferencesGroup" => (create_adw_widget(class_name)?, CreatedWidgetKind::PreferencesGroup),
            "AdwActionRow" => (create_adw_widget(class_name)?, CreatedWidgetKind::ActionRow),
            "AdwExpanderRow" => (create_adw_widget(class_name)?, CreatedWidgetKind::ActionRow),
            | "AdwAboutDialog"
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
            if let Ok(name_c) = CString::new(object_id.as_bytes()) {
                unsafe {
                    // Set the GTK CSS widget name (used for styling/lookup)
                    gtk_widget_set_name(raw, name_c.as_ptr());
                    // For GtkBox (which defaults to GTK_ACCESSIBLE_ROLE_GENERIC),
                    // upgrade to GROUP (18) so the accessible name is exposed by AT-SPI.
                    // GTK_ACCESSIBLE_ROLE_GROUP = 18
                    if matches!(class_name, "GtkBox") {
                        if let Ok(role_prop) = CString::new("accessible-role") {
                            g_object_set(raw, role_prop.as_ptr(), 18i32, std::ptr::null::<c_char>());
                        }
                    }
                    // Set the AT-SPI accessible label so AT-SPI clients can find
                    // widgets by their id. GTK_ACCESSIBLE_PROPERTY_LABEL = 4,
                    // terminated by -1.
                    gtk_accessible_update_property(
                        raw,
                        4i32,                  // GTK_ACCESSIBLE_PROPERTY_LABEL
                        name_c.as_ptr(),       // label value (const char*)
                        -1i32,                 // sentinel
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
                if let GtkBuilderNode::Element { attrs, children: cc, .. } = child.node {
                    let p = collect_object_properties(attrs, cc);
                    p.get("css-class").cloned().unwrap_or_default()
                } else { String::new() }
            } else { String::new() };
            let child_class_name = if matches!(kind, CreatedWidgetKind::Overlay) {
                if let GtkBuilderNode::Element { attrs, .. } = child.node {
                    node_attr(attrs, "class").unwrap_or("").to_string()
                } else { String::new() }
            } else { String::new() };

            let (child_id, child_live) = build_widget_from_node_real(state, child.node, id_map)?;
            let child_raw = widget_ptr(state, child_id, "buildFromNode")?;

            // Track for scroll-fade auto-wiring
            if matches!(kind, CreatedWidgetKind::Overlay) {
                if child_class_name == "GtkScrolledWindow" && child_css.contains("fading-scroll") {
                    scroll_fade_scrolled = child_raw;
                }
                if child_css.contains("fade-top") { scroll_fade_top = child_raw; }
                if child_css.contains("fade-bottom") { scroll_fade_bottom = child_raw; }
            }
            if child.child_type.as_deref() == Some("controller") {
                unsafe { gtk_widget_add_controller(raw, child_raw) };
                if let Some(gesture) = state.gesture_clicks.get_mut(&child_id) {
                    gesture.widget_id = id;
                }
                live_children.push(LiveChild { child_type: child.child_type.clone(), node: child_live });
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
                    unsafe {
                        g_object_set(
                            raw,
                            prop_c.as_ptr(),
                            child_raw,
                            std::ptr::null::<c_char>(),
                        );
                    }
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
            live_children.push(LiveChild { child_type: child.child_type.clone(), node: child_live });
        }

        // Auto-wire scroll fades for GtkOverlay containing a fading-scroll scrolled window.
        if !scroll_fade_scrolled.is_null() && (!scroll_fade_top.is_null() || !scroll_fade_bottom.is_null()) {
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
