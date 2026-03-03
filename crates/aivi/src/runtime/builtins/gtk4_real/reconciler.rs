    // ── Reconciliation ────────────────────────────────────────────────────────

    /// Patch CSS classes: remove old-only, add new-only.
    fn patch_css_classes(
        widget: *mut c_void,
        old_css: Option<&String>,
        new_css: Option<&String>,
    ) -> Result<(), RuntimeError> {
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
    ) -> Result<(), RuntimeError> {
        // Diff CSS classes specially
        patch_css_classes(widget, old_props.get("css-class"), new_props.get("css-class"))?;
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
    ) -> Result<(), RuntimeError> {
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
                Some("title") => unsafe {
                    gtk_header_bar_set_title_widget(parent_raw, child_raw)
                },
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
            CreatedWidgetKind::Revealer => unsafe {
                gtk_revealer_set_child(parent_raw, child_raw)
            },
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
                unsafe {
                    g_object_set(
                        parent_raw,
                        prop_c.as_ptr(),
                        child_raw,
                        std::ptr::null::<c_char>(),
                    );
                }
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

    /// Reconcile a single live node against a new GtkBuilderNode.
    /// Returns the (possibly replaced) LiveNode.
    fn reconcile_node(
        state: &mut RealGtkState,
        live: &mut LiveNode,
        new_node: &GtkBuilderNode,
        id_map: &mut HashMap<String, i64>,
    ) -> Result<bool, RuntimeError> {
        let GtkBuilderNode::Element {
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

        // Patch properties
        patch_widget_properties(raw, new_class, &live.props, &new_props, state)?;
        live.props = new_props;

        // Patch signals: disconnect old, connect new if changed
        if live.signals != new_signals {
            for &hid in &live.signal_handler_ids {
                if hid != 0 {
                    unsafe { g_signal_handler_disconnect(raw, hid) };
                }
            }
            let mut new_handler_ids = Vec::new();
            for binding in &new_signals {
                let hid = connect_widget_signal(raw, live.widget_id, new_class, binding)?;
                new_handler_ids.push(hid);
            }
            live.signals = new_signals;
            live.signal_handler_ids = new_handler_ids;
        }

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
    ) -> Result<(), RuntimeError> {
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
                remove_child_from_parent(state, parent_id, parent_kind, old_wid, old_ct.as_deref(), Some(&old_live))?;
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
                        add_child_to_parent(parent_raw, parent_kind, new_raw, new_children[i].child_type.as_deref(), i);
                    }
                } else {
                    add_child_to_parent(parent_raw, parent_kind, new_raw, new_children[i].child_type.as_deref(), i);
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
