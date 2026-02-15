use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Datelike, NaiveDate};
use regex::RegexBuilder;
use url::Url;

use crate::hir::{
    HirBlockItem, HirExpr, HirListItem, HirLiteral, HirMatchArm, HirPathSegment, HirPattern,
    HirProgram, HirRecordField, HirTextPart,
};
use crate::i18n::{parse_message_template, validate_key_text, MessagePart};
use crate::AiviError;

mod builtins;
mod environment;
mod http;
#[cfg(test)]
mod tests;
mod values;

use self::builtins::register_builtins;
use self::environment::{Env, RuntimeContext};
use self::values::{
    BuiltinImpl, BuiltinValue, ClosureValue, EffectValue, KeyValue, ResourceValue, SourceValue,
    ThunkValue, Value,
};

#[derive(Debug)]
struct CancelToken {
    local: AtomicBool,
    parent: Option<Arc<CancelToken>>,
}

impl CancelToken {
    fn root() -> Arc<Self> {
        Arc::new(Self {
            local: AtomicBool::new(false),
            parent: None,
        })
    }

    fn child(parent: Arc<CancelToken>) -> Arc<Self> {
        Arc::new(Self {
            local: AtomicBool::new(false),
            parent: Some(parent),
        })
    }

    fn cancel(&self) {
        self.local.store(true, Ordering::SeqCst);
    }

    fn parent(&self) -> Option<Arc<CancelToken>> {
        self.parent.clone()
    }

    fn is_cancelled(&self) -> bool {
        if self.local.load(Ordering::SeqCst) {
            return true;
        }
        self.parent
            .as_ref()
            .is_some_and(|parent| parent.is_cancelled())
    }
}

struct Runtime {
    ctx: Arc<RuntimeContext>,
    cancel: Arc<CancelToken>,
    cancel_mask: usize,
    fuel: Option<u64>,
    rng_state: u64,
    debug_stack: Vec<DebugFrame>,
}

#[derive(Clone)]
struct DebugFrame {
    fn_name: String,
    call_id: u64,
    start: Option<std::time::Instant>,
}

#[derive(Clone)]
enum RuntimeError {
    Error(Value),
    Cancelled,
    Message(String),
}

#[derive(Debug, Clone)]
pub struct TestFailure {
    pub name: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct TestReport {
    pub passed: usize,
    pub failed: usize,
    pub failures: Vec<TestFailure>,
}

pub fn run_native(program: HirProgram) -> Result<(), AiviError> {
    let mut runtime = build_runtime_from_program(program)?;
    let main = runtime
        .ctx
        .globals
        .get("main")
        .ok_or_else(|| AiviError::Runtime("missing main definition".to_string()))?;
    let main_value = match runtime.force_value(main) {
        Ok(value) => value,
        Err(err) => return Err(AiviError::Runtime(format_runtime_error(err))),
    };
    let effect = match main_value {
        Value::Effect(effect) => Value::Effect(effect),
        other => {
            return Err(AiviError::Runtime(format!(
                "main must be an Effect value, got {}",
                format_value(&other)
            )))
        }
    };

    match runtime.run_effect_value(effect) {
        Ok(_) => Ok(()),
        Err(err) => Err(AiviError::Runtime(format_runtime_error(err))),
    }
}

/// Runs `main` with a simple "fuel" limit to prevent hangs in fuzzers/tests.
///
/// If fuel is exhausted, execution is cancelled and treated as success (the program is considered
/// non-terminating within the provided budget).
pub fn run_native_with_fuel(program: HirProgram, fuel: u64) -> Result<(), AiviError> {
    let mut runtime = build_runtime_from_program(program)?;
    runtime.fuel = Some(fuel);

    let main = runtime
        .ctx
        .globals
        .get("main")
        .ok_or_else(|| AiviError::Runtime("missing main definition".to_string()))?;
    let main_value = match runtime.force_value(main) {
        Ok(value) => value,
        Err(RuntimeError::Cancelled) => return Ok(()),
        Err(err) => return Err(AiviError::Runtime(format_runtime_error(err))),
    };
    let effect = match main_value {
        Value::Effect(effect) => Value::Effect(effect),
        other => {
            return Err(AiviError::Runtime(format!(
                "main must be an Effect value, got {}",
                format_value(&other)
            )))
        }
    };

    match runtime.run_effect_value(effect) {
        Ok(_) => Ok(()),
        Err(RuntimeError::Cancelled) => Ok(()),
        Err(err) => Err(AiviError::Runtime(format_runtime_error(err))),
    }
}

pub fn run_test_suite(program: HirProgram, test_names: &[String]) -> Result<TestReport, AiviError> {
    let mut runtime = build_runtime_from_program(program)?;
    let mut report = TestReport {
        passed: 0,
        failed: 0,
        failures: Vec::new(),
    };

    for name in test_names {
        let Some(value) = runtime.ctx.globals.get(name) else {
            report.failed += 1;
            report.failures.push(TestFailure {
                name: name.clone(),
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
                    message: format!("test must be an Effect value, got {}", format_value(&other)),
                });
                continue;
            }
        };

        match runtime.run_effect_value(effect) {
            Ok(_) => report.passed += 1,
            Err(err) => {
                report.failed += 1;
                report.failures.push(TestFailure {
                    name: name.clone(),
                    message: format_runtime_error(err),
                });
            }
        }
    }

    Ok(report)
}

