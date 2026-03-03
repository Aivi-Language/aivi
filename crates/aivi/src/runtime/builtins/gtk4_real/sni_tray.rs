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
                    let sni = SniObject { state: state_clone.clone() };
                    if let Err(e) = conn.object_server().at(obj_path, sni).await {
                        eprintln!("sni-tray: object_server error: {e}");
                        return;
                    }
                    // Register DBusMenu at /MenuBar for GNOME AppIndicator compatibility
                    let dbus_menu = DbusMenuObject { tray_state: state_clone };
                    if let Err(e) = conn.object_server().at("/MenuBar", dbus_menu).await {
                        eprintln!("sni-tray: dbusmenu register error: {e}");
                    }
                    if let Err(e) = conn.request_name(bus_name.as_str()).await {
                        eprintln!("sni-tray: request_name error: {e}");
                        return;
                    }
                    
                    let mailfox_dbus = MailfoxDesktopObject;
                    if let Err(e) = conn.object_server().at("/com/mailfox/desktop", mailfox_dbus).await {
                        eprintln!("sni-tray: mailfox desktop register error: {e}");
                    }
                    if let Err(e) = conn.request_name("com.mailfox.desktop.tray").await {
                        eprintln!("sni-tray: request_name mailfox desktop tray error: {e}");
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
                    // Poll for menu-items changes and emit LayoutUpdated to refresh GNOME cache.
                    let conn_signal = conn.clone();
                    tokio::spawn(async move {
                        let mut revision: u32 = 1;
                        loop {
                            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                            let should_update = pending_layout_update()
                                .lock()
                                .map(|mut f| { let v = *f; *f = false; v })
                                .unwrap_or(false);
                            if should_update {
                                revision += 1;
                                if let Ok(iface_ref) = conn_signal
                                    .object_server()
                                    .interface::<_, DbusMenuObject>("/MenuBar")
                                    .await
                                {
                                    let _ = DbusMenuObject::layout_updated(
                                        iface_ref.signal_emitter(),
                                        revision,
                                        0i32,
                                    )
                                    .await;
                                }
                            }
                        }
                    });
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
        #[allow(clippy::type_complexity)]
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
            true
        }

        #[zbus(property)]
        fn menu(&self) -> zbus::zvariant::OwnedObjectPath {
            zbus::zvariant::OwnedObjectPath::try_from("/MenuBar")
                .unwrap_or_else(|_| zbus::zvariant::OwnedObjectPath::try_from("/").unwrap())
        }

        fn activate(&self, x: i32, y: i32) {
            eprintln!("sni-tray: Activate({x}, {y}) called");
            if let Ok(mut q) = pending_tray_actions().lock() {
                q.push_back(format!("left_click:{x}:{y}"));
            }
        }
        fn secondary_activate(&self, _x: i32, _y: i32) {}
        fn context_menu(&self, x: i32, y: i32) {
            eprintln!("sni-tray: ContextMenu({x}, {y}) called");
            if let Ok(mut q) = pending_tray_actions().lock() {
                q.push_back(format!("context_menu:{x}:{y}"));
            }
        }
        fn scroll(&self, _delta: i32, _orientation: &str) {}
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
    }

    // ── DBusMenu (com.canonical.dbusmenu) ─────────────────────────────────────
    // Provides the right-click context menu to GNOME AppIndicator and compatible
    // tray hosts. Items are stored in SniTrayState and reflected here.

    type ItemProps = std::collections::HashMap<String, zbus::zvariant::OwnedValue>;
    type DMenuItem = (i32, ItemProps, Vec<zbus::zvariant::OwnedValue>);

    struct DbusMenuObject {
        tray_state: Arc<Mutex<SniTrayState>>,
    }

    #[zbus::interface(name = "com.canonical.dbusmenu")]
    impl DbusMenuObject {
        #[zbus(property)]
        fn version(&self) -> u32 {
            3
        }

        #[zbus(property)]
        fn text_direction(&self) -> &str {
            "ltr"
        }

        #[zbus(property)]
        fn status(&self) -> &str {
            "normal"
        }

        #[zbus(property)]
        fn icon_theme_path(&self) -> Vec<String> {
            vec![]
        }

        fn get_layout(
            &self,
            _parent_id: i32,
            _recursion_depth: i32,
            _property_names: Vec<String>,
        ) -> (u32, DMenuItem) {
            use zbus::zvariant::{OwnedValue, Str, StructureBuilder};

            let items = self
                .tray_state
                .lock()
                .map(|s| s.menu_items.clone())
                .unwrap_or_default();

            let children: Vec<OwnedValue> = items
                .iter()
                .enumerate()
                .filter_map(|(i, (label, action))| {
                    let id = (i + 1) as i32;
                    let mut props = ItemProps::new();
                    props.insert(
                        "label".to_string(),
                        OwnedValue::from(Str::from(label.as_str())),
                    );
                    props.insert("enabled".to_string(), OwnedValue::from(true));
                    props.insert("visible".to_string(), OwnedValue::from(true));
                    // Store action in accessible-desc so Event can look it up by id
                    let _ = action; // looked up in event() by id
                    let item: DMenuItem = (id, props, vec![]);
                    let s = StructureBuilder::new()
                        .add_field(item.0)
                        .add_field(item.1)
                        .add_field(item.2)
                        .build()
                        .ok()?;
                    OwnedValue::try_from(s).ok()
                })
                .collect();

            let mut root_props = ItemProps::new();
            root_props.insert(
                "children-display".to_string(),
                OwnedValue::from(Str::from_static("submenu")),
            );

            (1u32, (0i32, root_props, children))
        }

        fn event(
            &self,
            id: i32,
            event_id: &str,
            _data: zbus::zvariant::OwnedValue,
            _timestamp: u32,
        ) -> zbus::fdo::Result<()> {
            if event_id == "clicked" {
                let items = self
                    .tray_state
                    .lock()
                    .map(|s| s.menu_items.clone())
                    .unwrap_or_default();
                let idx = (id - 1) as usize;
                if let Some((_label, action)) = items.get(idx) {
                    if let Ok(mut q) = pending_tray_actions().lock() {
                        q.push_back(action.clone());
                    }
                }
            }
            Ok(())
        }

        fn event_group(
            &self,
            events: Vec<(i32, String, zbus::zvariant::OwnedValue, u32)>,
        ) -> zbus::fdo::Result<Vec<i32>> {
            for (id, event_id, data, timestamp) in events {
                let _ = self.event(id, &event_id, data, timestamp);
            }
            Ok(vec![])
        }

        fn about_to_show(&self, _id: i32) -> bool {
            false
        }

        fn about_to_show_group(&self, _ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
            (vec![], vec![])
        }

        fn get_group_properties(
            &self,
            _ids: Vec<i32>,
            _property_names: Vec<String>,
        ) -> Vec<(i32, ItemProps)> {
            vec![]
        }

        fn get_property(
            &self,
            _id: i32,
            _name: &str,
        ) -> zbus::zvariant::OwnedValue {
            zbus::zvariant::OwnedValue::from(0i32)
        }

        #[zbus(signal)]
        async fn layout_updated(
            ctxt: &zbus::object_server::SignalEmitter<'_>,
            revision: u32,
            parent: i32,
        ) -> zbus::Result<()>;

        #[zbus(signal)]
        async fn item_activation_requested(
            ctxt: &zbus::object_server::SignalEmitter<'_>,
            id: i32,
            timestamp: u32,
        ) -> zbus::Result<()>;

        #[zbus(signal)]
        async fn items_properties_updated(
            ctxt: &zbus::object_server::SignalEmitter<'_>,
            updated_props: Vec<(i32, ItemProps)>,
            removed_props: Vec<(i32, Vec<String>)>,
        ) -> zbus::Result<()>;
    }
