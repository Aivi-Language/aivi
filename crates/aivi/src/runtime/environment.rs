use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::{Mutex, RwLock};

use super::{values::Value, ReactiveGraphState};

#[derive(Clone)]
pub(crate) struct Env {
    inner: Arc<EnvInner>,
}

struct EnvInner {
    parent: Option<Env>,
    values: RwLock<HashMap<String, Value>>,
}

impl Env {
    pub(crate) fn new(parent: Option<Env>) -> Self {
        Self {
            inner: Arc::new(EnvInner {
                parent,
                values: RwLock::new(HashMap::new()),
            }),
        }
    }

    pub(crate) fn get(&self, name: &str) -> Option<Value> {
        if let Some(value) = self.inner.values.read().get(name) {
            return Some(value.clone());
        }
        self.inner
            .parent
            .as_ref()
            .and_then(|parent| parent.get(name))
    }

    pub(crate) fn set(&self, name: String, value: Value) {
        self.inner.values.write().insert(name, value);
    }

    #[cfg(test)]
    pub(crate) fn keys(&self) -> Vec<String> {
        let mut names: Vec<String> = self.inner.values.read().keys().cloned().collect();
        if let Some(parent) = &self.inner.parent {
            for k in parent.keys() {
                if !names.contains(&k) {
                    names.push(k);
                }
            }
        }
        names
    }

}

pub(crate) struct RuntimeContext {
    pub(crate) globals: Env,
    constructor_ordinals: HashMap<String, Option<usize>>,
    pub(crate) gtk_auto_bindings: RwLock<GtkAutoBindingsState>,
    gtk_binding_store: Mutex<GtkBindingStore>,
    gtk_binding_scopes: Mutex<HashMap<i64, Vec<usize>>>,
    gtk_runtime_handler_store: Mutex<GtkRuntimeHandlerStore>,
    pub(crate) reactive_graph: Arc<Mutex<ReactiveGraphState>>,
    console_capture: Mutex<Option<ConsoleCapture>>,
}

#[derive(Clone, Default)]
pub(crate) struct GtkAutoBindingsState {
    pub(crate) named_handlers: HashMap<(String, String), String>,
    pub(crate) unique_handlers_by_signal: HashMap<String, Option<String>>,
}

#[derive(Default)]
struct GtkBindingStore {
    next_handle: i64,
    values: HashMap<i64, Value>,
}

#[derive(Default)]
struct GtkRuntimeHandlerStore {
    next_handle: usize,
    dispatcher_started: bool,
    handlers: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ConsoleCapture {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

impl RuntimeContext {
    pub(crate) fn new_with_constructor_ordinals(
        globals: Env,
        constructor_ordinals: HashMap<String, Option<usize>>,
    ) -> Self {
        Self {
            globals,
            constructor_ordinals,
            gtk_auto_bindings: RwLock::new(GtkAutoBindingsState::default()),
            gtk_binding_store: Mutex::new(GtkBindingStore::default()),
            gtk_binding_scopes: Mutex::new(HashMap::new()),
            gtk_runtime_handler_store: Mutex::new(GtkRuntimeHandlerStore::default()),
            reactive_graph: Arc::new(Mutex::new(ReactiveGraphState {
                next_signal_id: 1,
                next_watcher_id: 1,
                batch_depth: 0,
                flushing: false,
                deferred_flush: false,
                signals: HashMap::new(),
                watchers: HashMap::new(),
                watchers_by_signal: HashMap::new(),
                pending_notifications: std::collections::HashSet::new(),
            })),
            console_capture: Mutex::new(None),
        }
    }

    pub(crate) fn begin_console_capture(&self) {
        *self.console_capture.lock() = Some(ConsoleCapture::default());
    }

    pub(crate) fn take_console_capture(&self) -> ConsoleCapture {
        self.console_capture.lock().take().unwrap_or_default()
    }

    pub(crate) fn capture_stdout(&self, text: &str, newline: bool) -> bool {
        self.capture_console_text(text, newline, false)
    }

    pub(crate) fn capture_stderr(&self, text: &str, newline: bool) -> bool {
        self.capture_console_text(text, newline, true)
    }

    fn capture_console_text(&self, text: &str, newline: bool, stderr: bool) -> bool {
        let mut guard = self.console_capture.lock();
        let Some(capture) = guard.as_mut() else {
            return false;
        };
        let target = if stderr {
            &mut capture.stderr
        } else {
            &mut capture.stdout
        };
        target.push_str(text);
        if newline {
            target.push('\n');
        }
        true
    }

    pub(crate) fn constructor_ordinal(&self, name: &str) -> Option<Option<usize>> {
        self.constructor_ordinals.get(name).copied()
    }

    pub(crate) fn capture_gtk_binding(&self, value: Value) -> i64 {
        let mut store = self.gtk_binding_store.lock();
        let handle = store.next_handle;
        store.next_handle += 1;
        store.values.insert(handle, value);
        handle
    }

    pub(crate) fn resolve_gtk_binding(&self, handle: i64) -> Option<Value> {
        self.gtk_binding_store.lock().values.get(&handle).cloned()
    }

    pub(crate) fn push_gtk_binding_watcher(&self, widget_id: i64, watcher_id: usize) {
        self.gtk_binding_scopes
            .lock()
            .entry(widget_id)
            .or_default()
            .push(watcher_id);
    }

    pub(crate) fn take_gtk_binding_watchers(&self, widget_id: i64) -> Vec<usize> {
        self.gtk_binding_scopes
            .lock()
            .remove(&widget_id)
            .unwrap_or_default()
    }

    pub(crate) fn register_gtk_runtime_handler(&self, handler: Value) -> String {
        let mut store = self.gtk_runtime_handler_store.lock();
        let handle = store.next_handle;
        store.next_handle = store.next_handle.saturating_add(1);
        let token = format!("__aivi_runtime_handler_{handle}");
        store.handlers.insert(token.clone(), handler);
        token
    }

    pub(crate) fn resolve_gtk_runtime_handler(&self, token: &str) -> Option<Value> {
        self.gtk_runtime_handler_store.lock().handlers.get(token).cloned()
    }

    pub(crate) fn mark_gtk_runtime_dispatcher_started(&self) -> bool {
        let mut store = self.gtk_runtime_handler_store.lock();
        if store.dispatcher_started {
            false
        } else {
            store.dispatcher_started = true;
            true
        }
    }

    pub(crate) fn merge_constructor_ordinals(
        &mut self,
        ordinals: HashMap<String, Option<usize>>,
    ) {
        for (name, ordinal) in ordinals {
            self.constructor_ordinals.entry(name).or_insert(ordinal);
        }
    }
}