fn build_runtime_from_program(program: HirProgram) -> Result<Runtime, AiviError> {
    if program.modules.is_empty() {
        return Err(AiviError::Runtime("no modules to run".to_string()));
    }

    let mut grouped: HashMap<String, Vec<HirExpr>> = HashMap::new();
    for module in program.modules {
        let module_name = module.name.clone();
        for def in module.defs {
            // Unqualified entry (legacy/global namespace).
            grouped
                .entry(def.name.clone())
                .or_default()
                .push(def.expr.clone());

            // Qualified entry enables disambiguation (e.g. `aivi.database.load`) without relying
            // on wildcard imports to win against builtins like `load`.
            grouped
                .entry(format!("{module_name}.{}", def.name))
                .or_default()
                .push(def.expr);
        }
    }
    if grouped.is_empty() {
        return Err(AiviError::Runtime("no definitions to run".to_string()));
    }

    let globals = Env::new(None);
    register_builtins(&globals);
    for (name, exprs) in grouped {
        // Builtins are the "runtime stdlib" today; don't let parsed source overwrite them.
        if globals.get(&name).is_some() {
            continue;
        }
        if exprs.len() == 1 {
            let thunk = ThunkValue {
                expr: Arc::new(exprs.into_iter().next().unwrap()),
                env: globals.clone(),
                cached: Mutex::new(None),
                in_progress: AtomicBool::new(false),
            };
            globals.set(name, Value::Thunk(Arc::new(thunk)));
        } else {
            let mut clauses = Vec::new();
            for expr in exprs {
                let thunk = ThunkValue {
                    expr: Arc::new(expr),
                    env: globals.clone(),
                    cached: Mutex::new(None),
                    in_progress: AtomicBool::new(false),
                };
                clauses.push(Value::Thunk(Arc::new(thunk)));
            }
            globals.set(name, Value::MultiClause(clauses));
        }
    }

    let ctx = Arc::new(RuntimeContext::new(globals));
    let cancel = CancelToken::root();
    Ok(Runtime::new(ctx, cancel))
}

fn format_runtime_error(err: RuntimeError) -> String {
    match err {
        RuntimeError::Cancelled => "execution cancelled".to_string(),
        RuntimeError::Message(message) => message,
        RuntimeError::Error(value) => format!("runtime error: {}", format_value(&value)),
    }
}

include!("runtime_impl/lifecycle_and_cancel.rs");
include!("runtime_impl/eval_and_apply.rs");
include!("runtime_impl/resources.rs");

impl BuiltinValue {
    fn apply(&self, arg: Value, runtime: &mut Runtime) -> Result<Value, RuntimeError> {
        let mut args = self.args.clone();
        args.push(arg);
        if args.len() == self.imp.arity {
            (self.imp.func)(args, runtime)
        } else {
            Ok(Value::Builtin(BuiltinValue {
                imp: self.imp.clone(),
                args,
            }))
        }
    }
}

fn collect_pattern_bindings(pattern: &HirPattern, value: &Value) -> Option<HashMap<String, Value>> {
    let mut bindings = HashMap::new();
    if match_pattern(pattern, value, &mut bindings) {
        Some(bindings)
    } else {
        None
    }
}

fn match_pattern(
    pattern: &HirPattern,
    value: &Value,
    bindings: &mut HashMap<String, Value>,
) -> bool {
    match pattern {
        HirPattern::Wildcard { .. } => true,
        HirPattern::Var { name, .. } => {
            bindings.insert(name.clone(), value.clone());
            true
        }
        HirPattern::Literal { value: lit, .. } => match (lit, value) {
            (HirLiteral::Number(text), Value::Int(num)) => parse_number_literal(text) == Some(*num),
            (HirLiteral::Number(text), Value::Float(num)) => text.parse::<f64>().ok() == Some(*num),
            (HirLiteral::String(text), Value::Text(val)) => text == val,
            (HirLiteral::Sigil { tag, body, flags }, Value::Record(map)) => {
                let tag_ok = matches!(map.get("tag"), Some(Value::Text(val)) if val == tag);
                let body_ok = matches!(map.get("body"), Some(Value::Text(val)) if val == body);
                let flags_ok = matches!(map.get("flags"), Some(Value::Text(val)) if val == flags);
                tag_ok && body_ok && flags_ok
            }
            (HirLiteral::Bool(flag), Value::Bool(val)) => *flag == *val,
            (HirLiteral::DateTime(text), Value::DateTime(val)) => text == val,
            _ => false,
        },
        HirPattern::Constructor { name, args, .. } => match value {
            Value::Constructor {
                name: value_name,
                args: value_args,
            } => {
                if name != value_name || args.len() != value_args.len() {
                    return false;
                }
                for (pat, val) in args.iter().zip(value_args.iter()) {
                    if !match_pattern(pat, val, bindings) {
                        return false;
                    }
                }
                true
            }
            _ => false,
        },
        HirPattern::Tuple { items, .. } => match value {
            Value::Tuple(values) => {
                if items.len() != values.len() {
                    return false;
                }
                for (pat, val) in items.iter().zip(values.iter()) {
                    if !match_pattern(pat, val, bindings) {
                        return false;
                    }
                }
                true
            }
            _ => false,
        },
        HirPattern::List { items, rest, .. } => match value {
            Value::List(values) => {
                if values.len() < items.len() {
                    return false;
                }
                for (pat, val) in items.iter().zip(values.iter()) {
                    if !match_pattern(pat, val, bindings) {
                        return false;
                    }
                }
                if let Some(rest) = rest {
                    let tail = values[items.len()..].to_vec();
                    match_pattern(rest, &Value::List(Arc::new(tail)), bindings)
                } else {
                    values.len() == items.len()
                }
            }
            _ => false,
        },
        HirPattern::Record { fields, .. } => match value {
            Value::Record(map) => {
                for field in fields {
                    let Some(value) = record_get_path(map, &field.path) else {
                        return false;
                    };
                    if !match_pattern(&field.pattern, value, bindings) {
                        return false;
                    }
                }
                true
            }
            _ => false,
        },
    }
}
