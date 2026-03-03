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
        signal_senders: Vec<mpsc::SyncSender<Value>>,
        signal_bool_bindings: HashMap<String, Vec<SignalBoolBinding>>,
        signal_css_bindings: HashMap<String, Vec<SignalCssBinding>>,
        signal_toggle_bool_bindings: HashMap<String, Vec<SignalToggleBoolBinding>>,
        signal_toggle_css_bindings: HashMap<String, Vec<SignalToggleCssBinding>>,
        signal_dialog_bindings: HashMap<String, Vec<SignalDialogBinding>>,
        signal_stack_page_bindings: HashMap<String, Vec<SignalStackPageBinding>>,
        named_widgets: HashMap<String, i64>,
        widget_id_to_name: HashMap<i64, String>,
        tray_handles: HashMap<i64, Arc<Mutex<SniTrayState>>>,
        pending_icon_search_paths: Vec<String>,
        pending_css_texts: Vec<String>,
        resources_registered: bool,
        /// Root widget id → LiveNode tree for reconciliation.
        live_trees: HashMap<i64, LiveNode>,
    }

    struct SignalBoolBinding {
        widget_id: i64,
        property: String,
        value: bool,
    }

    struct SignalCssBinding {
        widget_id: i64,
        class_name: String,
        add: bool,
    }

    struct SignalToggleBoolBinding {
        widget_id: i64,
        property: String,
    }

    struct SignalToggleCssBinding {
        widget_id: i64,
        class_name: String,
    }

    struct SignalDialogBinding {
        dialog_id: i64,
        parent_id: i64,
    }

    struct SignalStackPageBinding {
        stack_id: i64,
        page_name: String,
    }

    struct SniTrayState {
        icon_name: String,
        tooltip: String,
        visible: bool,
        menu_items: Vec<(String, String)>,
    }

    impl Default for SniTrayState {
        fn default() -> Self {
            Self {
                icon_name: String::new(),
                tooltip: String::new(),
                visible: true,
                menu_items: Vec::new(),
            }
        }
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
        node: &'a GtkBuilderNode,
        child_type: Option<String>,
        position: Option<usize>,
    }
