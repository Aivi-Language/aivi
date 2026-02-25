use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

use super::values::Value;

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
}

pub(crate) struct RuntimeContext {
    pub(crate) globals: Env,
    constructor_ordinals: HashMap<String, Option<usize>>,
}

impl RuntimeContext {
    pub(crate) fn new_with_constructor_ordinals(
        globals: Env,
        constructor_ordinals: HashMap<String, Option<usize>>,
    ) -> Self {
        Self {
            globals,
            constructor_ordinals,
        }
    }

    pub(crate) fn constructor_ordinal(&self, name: &str) -> Option<Option<usize>> {
        self.constructor_ordinals.get(name).copied()
    }
}
