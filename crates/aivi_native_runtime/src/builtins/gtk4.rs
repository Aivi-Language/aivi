use std::collections::HashMap;
use std::sync::Arc;

use super::util::builtin;
use crate::{EffectValue, RuntimeError, Value};

thread_local! {
    static GTK4_STATE: std::cell::RefCell<Gtk4State> = std::cell::RefCell::new(Gtk4State::default());
}

#[derive(Default)]
struct Gtk4State {
    next_id: i64,
    apps: HashMap<i64, String>,
    windows: HashMap<i64, WindowState>,
}

#[derive(Clone)]
struct WindowState {
    app_id: i64,
    title: String,
    width: i64,
    height: i64,
}

impl Gtk4State {
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

pub(super) fn build_gtk4_record() -> Value {
    let mut fields = HashMap::new();

    fields.insert(
        "init".to_string(),
        builtin("gtk4.init", 1, |_, _| Ok(effect(|_| Ok(Value::Unit)))),
    );

    fields.insert(
        "appNew".to_string(),
        builtin("gtk4.appNew", 1, |mut args, _| {
            let app_id = match args.remove(0) {
                Value::Text(text) => text,
                _ => return Err(invalid("gtk4.appNew expects Text application id")),
            };
            Ok(effect(move |_| {
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let id = state.alloc_id();
                    state.apps.insert(id, app_id.clone());
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
                let id = GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    if !state.apps.contains_key(&app_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.windowNew unknown app id {app_id}"
                        ))));
                    }
                    let id = state.alloc_id();
                    state.windows.insert(
                        id,
                        WindowState {
                            app_id,
                            title: title.clone(),
                            width,
                            height,
                        },
                    );
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
                GTK4_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    let Some(window) = state.windows.get_mut(&window_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.windowSetTitle unknown window id {window_id}"
                        ))));
                    };
                    window.title = title.clone();
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
                GTK4_STATE.with(|state| {
                    let state = state.borrow();
                    let Some(window) = state.windows.get(&window_id) else {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.windowPresent unknown window id {window_id}"
                        ))));
                    };
                    let _ = (&window.title, window.width, window.height, window.app_id);
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
                GTK4_STATE.with(|state| {
                    let state = state.borrow();
                    if !state.apps.contains_key(&app_id) {
                        return Err(RuntimeError::Error(Value::Text(format!(
                            "gtk4.appRun unknown app id {app_id}"
                        ))));
                    }
                    Ok(Value::Unit)
                })
            }))
        }),
    );

    Value::Record(Arc::new(fields))
}
