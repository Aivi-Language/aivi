use std::collections::HashMap;

use rudo_gc::{Gc, GcMutex, Trace};

use super::values::Value;

#[derive(Clone, Trace)]
pub(super) struct Env {
    inner: Gc<EnvInner>,
}

#[derive(Trace)]
struct EnvInner {
    parent: Option<Env>,
    values: GcMutex<HashMap<String, Value>>,
}

impl Env {
    pub(super) fn new(parent: Option<Env>) -> Self {
        Self {
            inner: Gc::new(EnvInner {
                parent,
                values: GcMutex::new(HashMap::new()),
            }),
        }
    }

    pub(super) fn get(&self, name: &str) -> Option<Value> {
        if let Some(value) = self.inner.values.lock().get(name) {
            return Some(value.clone());
        }
        self.inner
            .parent
            .as_ref()
            .and_then(|parent| parent.get(name))
    }

    pub(super) fn set(&self, name: String, value: Value) {
        self.inner.values.lock().insert(name, value);
    }
}

pub(super) struct RuntimeContext {
    pub(super) globals: Env,
}
