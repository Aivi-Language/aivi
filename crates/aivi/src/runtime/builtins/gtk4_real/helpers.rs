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

    fn apply_pending_display_customizations(state: &mut RealGtkState) -> Result<(), RuntimeError> {
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

    fn widget_ptr(state: &RealGtkState, id: i64, ctx: &str) -> Result<*mut c_void, RuntimeError> {
        state.widgets.get(&id).copied().ok_or_else(|| {
            RuntimeError::Error(Value::Text(format!("gtk4.{ctx} unknown widget id {id}")))
        })
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

    fn collect_text(children: &[GtkBuilderNode]) -> String {
        let mut out = String::new();
        for child in children {
            if let GtkBuilderNode::Text(text) = child {
                out.push_str(text);
            }
        }
        out.trim().to_string()
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
            .unwrap_or_else(|| serialize_signal_value(&args[1]));
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
            let Ok(name) = CString::new(lib_name) else { continue; };
            let handle = unsafe { dlopen(name.as_ptr(), RTLD_NOW | RTLD_NODELETE) };
            if handle.is_null() { continue; }
            let Ok(sym) = CString::new(fn_name) else { break; };
            let ptr = unsafe { dlsym(handle, sym.as_ptr()) };
            if !ptr.is_null() {
                let f: unsafe extern "C" fn(*mut c_void, *mut c_void) = unsafe { std::mem::transmute(ptr) };
                unsafe { f(arg0, arg1) };
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
