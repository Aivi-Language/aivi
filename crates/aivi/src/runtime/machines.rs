use std::collections::HashMap;
use std::sync::Arc;

use super::environment::{Env, MachineEdge};
use super::values::{EffectValue, Value};
use super::{format_value, runtime_builtin, Runtime, RuntimeError};

pub(crate) fn machine_transition_builtin_name(machine_name: &str, event: &str) -> String {
    format!("__machine_transition|{machine_name}|{event}")
}

fn parse_machine_transition_ref(value: &Value) -> Option<(String, String)> {
    let Value::Builtin(builtin) = value else {
        return None;
    };
    if !builtin.args.is_empty() {
        return None;
    }
    let name = &builtin.imp.name;
    let mut parts = name.splitn(3, '|');
    let prefix = parts.next()?;
    if prefix != "__machine_transition" {
        return None;
    }
    let machine = parts.next()?.to_string();
    let event = parts.next()?.to_string();
    Some((machine, event))
}

pub(super) fn make_machine_on_builtin() -> Value {
    runtime_builtin("__machine_on", 2, |mut args, _| {
        let handler = args.pop().unwrap_or(Value::Unit);
        let transition = args.pop().unwrap_or(Value::Unit);
        if let Some((machine_name, event_name)) = parse_machine_transition_ref(&transition) {
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    runtime.ctx.register_machine_handler(
                        &machine_name,
                        &event_name,
                        handler.clone(),
                    );
                    Ok(Value::Unit)
                }),
            };
            return Ok(Value::Effect(Arc::new(effect)));
        }

        match handler {
            Value::Effect(_) | Value::Source(_) => Ok(handler),
            other => Err(RuntimeError::Message(format!(
                "`on` handler must be an Effect, got {}",
                format_value(&other)
            ))),
        }
    })
}

#[allow(dead_code)]
pub(crate) fn make_machine_transition_builtin(machine_name: String, event_name: String) -> Value {
    let builtin_name = machine_transition_builtin_name(&machine_name, &event_name);
    runtime_builtin(&builtin_name, 1, move |mut args, _| {
        let _payload = args.pop().unwrap_or(Value::Unit);
        let machine_name = machine_name.clone();
        let event_name = event_name.clone();
        let effect = EffectValue::Thunk {
            func: Arc::new(move |runtime| {
                runtime
                    .ctx
                    .apply_machine_transition(&machine_name, &event_name)
                    .map_err(|err| RuntimeError::Error(err.into_value()))?;
                for handler in runtime.ctx.machine_handlers(&machine_name, &event_name) {
                    runtime.run_effect_value(handler)?;
                }
                Ok(Value::Unit)
            }),
        };
        Ok(Value::Effect(Arc::new(effect)))
    })
}

pub(crate) fn make_machine_current_state_builtin(machine_name: String) -> Value {
    runtime_builtin(
        &format!("__machine_current_state|{machine_name}"),
        1,
        move |mut args, runtime| {
            let _ = args.pop();
            let Some(state) = runtime.ctx.machine_current_state(&machine_name) else {
                return Err(RuntimeError::Message(format!(
                    "unknown machine state for {machine_name}"
                )));
            };
            Ok(Value::Constructor {
                name: state,
                args: Vec::new(),
            })
        },
    )
}

pub(crate) fn make_machine_can_builtin(machine_name: String, event_name: String) -> Value {
    runtime_builtin(
        &format!("__machine_can|{machine_name}|{event_name}"),
        1,
        move |mut args, runtime| {
            let _ = args.pop();
            Ok(Value::Bool(
                runtime
                    .ctx
                    .machine_can_transition(&machine_name, &event_name),
            ))
        },
    )
}

#[allow(dead_code, clippy::type_complexity)]
pub(super) fn bind_module_machine_values(
    surface_module: &crate::surface::Module,
    module_name: &str,
    module_env: &Env,
    globals: &Env,
    machine_specs: &mut Vec<(String, String, HashMap<String, Vec<MachineEdge>>)>,
) {
    for item in &surface_module.items {
        let crate::surface::ModuleItem::MachineDecl(machine_decl) = item else {
            continue;
        };

        let runtime_machine_name = format!("{module_name}.{}", machine_decl.name.name);
        let mut transitions: HashMap<String, Vec<MachineEdge>> = HashMap::new();
        let mut initial_state = machine_decl
            .transitions
            .iter()
            .find(|transition| transition.source.name.is_empty())
            .map(|transition| transition.target.name.clone())
            .or_else(|| {
                machine_decl
                    .transitions
                    .first()
                    .map(|transition| transition.target.name.clone())
            })
            .or_else(|| {
                machine_decl
                    .states
                    .first()
                    .map(|state| state.name.name.clone())
            })
            .unwrap_or_else(|| "Closed".to_string());

        for transition in &machine_decl.transitions {
            let source = if transition.source.name.is_empty() {
                None
            } else {
                Some(transition.source.name.clone())
            };
            if source.is_none() {
                initial_state = transition.target.name.clone();
            }
            transitions
                .entry(transition.name.name.clone())
                .or_default()
                .push(MachineEdge {
                    source,
                    target: transition.target.name.clone(),
                });
        }

        let mut state_names = machine_decl
            .states
            .iter()
            .map(|state| state.name.name.clone())
            .collect::<Vec<_>>();
        state_names.sort();
        state_names.dedup();
        for state_name in state_names {
            let state_ctor = Value::Constructor {
                name: state_name.clone(),
                args: Vec::new(),
            };
            module_env.set(state_name.clone(), state_ctor.clone());
            let qualified = format!("{module_name}.{state_name}");
            if globals.get(&qualified).is_none() {
                globals.set(qualified, state_ctor);
            }
        }

        let mut machine_fields: HashMap<String, Value> = HashMap::new();
        let mut can_fields: HashMap<String, Value> = HashMap::new();
        let mut event_names = transitions.keys().cloned().collect::<Vec<_>>();
        event_names.sort();
        for event_name in event_names {
            let transition_value =
                make_machine_transition_builtin(runtime_machine_name.clone(), event_name.clone());
            machine_fields.insert(event_name.clone(), transition_value.clone());
            module_env.set(event_name.clone(), transition_value.clone());
            let qualified_transition = format!("{module_name}.{event_name}");
            if globals.get(&qualified_transition).is_none() {
                globals.set(qualified_transition, transition_value);
            }
            can_fields.insert(
                event_name.clone(),
                make_machine_can_builtin(runtime_machine_name.clone(), event_name),
            );
        }

        machine_fields.insert(
            "currentState".to_string(),
            make_machine_current_state_builtin(runtime_machine_name.clone()),
        );
        machine_fields.insert("can".to_string(), Value::Record(Arc::new(can_fields)));
        let machine_value = Value::Record(Arc::new(machine_fields));
        module_env.set(machine_decl.name.name.clone(), machine_value.clone());
        let qualified_machine = format!("{module_name}.{}", machine_decl.name.name);
        if globals.get(&qualified_machine).is_none() {
            globals.set(qualified_machine, machine_value);
        }

        machine_specs.push((runtime_machine_name, initial_state, transitions));
    }
}

