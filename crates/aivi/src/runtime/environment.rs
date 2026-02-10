use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::Value;

#[derive(Clone)]
pub(super) struct Env {
    pub(super) parent: Option<Arc<Env>>,
    pub(super) values: Arc<Mutex<HashMap<String, Value>>>,
}

impl Env {
    pub(super) fn new(parent: Option<Arc<Env>>) -> Self {
        Self {
            parent,
            values: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(super) fn get(&self, name: &str) -> Option<Value> {
        if let Ok(values) = self.values.lock() {
            if let Some(value) = values.get(name) {
                return Some(value.clone());
            }
        }
        self.parent.as_ref().and_then(|parent| parent.get(name))
    }

    pub(super) fn set(&self, name: String, value: Value) {
        if let Ok(mut values) = self.values.lock() {
            values.insert(name, value);
        }
    }
}

pub(super) struct RuntimeContext {
    pub(super) globals: Env,
}
