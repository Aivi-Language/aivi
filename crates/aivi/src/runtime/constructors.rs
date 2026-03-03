use std::collections::HashMap;

use super::values::{BuiltinValue, TaggedValue, Value};
use super::{format_runtime_error, format_value, Runtime, RuntimeError};
use crate::hir::HirProgram;
use crate::AiviError;

#[derive(Debug, Clone)]
pub struct TestFailure {
    pub name: String,
    pub description: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct TestSuccess {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct TestReport {
    pub passed: usize,
    pub failed: usize,
    pub failures: Vec<TestFailure>,
    pub successes: Vec<TestSuccess>,
}

#[allow(dead_code)]
pub fn run_test_suite(
    program: HirProgram,
    test_entries: &[(String, String)],
    surface_modules: &[crate::surface::Module],
) -> Result<TestReport, AiviError> {
    const TEST_FUEL_BUDGET: u64 = 500_000;
    let mut runtime = super::build_runtime_from_program_scoped(program, surface_modules)?;
    let mut report = TestReport {
        passed: 0,
        failed: 0,
        failures: Vec::new(),
        successes: Vec::new(),
    };

    for (name, description) in test_entries {
        // Keep a runaway test from exhausting the thread stack; each test gets a fresh budget.
        runtime.fuel = Some(TEST_FUEL_BUDGET);
        let Some(value) = runtime.ctx.globals.get(name) else {
            report.failed += 1;
            report.failures.push(TestFailure {
                name: name.clone(),
                description: description.clone(),
                message: "missing definition".to_string(),
            });
            continue;
        };

        let value = match runtime.force_value(value) {
            Ok(value) => value,
            Err(err) => {
                report.failed += 1;
                report.failures.push(TestFailure {
                    name: name.clone(),
                    description: description.clone(),
                    message: format_runtime_error(err),
                });
                continue;
            }
        };

        let effect = match value {
            Value::Effect(effect) => Value::Effect(effect),
            other => {
                report.failed += 1;
                report.failures.push(TestFailure {
                    name: name.clone(),
                    description: description.clone(),
                    message: format!("test must be an Effect value, got {}", format_value(&other)),
                });
                continue;
            }
        };

        match runtime.run_effect_value(effect) {
            Ok(_) => {
                report.passed += 1;
                report.successes.push(TestSuccess {
                    name: name.clone(),
                    description: description.clone(),
                });
            }
            Err(err) => {
                report.failed += 1;
                report.failures.push(TestFailure {
                    name: name.clone(),
                    description: description.clone(),
                    message: format_runtime_error(err),
                });
            }
        }
    }

    Ok(report)
}

pub(super) fn insert_constructor_ordinal(
    ordinals: &mut HashMap<String, Option<usize>>,
    name: String,
    ordinal: usize,
) {
    match ordinals.get(&name) {
        None => {
            ordinals.insert(name, Some(ordinal));
        }
        Some(Some(existing)) if *existing == ordinal => {}
        _ => {
            ordinals.insert(name, None);
        }
    }
}

pub(super) fn core_constructor_ordinals() -> HashMap<String, Option<usize>> {
    let mut ordinals = HashMap::new();
    insert_constructor_ordinal(&mut ordinals, "True".to_string(), 0);
    insert_constructor_ordinal(&mut ordinals, "False".to_string(), 1);
    insert_constructor_ordinal(&mut ordinals, "None".to_string(), 0);
    insert_constructor_ordinal(&mut ordinals, "Some".to_string(), 1);
    insert_constructor_ordinal(&mut ordinals, "Err".to_string(), 0);
    insert_constructor_ordinal(&mut ordinals, "Ok".to_string(), 1);
    insert_constructor_ordinal(&mut ordinals, "Closed".to_string(), 0);
    ordinals
}

pub(crate) fn collect_surface_constructor_ordinals(
    surface_modules: &[crate::surface::Module],
) -> HashMap<String, Option<usize>> {
    let mut ordinals = HashMap::new();
    for module in surface_modules {
        for item in &module.items {
            match item {
                crate::surface::ModuleItem::TypeDecl(decl) => {
                    for (ordinal, ctor) in decl.constructors.iter().enumerate() {
                        insert_constructor_ordinal(&mut ordinals, ctor.name.name.clone(), ordinal);
                    }
                }
                crate::surface::ModuleItem::DomainDecl(domain) => {
                    for domain_item in &domain.items {
                        let crate::surface::DomainItem::TypeAlias(decl) = domain_item else {
                            continue;
                        };
                        for (ordinal, ctor) in decl.constructors.iter().enumerate() {
                            insert_constructor_ordinal(
                                &mut ordinals,
                                ctor.name.name.clone(),
                                ordinal,
                            );
                        }
                    }
                }
                crate::surface::ModuleItem::MachineDecl(machine_decl) => {
                    for (ordinal, state) in machine_decl.states.iter().enumerate() {
                        insert_constructor_ordinal(&mut ordinals, state.name.name.clone(), ordinal);
                    }
                }
                _ => {}
            }
        }
    }
    ordinals
}

impl BuiltinValue {
    pub(super) fn apply(&self, arg: Value, runtime: &mut Runtime) -> Result<Value, RuntimeError> {
        let mut args = self.args.clone();
        let mut tagged_args = self.tagged_args.clone();
        let mut pending_arg = Some(arg);
        if let Some(existing) = tagged_args.as_mut() {
            if let Some(tagged) =
                TaggedValue::from_value(pending_arg.as_ref().expect("pending arg"))
            {
                existing.push(tagged);
                pending_arg = None;
            } else {
                args = existing
                    .iter()
                    .copied()
                    .map(TaggedValue::to_value)
                    .collect();
                tagged_args = None;
            }
        }
        if let Some(arg) = pending_arg {
            args.push(arg);
        }
        if args.is_empty() {
            if let Some(existing) = tagged_args.as_ref() {
                if existing.len() == self.imp.arity {
                    args = existing
                        .iter()
                        .copied()
                        .map(TaggedValue::to_value)
                        .collect();
                } else {
                    return Ok(Value::Builtin(BuiltinValue {
                        imp: self.imp.clone(),
                        args,
                        tagged_args,
                    }));
                }
            }
        }
        if args.len() == self.imp.arity {
            (self.imp.func)(args, runtime)
        } else {
            Ok(Value::Builtin(BuiltinValue {
                imp: self.imp.clone(),
                args,
                tagged_args,
            }))
        }
    }
}
