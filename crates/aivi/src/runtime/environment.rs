use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;

use super::values::Value;

#[derive(Clone)]
pub(super) struct Env {
    inner: Arc<EnvInner>,
}

struct EnvInner {
    parent: Option<Env>,
    values: RwLock<HashMap<String, Value>>,
}

impl Env {
    pub(super) fn new(parent: Option<Env>) -> Self {
        Self {
            inner: Arc::new(EnvInner {
                parent,
                values: RwLock::new(HashMap::new()),
            }),
        }
    }

    pub(super) fn get(&self, name: &str) -> Option<Value> {
        if let Some(value) = self.inner.values.read().get(name) {
            return Some(value.clone());
        }
        self.inner
            .parent
            .as_ref()
            .and_then(|parent| parent.get(name))
    }

    pub(super) fn set(&self, name: String, value: Value) {
        self.inner.values.write().insert(name, value);
    }

    #[allow(dead_code)]
    pub(super) fn has_local(&self, name: &str) -> bool {
        self.inner.values.read().contains_key(name)
    }
}

pub(super) struct RuntimeContext {
    pub(super) globals: Env,
    debug_call_id: AtomicU64,
    constructor_ordinals: HashMap<String, Option<usize>>,
    machine_specs: RwLock<HashMap<String, HashMap<String, Vec<MachineEdge>>>>,
    machine_states: RwLock<HashMap<String, String>>,
    machine_handlers: RwLock<HashMap<(String, String), Vec<Value>>>,
}

#[derive(Clone, Debug)]
pub(super) struct MachineEdge {
    pub(super) source: Option<String>,
    pub(super) target: String,
}

#[derive(Clone, Debug)]
pub(super) struct MachineTransitionError {
    pub(super) machine: String,
    pub(super) from: String,
    pub(super) event: String,
    pub(super) expected_from: Vec<String>,
}

impl MachineTransitionError {
    pub(super) fn into_value(self) -> Value {
        let mut detail = HashMap::new();
        detail.insert("machine".to_string(), Value::Text(self.machine));
        detail.insert("from".to_string(), Value::Text(self.from));
        detail.insert("event".to_string(), Value::Text(self.event));
        detail.insert(
            "expectedFrom".to_string(),
            Value::List(Arc::new(
                self.expected_from
                    .into_iter()
                    .map(Value::Text)
                    .collect::<Vec<_>>(),
            )),
        );
        Value::Constructor {
            name: "InvalidTransition".to_string(),
            args: vec![Value::Record(Arc::new(detail))],
        }
    }
}

impl RuntimeContext {
    #[allow(dead_code)]
    pub(super) fn new(globals: Env) -> Self {
        Self::new_with_constructor_ordinals(globals, HashMap::new())
    }

    pub(super) fn new_with_constructor_ordinals(
        globals: Env,
        constructor_ordinals: HashMap<String, Option<usize>>,
    ) -> Self {
        Self {
            globals,
            debug_call_id: AtomicU64::new(1),
            constructor_ordinals,
            machine_specs: RwLock::new(HashMap::new()),
            machine_states: RwLock::new(HashMap::new()),
            machine_handlers: RwLock::new(HashMap::new()),
        }
    }

    pub(super) fn next_debug_call_id(&self) -> u64 {
        self.debug_call_id.fetch_add(1, Ordering::Relaxed)
    }

    pub(super) fn constructor_ordinal(&self, name: &str) -> Option<Option<usize>> {
        self.constructor_ordinals.get(name).copied()
    }

    pub(super) fn register_machine(
        &self,
        machine_name: String,
        initial_state: String,
        transitions: HashMap<String, Vec<MachineEdge>>,
    ) {
        self.machine_specs.write().insert(machine_name.clone(), transitions);
        self.machine_states.write().insert(machine_name.clone(), initial_state);
        self.machine_handlers
            .write()
            .retain(|(name, _), _| name != &machine_name);
    }

    pub(super) fn machine_current_state(&self, machine_name: &str) -> Option<String> {
        self.machine_states.read().get(machine_name).cloned()
    }

    pub(super) fn machine_can_transition(&self, machine_name: &str, event: &str) -> bool {
        let Some(current) = self.machine_current_state(machine_name) else {
            return false;
        };
        let specs = self.machine_specs.read();
        let Some(events) = specs.get(machine_name) else {
            return false;
        };
        let Some(edges) = events.get(event) else {
            return false;
        };
        edges
            .iter()
            .filter(|edge| edge.source.as_deref() == Some(current.as_str()))
            .count()
            == 1
    }

    pub(super) fn apply_machine_transition(
        &self,
        machine_name: &str,
        event: &str,
    ) -> Result<String, MachineTransitionError> {
        let Some(current) = self.machine_current_state(machine_name) else {
            return Err(MachineTransitionError {
                machine: machine_name.to_string(),
                from: "<unknown>".to_string(),
                event: event.to_string(),
                expected_from: Vec::new(),
            });
        };

        let specs = self.machine_specs.read();
        let edges = specs
            .get(machine_name)
            .and_then(|events| events.get(event))
            .cloned()
            .unwrap_or_default();
        drop(specs);

        let expected_from = {
            let mut names: Vec<String> = edges.iter().filter_map(|edge| edge.source.clone()).collect();
            names.sort();
            names.dedup();
            names
        };

        let matching: Vec<MachineEdge> = edges
            .iter()
            .filter(|edge| edge.source.as_deref() == Some(current.as_str()))
            .cloned()
            .collect();
        if matching.len() != 1 {
            return Err(MachineTransitionError {
                machine: machine_name.to_string(),
                from: current,
                event: event.to_string(),
                expected_from,
            });
        }

        let next = matching[0].target.clone();
        self.machine_states
            .write()
            .insert(machine_name.to_string(), next.clone());
        Ok(next)
    }

    pub(super) fn register_machine_handler(&self, machine_name: &str, event: &str, handler: Value) {
        let key = (machine_name.to_string(), event.to_string());
        self.machine_handlers
            .write()
            .entry(key)
            .or_default()
            .push(handler);
    }

    pub(super) fn machine_handlers(&self, machine_name: &str, event: &str) -> Vec<Value> {
        self.machine_handlers
            .read()
            .get(&(machine_name.to_string(), event.to_string()))
            .cloned()
            .unwrap_or_default()
    }
}