/// Register machine transition builtins into the runtime globals.
/// Used by the Cranelift JIT path which doesn't have per-module environments.
/// The JIT codegen emits `rt_get_global("eventName")` with short names, so we
/// must register both short and qualified names in globals.
pub(crate) fn register_machines_for_jit(
    runtime: &Runtime,
    surface_modules: &[crate::surface::Module],
) {
    let globals = &runtime.ctx.globals;
    for module in surface_modules {
        let module_name = &module.name.name;
        for item in &module.items {
            let crate::surface::ModuleItem::MachineDecl(machine_decl) = item else {
                continue;
            };

            let runtime_machine_name = format!("{module_name}.{}", machine_decl.name.name);
            let mut transitions: HashMap<String, Vec<MachineEdge>> = HashMap::new();
            let mut initial_state = machine_decl
                .transitions
                .iter()
                .find(|t| t.source.name.is_empty())
                .map(|t| t.target.name.clone())
                .or_else(|| {
                    machine_decl
                        .transitions
                        .first()
                        .map(|t| t.target.name.clone())
                })
                .or_else(|| {
                    machine_decl
                        .states
                        .first()
                        .map(|s| s.name.name.clone())
                })
                .unwrap_or_else(|| "Closed".to_string());

            for transition in &machine_decl.transitions {
                let source = if transition.source.name.is_empty() {
                    None
                } else {
                    Some(transition.source.name.clone())
                };
                if source.is_none() {
                    initial_state = transition.target.name.clone();
                }
                transitions
                    .entry(transition.name.name.clone())
                    .or_default()
                    .push(MachineEdge {
                        source,
                        target: transition.target.name.clone(),
                    });
            }

            // Register state constructors (both short and qualified)
            let mut state_names = machine_decl
                .states
                .iter()
                .map(|s| s.name.name.clone())
                .collect::<Vec<_>>();
            state_names.sort();
            state_names.dedup();
            for state_name in &state_names {
                let state_ctor = Value::Constructor {
                    name: state_name.clone(),
                    args: Vec::new(),
                };
                globals.set(state_name.clone(), state_ctor.clone());
                let qualified = format!("{module_name}.{state_name}");
                if globals.get(&qualified).is_none() {
                    globals.set(qualified, state_ctor);
                }
            }

            // Register transition builtins (both short and qualified)
            let mut machine_fields: HashMap<String, Value> = HashMap::new();
            let mut can_fields: HashMap<String, Value> = HashMap::new();
            let mut event_names = transitions.keys().cloned().collect::<Vec<_>>();
            event_names.sort();
            for event_name in event_names {
                let transition_value = make_machine_transition_builtin(
                    runtime_machine_name.clone(),
                    event_name.clone(),
                );
                machine_fields.insert(event_name.clone(), transition_value.clone());
                // Short name in globals (JIT uses rt_get_global with short names)
                globals.set(event_name.clone(), transition_value.clone());
                let qualified_transition = format!("{module_name}.{event_name}");
                if globals.get(&qualified_transition).is_none() {
                    globals.set(qualified_transition, transition_value);
                }
                can_fields.insert(
                    event_name.clone(),
                    make_machine_can_builtin(runtime_machine_name.clone(), event_name),
                );
            }

            // Register machine record (both short and qualified)
            machine_fields.insert(
                "currentState".to_string(),
                make_machine_current_state_builtin(runtime_machine_name.clone()),
            );
            machine_fields.insert("can".to_string(), Value::Record(Arc::new(can_fields)));
            let machine_value = Value::Record(Arc::new(machine_fields));
            globals.set(machine_decl.name.name.clone(), machine_value.clone());
            let qualified_machine = format!("{module_name}.{}", machine_decl.name.name);
            if globals.get(&qualified_machine).is_none() {
                globals.set(qualified_machine, machine_value);
            }

            // Register machine spec with RuntimeContext
            runtime
                .ctx
                .register_machine(runtime_machine_name, initial_state, transitions);
        }
    }
}
