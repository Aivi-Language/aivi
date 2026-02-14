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
    BuiltinImpl, BuiltinValue, ClosureValue, EffectValue, KeyValue, ResourceValue, ThunkValue,
    Value,
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

impl Runtime {
    fn new(ctx: Arc<RuntimeContext>, cancel: Arc<CancelToken>) -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|dur| dur.as_nanos() as u64)
            .unwrap_or(0x1234_5678);
        Self {
            ctx,
            cancel,
            cancel_mask: 0,
            fuel: None,
            rng_state: seed ^ 0x9E37_79B9_7F4A_7C15,
            debug_stack: Vec::new(),
        }
    }

    fn check_cancelled(&mut self) -> Result<(), RuntimeError> {
        if self.cancel_mask > 0 {
            return Ok(());
        }
        if let Some(fuel) = self.fuel.as_mut() {
            if *fuel == 0 {
                return Err(RuntimeError::Cancelled);
            }
            *fuel = fuel.saturating_sub(1);
        }
        if self.cancel.is_cancelled() {
            Err(RuntimeError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn uncancelable<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        self.cancel_mask = self.cancel_mask.saturating_add(1);
        let result = f(self);
        self.cancel_mask = self.cancel_mask.saturating_sub(1);
        result
    }

    fn next_u64(&mut self) -> u64 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        self.rng_state
    }

    fn force_value(&mut self, value: Value) -> Result<Value, RuntimeError> {
        match value {
            Value::Thunk(thunk) => self.eval_thunk(thunk),
            other => Ok(other),
        }
    }

    fn eval_thunk(&mut self, thunk: Arc<ThunkValue>) -> Result<Value, RuntimeError> {
        let cached = thunk.cached.lock().expect("thunk cache lock");
        if let Some(value) = cached.clone() {
            return Ok(value);
        }
        drop(cached);
        if thunk.in_progress.swap(true, Ordering::SeqCst) {
            return Err(RuntimeError::Message(
                "recursive definition detected".to_string(),
            ));
        }
        let value = self.eval_expr(&thunk.expr, &thunk.env)?;
        let mut cached = thunk.cached.lock().expect("thunk cache lock");
        *cached = Some(value.clone());
        thunk.in_progress.store(false, Ordering::SeqCst);
        Ok(value)
    }

    fn eval_expr(&mut self, expr: &HirExpr, env: &Env) -> Result<Value, RuntimeError> {
        self.check_cancelled()?;
        match expr {
            HirExpr::Var { name, .. } => {
                if let Some(value) = env.get(name) {
                    return self.force_value(value);
                }
                if let Some(ctor) = constructor_segment(name) {
                    return Ok(Value::Constructor {
                        name: ctor.to_string(),
                        args: Vec::new(),
                    });
                }
                Err(RuntimeError::Message(format!("unknown name {name}")))
            }
            HirExpr::LitNumber { text, .. } => {
                if let Some(value) = parse_number_value(text) {
                    return Ok(value);
                }
                let value = env.get(text).ok_or_else(|| {
                    RuntimeError::Message(format!("unknown numeric literal {text}"))
                })?;
                self.force_value(value)
            }
            HirExpr::LitString { text, .. } => Ok(Value::Text(text.clone())),
            HirExpr::TextInterpolate { parts, .. } => {
                let mut out = String::new();
                for part in parts {
                    match part {
                        HirTextPart::Text { text } => out.push_str(text),
                        HirTextPart::Expr { expr } => {
                            let value = self.eval_expr(expr, env)?;
                            out.push_str(&format_value(&value));
                        }
                    }
                }
                Ok(Value::Text(out))
            }
            HirExpr::LitSigil {
                tag, body, flags, ..
            } => match tag.as_str() {
                // Keep the runtime behavior aligned with `specs/02_syntax/13_sigils.md` and
                // `specs/05_stdlib/00_core/29_i18n.md`:
                // - ~k/~m are record-shaped values.
                // - ~m includes compiled `parts` for `i18n.render`.
                "r" => {
                    let mut builder = RegexBuilder::new(body);
                    for flag in flags.chars() {
                        match flag {
                            'i' => {
                                builder.case_insensitive(true);
                            }
                            'm' => {
                                builder.multi_line(true);
                            }
                            's' => {
                                builder.dot_matches_new_line(true);
                            }
                            'x' => {
                                builder.ignore_whitespace(true);
                            }
                            _ => {}
                        }
                    }
                    let regex = builder.build().map_err(|err| {
                        RuntimeError::Message(format!("invalid regex literal: {err}"))
                    })?;
                    Ok(Value::Regex(Arc::new(regex)))
                }
                "u" => {
                    let parsed = Url::parse(body).map_err(|err| {
                        RuntimeError::Message(format!("invalid url literal: {err}"))
                    })?;
                    Ok(Value::Record(Arc::new(url_to_record(&parsed))))
                }
                "d" => {
                    let date = NaiveDate::parse_from_str(body, "%Y-%m-%d").map_err(|err| {
                        RuntimeError::Message(format!("invalid date literal: {err}"))
                    })?;
                    Ok(Value::Record(Arc::new(date_to_record(date))))
                }
                "t" | "dt" => {
                    let _ = chrono::DateTime::parse_from_rfc3339(body).map_err(|err| {
                        RuntimeError::Message(format!("invalid datetime literal: {err}"))
                    })?;
                    Ok(Value::DateTime(body.clone()))
                }
                "k" => {
                    validate_key_text(body).map_err(|msg| {
                        RuntimeError::Message(format!("invalid i18n key literal: {msg}"))
                    })?;
                    let mut map = HashMap::new();
                    map.insert("tag".to_string(), Value::Text(tag.clone()));
                    map.insert("body".to_string(), Value::Text(body.trim().to_string()));
                    map.insert("flags".to_string(), Value::Text(flags.clone()));
                    Ok(Value::Record(Arc::new(map)))
                }
                "m" => {
                    let parsed = parse_message_template(body).map_err(|msg| {
                        RuntimeError::Message(format!("invalid i18n message literal: {msg}"))
                    })?;
                    let mut map = HashMap::new();
                    map.insert("tag".to_string(), Value::Text(tag.clone()));
                    map.insert("body".to_string(), Value::Text(body.clone()));
                    map.insert("flags".to_string(), Value::Text(flags.clone()));
                    map.insert("parts".to_string(), i18n_message_parts_value(&parsed.parts));
                    Ok(Value::Record(Arc::new(map)))
                }
                _ => {
                    let mut map = HashMap::new();
                    map.insert("tag".to_string(), Value::Text(tag.clone()));
                    map.insert("body".to_string(), Value::Text(body.clone()));
                    map.insert("flags".to_string(), Value::Text(flags.clone()));
                    Ok(Value::Record(Arc::new(map)))
                }
            },
            HirExpr::LitBool { value, .. } => Ok(Value::Bool(*value)),
            HirExpr::LitDateTime { text, .. } => Ok(Value::DateTime(text.clone())),
            HirExpr::Lambda { param, body, .. } => Ok(Value::Closure(Arc::new(ClosureValue {
                param: param.clone(),
                body: Arc::new((**body).clone()),
                env: env.clone(),
            }))),
            HirExpr::App { func, arg, .. } => {
                let func_value = self.eval_expr(func, env)?;
                let arg_value = self.eval_expr(arg, env)?;
                self.apply(func_value, arg_value)
            }
            HirExpr::Call { func, args, .. } => {
                let mut func_value = self.eval_expr(func, env)?;
                for arg in args {
                    let arg_value = self.eval_expr(arg, env)?;
                    func_value = self.apply(func_value, arg_value)?;
                }
                Ok(func_value)
            }
            HirExpr::DebugFn {
                fn_name,
                arg_vars,
                log_args,
                log_return,
                log_time,
                body,
                ..
            } => {
                let call_id = self.ctx.next_debug_call_id();
                let start = log_time.then(std::time::Instant::now);

                let ts = log_time.then(now_unix_ms);
                let args_json = if *log_args {
                    Some(
                        arg_vars
                            .iter()
                            .map(|name| {
                                env.get(name)
                                    .as_ref()
                                    .map(|v| debug_value_to_json(v, 0))
                                    .unwrap_or(serde_json::Value::Null)
                            })
                            .collect::<Vec<_>>(),
                    )
                } else {
                    None
                };

                self.debug_stack.push(DebugFrame {
                    fn_name: fn_name.clone(),
                    call_id,
                    start,
                });

                let mut enter = serde_json::Map::new();
                enter.insert("kind".to_string(), serde_json::Value::String("fn.enter".to_string()));
                enter.insert("fn".to_string(), serde_json::Value::String(fn_name.clone()));
                enter.insert(
                    "callId".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(call_id)),
                );
                if let Some(args_json) = args_json {
                    enter.insert("args".to_string(), serde_json::Value::Array(args_json));
                }
                if let Some(ts) = ts {
                    enter.insert(
                        "ts".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(ts)),
                    );
                }
                emit_debug_event(serde_json::Value::Object(enter));

                let result = self.eval_expr(body, env);

                let frame = self.debug_stack.pop();
                if let Some(frame) = frame {
                    let dur_ms = if *log_time {
                        frame
                            .start
                            .map(|s| s.elapsed().as_millis() as u64)
                            .unwrap_or(0)
                    } else {
                        0
                    };

                    let mut exit = serde_json::Map::new();
                    exit.insert("kind".to_string(), serde_json::Value::String("fn.exit".to_string()));
                    exit.insert("fn".to_string(), serde_json::Value::String(frame.fn_name));
                    exit.insert(
                        "callId".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(frame.call_id)),
                    );
                    if *log_return {
                        if let Ok(ref value) = result {
                            exit.insert("ret".to_string(), debug_value_to_json(value, 0));
                        }
                    }
                    if *log_time {
                        exit.insert(
                            "durMs".to_string(),
                            serde_json::Value::Number(serde_json::Number::from(dur_ms)),
                        );
                    }
                    emit_debug_event(serde_json::Value::Object(exit));
                }

                result
            }
            HirExpr::Pipe {
                pipe_id,
                step,
                label,
                log_time,
                func,
                arg,
                ..
            } => {
                let func_value = self.eval_expr(func, env)?;
                let arg_value = self.eval_expr(arg, env)?;

                let Some(frame) = self.debug_stack.last().cloned() else {
                    return self.apply(func_value, arg_value);
                };

                let ts_in = log_time.then(now_unix_ms);
                let mut pipe_in = serde_json::Map::new();
                pipe_in.insert("kind".to_string(), serde_json::Value::String("pipe.in".to_string()));
                pipe_in.insert("fn".to_string(), serde_json::Value::String(frame.fn_name.clone()));
                pipe_in.insert(
                    "callId".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(frame.call_id)),
                );
                pipe_in.insert(
                    "pipeId".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(*pipe_id)),
                );
                pipe_in.insert(
                    "step".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(*step)),
                );
                pipe_in.insert("label".to_string(), serde_json::Value::String(label.clone()));
                pipe_in.insert("value".to_string(), debug_value_to_json(&arg_value, 0));
                if let Some(ts) = ts_in {
                    pipe_in.insert(
                        "ts".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(ts)),
                    );
                }
                emit_debug_event(serde_json::Value::Object(pipe_in));

                let step_start = log_time.then(std::time::Instant::now);
                let out_value = self.apply(func_value, arg_value)?;

                let dur_ms = if *log_time {
                    step_start
                        .map(|s| s.elapsed().as_millis() as u64)
                        .unwrap_or(0)
                } else {
                    0
                };
                let shape = debug_shape_tag(&out_value);

                let mut pipe_out = serde_json::Map::new();
                pipe_out.insert(
                    "kind".to_string(),
                    serde_json::Value::String("pipe.out".to_string()),
                );
                pipe_out.insert("fn".to_string(), serde_json::Value::String(frame.fn_name));
                pipe_out.insert(
                    "callId".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(frame.call_id)),
                );
                pipe_out.insert(
                    "pipeId".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(*pipe_id)),
                );
                pipe_out.insert(
                    "step".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(*step)),
                );
                pipe_out.insert("label".to_string(), serde_json::Value::String(label.clone()));
                pipe_out.insert("value".to_string(), debug_value_to_json(&out_value, 0));
                if *log_time {
                    pipe_out.insert(
                        "durMs".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(dur_ms)),
                    );
                }
                if let Some(shape) = shape {
                    pipe_out.insert("shape".to_string(), serde_json::Value::String(shape));
                }
                emit_debug_event(serde_json::Value::Object(pipe_out));

                Ok(out_value)
            }
            HirExpr::List { items, .. } => self.eval_list(items, env),
            HirExpr::Tuple { items, .. } => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(self.eval_expr(item, env)?);
                }
                Ok(Value::Tuple(values))
            }
            HirExpr::Record { fields, .. } => self.eval_record(fields, env),
            HirExpr::Patch { target, fields, .. } => self.eval_patch(target, fields, env),
            HirExpr::FieldAccess { base, field, .. } => {
                let base_value = self.eval_expr(base, env)?;
                match base_value {
                    Value::Record(map) => map
                        .get(field)
                        .cloned()
                        .ok_or_else(|| RuntimeError::Message(format!("missing field {field}"))),
                    _ => Err(RuntimeError::Message(format!(
                        "field access on non-record {field}"
                    ))),
                }
            }
            HirExpr::Index { base, index, .. } => {
                let base_value = self.eval_expr(base, env)?;
                let index_value = self.eval_expr(index, env)?;
                match base_value {
                    Value::List(items) => {
                        let Value::Int(idx) = index_value else {
                            return Err(RuntimeError::Message(
                                "list index expects an Int".to_string(),
                            ));
                        };
                        let idx = idx as usize;
                        items
                            .get(idx)
                            .cloned()
                            .ok_or_else(|| RuntimeError::Message("index out of bounds".to_string()))
                    }
                    Value::Tuple(items) => {
                        let Value::Int(idx) = index_value else {
                            return Err(RuntimeError::Message(
                                "tuple index expects an Int".to_string(),
                            ));
                        };
                        let idx = idx as usize;
                        items
                            .get(idx)
                            .cloned()
                            .ok_or_else(|| RuntimeError::Message("index out of bounds".to_string()))
                    }
                    Value::Map(entries) => {
                        let Some(key) = KeyValue::try_from_value(&index_value) else {
                            return Err(RuntimeError::Message(format!(
                                "map key is not a valid key type: {}",
                                format_value(&index_value)
                            )));
                        };
                        entries
                            .get(&key)
                            .cloned()
                            .ok_or_else(|| RuntimeError::Message("missing map key".to_string()))
                    }
                    _ => Err(RuntimeError::Message(
                        "index on unsupported value".to_string(),
                    )),
                }
            }
            HirExpr::Match {
                scrutinee, arms, ..
            } => {
                let value = self.eval_expr(scrutinee, env)?;
                self.eval_match(&value, arms, env)
            }
            HirExpr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                let cond_value = self.eval_expr(cond, env)?;
                if matches!(cond_value, Value::Bool(true)) {
                    self.eval_expr(then_branch, env)
                } else {
                    self.eval_expr(else_branch, env)
                }
            }
            HirExpr::Binary {
                op, left, right, ..
            } => {
                let left_value = self.eval_expr(left, env)?;
                let right_value = self.eval_expr(right, env)?;
                self.eval_binary(op, left_value, right_value, env)
            }
            HirExpr::Block {
                block_kind, items, ..
            } => match block_kind {
                crate::hir::HirBlockKind::Plain => self.eval_plain_block(items, env),
                crate::hir::HirBlockKind::Effect => {
                    Ok(Value::Effect(Arc::new(EffectValue::Block {
                        env: env.clone(),
                        items: Arc::new(items.clone()),
                    })))
                }
                crate::hir::HirBlockKind::Resource => {
                    Ok(Value::Resource(Arc::new(ResourceValue {
                        items: Arc::new(items.clone()),
                    })))
                }
                crate::hir::HirBlockKind::Generate => self.eval_generate_block(items, env),
            },
            HirExpr::Raw { .. } => Err(RuntimeError::Message(
                "raw expressions are not supported in native runtime yet".to_string(),
            )),
        }
    }

    fn apply(&mut self, func: Value, arg: Value) -> Result<Value, RuntimeError> {
        let func = self.force_value(func)?;
        match func {
            Value::Closure(closure) => {
                let new_env = Env::new(Some(closure.env.clone()));
                new_env.set(closure.param.clone(), arg);
                self.eval_expr(&closure.body, &new_env)
            }
            Value::Builtin(builtin) => builtin.apply(arg, self),
            Value::MultiClause(clauses) => self.apply_multi_clause(clauses, arg),
            Value::Constructor { name, mut args } => {
                args.push(arg);
                Ok(Value::Constructor { name, args })
            }
            other => Err(RuntimeError::Message(format!(
                "attempted to call a non-function: {}",
                format_value(&other)
            ))),
        }
    }

    fn apply_multi_clause(
        &mut self,
        clauses: Vec<Value>,
        arg: Value,
    ) -> Result<Value, RuntimeError> {
        let mut results = Vec::new();
        let mut match_failures = 0;
        let mut last_error = None;
        for clause in clauses {
            match self.apply(clause.clone(), arg.clone()) {
                Ok(value) => results.push(value),
                Err(RuntimeError::Message(message)) if is_match_failure_message(&message) => {
                    match_failures += 1;
                }
                Err(err) => {
                    last_error = Some(err);
                }
            }
        }
        if !results.is_empty() {
            let mut callable = results
                .iter()
                .filter(|value| is_callable(value))
                .cloned()
                .collect::<Vec<_>>();
            if !callable.is_empty() {
                if callable.len() == 1 {
                    return Ok(callable.remove(0));
                }
                return Ok(Value::MultiClause(callable));
            }
            return Ok(results.remove(0));
        }
        if match_failures > 0 && last_error.is_none() {
            return Err(RuntimeError::Message("non-exhaustive match".to_string()));
        }
        Err(last_error.unwrap_or_else(|| RuntimeError::Message("no matching clause".to_string())))
    }

    fn eval_plain_block(
        &mut self,
        items: &[HirBlockItem],
        env: &Env,
    ) -> Result<Value, RuntimeError> {
        let local_env = Env::new(Some(env.clone()));
        let mut last_value = Value::Unit;
        for (index, item) in items.iter().enumerate() {
            let last = index + 1 == items.len();
            match item {
                HirBlockItem::Bind { pattern, expr } => {
                    let value = self.eval_expr(expr, &local_env)?;
                    let bindings = collect_pattern_bindings(pattern, &value)
                        .ok_or_else(|| RuntimeError::Message("pattern match failed".to_string()))?;
                    for (name, value) in bindings {
                        local_env.set(name, value);
                    }
                    if last {
                        last_value = Value::Unit;
                    }
                }
                HirBlockItem::Expr { expr } => {
                    last_value = self.eval_expr(expr, &local_env)?;
                    if !last {
                        last_value = Value::Unit;
                    }
                }
                HirBlockItem::Filter { .. }
                | HirBlockItem::Yield { .. }
                | HirBlockItem::Recurse { .. } => {
                    return Err(RuntimeError::Message(
                        "unsupported block item in plain block".to_string(),
                    ));
                }
            }
        }
        Ok(last_value)
    }

    fn eval_generate_block(
        &mut self,
        items: &[HirBlockItem],
        env: &Env,
    ) -> Result<Value, RuntimeError> {
        // Eagerly materialize the generator items into a Vec<Value>
        let mut values = Vec::new();
        self.materialize_generate(items, env, &mut values)?;

        // Return a builtin function: \k -> \z -> foldl k z values
        let values = Arc::new(values);
        Ok(Value::Builtin(BuiltinValue {
            imp: Arc::new(BuiltinImpl {
                name: "<generator>".to_string(),
                arity: 2,
                func: Arc::new(move |mut args, runtime| {
                    let z = args.pop().unwrap();
                    let k = args.pop().unwrap();
                    let mut acc = z;
                    for val in values.iter() {
                        // k(acc, x)
                        let partial = runtime.apply(k.clone(), acc)?;
                        acc = runtime.apply(partial, val.clone())?;
                    }
                    Ok(acc)
                }),
            }),
            args: Vec::new(),
        }))
    }

    fn materialize_generate(
        &mut self,
        items: &[HirBlockItem],
        env: &Env,
        out: &mut Vec<Value>,
    ) -> Result<(), RuntimeError> {
        let local_env = Env::new(Some(env.clone()));
        for item in items {
            match item {
                HirBlockItem::Yield { expr } => {
                    let value = self.eval_expr(expr, &local_env)?;
                    out.push(value);
                }
                HirBlockItem::Bind { pattern, expr } => {
                    let source = self.eval_expr(expr, &local_env)?;
                    // The source should be a generator (a builtin that takes k and z).
                    // We need to extract its elements. We do this by folding with a
                    // list-accumulate step function.
                    let source_items = self.generator_to_list(source)?;
                    // For each element from the source, bind it to the pattern
                    // and process the rest of the items in this scope.
                    let rest =
                        &items[items.iter().position(|i| std::ptr::eq(i, item)).unwrap() + 1..];
                    for val in source_items {
                        let bind_env = Env::new(Some(local_env.clone()));
                        let bindings =
                            collect_pattern_bindings(pattern, &val).ok_or_else(|| {
                                RuntimeError::Message(
                                    "pattern match failed in generator bind".to_string(),
                                )
                            })?;
                        for (name, bound_val) in bindings {
                            bind_env.set(name, bound_val);
                        }
                        self.materialize_generate(rest, &bind_env, out)?;
                    }
                    return Ok(());
                }
                HirBlockItem::Filter { expr } => {
                    let cond = self.eval_expr(expr, &local_env)?;
                    if !matches!(cond, Value::Bool(true)) {
                        return Ok(());
                    }
                }
                HirBlockItem::Expr { expr } => {
                    // An expression in a generate block acts as a sub-generator to spread
                    let sub = self.eval_expr(expr, &local_env)?;
                    let sub_items = self.generator_to_list(sub)?;
                    out.extend(sub_items);
                }
                HirBlockItem::Recurse { .. } => {
                    // Unsupported for now
                }
            }
        }
        Ok(())
    }

    fn generator_to_list(&mut self, gen: Value) -> Result<Vec<Value>, RuntimeError> {
        // A generator is a function (k -> z -> R).
        // We fold it with a list-append step: k = \acc x -> acc ++ [x], z = []
        let step = Value::Builtin(BuiltinValue {
            imp: Arc::new(BuiltinImpl {
                name: "<gen_to_list_step>".to_string(),
                arity: 2,
                func: Arc::new(|mut args, _runtime| {
                    let x = args.pop().unwrap();
                    let acc = args.pop().unwrap();
                    let mut list = match acc {
                        Value::List(items) => (*items).clone(),
                        _ => {
                            return Err(RuntimeError::Message(
                                "expected list accumulator".to_string(),
                            ))
                        }
                    };
                    list.push(x);
                    Ok(Value::List(Arc::new(list)))
                }),
            }),
            args: Vec::new(),
        });
        let init = Value::List(Arc::new(Vec::new()));
        let with_step = self.apply(gen, step)?;
        let result = self.apply(with_step, init)?;
        match result {
            Value::List(items) => Ok((*items).clone()),
            _ => Err(RuntimeError::Message(
                "generator fold did not produce a list".to_string(),
            )),
        }
    }

    fn eval_match(
        &mut self,
        value: &Value,
        arms: &[HirMatchArm],
        env: &Env,
    ) -> Result<Value, RuntimeError> {
        for arm in arms {
            if let Some(bindings) = collect_pattern_bindings(&arm.pattern, value) {
                if let Some(guard) = &arm.guard {
                    let guard_env = Env::new(Some(env.clone()));
                    for (name, value) in bindings.clone() {
                        guard_env.set(name, value);
                    }
                    let guard_value = self.eval_expr(guard, &guard_env)?;
                    if !matches!(guard_value, Value::Bool(true)) {
                        continue;
                    }
                }
                let arm_env = Env::new(Some(env.clone()));
                for (name, value) in bindings {
                    arm_env.set(name, value);
                }
                return self.eval_expr(&arm.body, &arm_env);
            }
        }
        Err(RuntimeError::Message("non-exhaustive match".to_string()))
    }

    fn eval_list(&mut self, items: &[HirListItem], env: &Env) -> Result<Value, RuntimeError> {
        let mut values = Vec::new();
        for item in items {
            let value = self.eval_expr(&item.expr, env)?;
            if item.spread {
                match value {
                    Value::List(inner) => values.extend(inner.iter().cloned()),
                    _ => {
                        return Err(RuntimeError::Message(
                            "list spread expects a list".to_string(),
                        ))
                    }
                }
            } else {
                values.push(value);
            }
        }
        Ok(Value::List(Arc::new(values)))
    }

    fn eval_record(&mut self, fields: &[HirRecordField], env: &Env) -> Result<Value, RuntimeError> {
        let mut map = HashMap::new();
        for field in fields {
            let value = self.eval_expr(&field.value, env)?;
            if field.spread {
                match value {
                    Value::Record(inner) => {
                        for (k, v) in inner.as_ref().iter() {
                            map.insert(k.clone(), v.clone());
                        }
                    }
                    _ => {
                        return Err(RuntimeError::Message(
                            "record spread expects a record".to_string(),
                        ))
                    }
                }
                continue;
            }
            insert_record_path(&mut map, &field.path, value)?;
        }
        Ok(Value::Record(Arc::new(map)))
    }

    fn eval_patch(
        &mut self,
        target: &HirExpr,
        fields: &[HirRecordField],
        env: &Env,
    ) -> Result<Value, RuntimeError> {
        let base_value = self.eval_expr(target, env)?;
        let Value::Record(map) = base_value else {
            return Err(RuntimeError::Message(
                "patch target must be a record".to_string(),
            ));
        };
        let mut map = map.as_ref().clone();
        for field in fields {
            if field.spread {
                return Err(RuntimeError::Message(
                    "patch fields do not support record spread".to_string(),
                ));
            }
            self.apply_patch_field(&mut map, &field.path, &field.value, env)?;
        }
        Ok(Value::Record(Arc::new(map)))
    }

    fn apply_patch_field(
        &mut self,
        record: &mut HashMap<String, Value>,
        path: &[HirPathSegment],
        expr: &HirExpr,
        env: &Env,
    ) -> Result<(), RuntimeError> {
        if path.is_empty() {
            return Err(RuntimeError::Message(
                "patch field path must not be empty".to_string(),
            ));
        }
        let mut current = record;
        for segment in &path[..path.len() - 1] {
            match segment {
                HirPathSegment::Field(name) => {
                    let entry = current
                        .entry(name.clone())
                        .or_insert_with(|| Value::Record(Arc::new(HashMap::new())));
                    match entry {
                        Value::Record(map) => {
                            current = Arc::make_mut(map);
                        }
                        _ => {
                            return Err(RuntimeError::Message(format!(
                                "patch path conflict at {name}"
                            )))
                        }
                    }
                }
                HirPathSegment::Index(_) | HirPathSegment::All => {
                    return Err(RuntimeError::Message(
                        "patch index paths are not supported in native runtime yet".to_string(),
                    ))
                }
            }
        }
        let segment = path.last().unwrap();
        match segment {
            HirPathSegment::Field(name) => {
                let existing = current.get(name).cloned();
                let value = self.eval_expr(expr, env)?;
                let new_value = match existing {
                    Some(existing) if is_callable(&value) => self.apply(value, existing)?,
                    Some(_) | None if is_callable(&value) => {
                        return Err(RuntimeError::Message(format!(
                            "patch transform expects existing field {name}"
                        )));
                    }
                    _ => value,
                };
                current.insert(name.clone(), new_value);
                Ok(())
            }
            HirPathSegment::Index(_) | HirPathSegment::All => Err(RuntimeError::Message(
                "patch index paths are not supported in native runtime yet".to_string(),
            )),
        }
    }

    fn eval_binary(
        &mut self,
        op: &str,
        left: Value,
        right: Value,
        env: &Env,
    ) -> Result<Value, RuntimeError> {
        if let Some(result) = eval_binary_builtin(op, &left, &right) {
            return Ok(result);
        }
        let op_name = format!("({})", op);
        if let Some(op_value) = env.get(&op_name) {
            let applied = self.apply(op_value, left)?;
            return self.apply(applied, right);
        }
        Err(RuntimeError::Message(format!(
            "unsupported binary operator {op}"
        )))
    }

    fn run_effect_value(&mut self, value: Value) -> Result<Value, RuntimeError> {
        self.check_cancelled()?;
        match value {
            Value::Effect(effect) => match effect.as_ref() {
                EffectValue::Block { env, items } => {
                    self.run_effect_block(env.clone(), items.as_ref())
                }
                EffectValue::Thunk { func } => func(self),
            },
            other => Err(RuntimeError::Message(format!(
                "expected Effect, got {}",
                format_value(&other)
            ))),
        }
    }

    fn run_effect_block(
        &mut self,
        env: Env,
        items: &[HirBlockItem],
    ) -> Result<Value, RuntimeError> {
        let local_env = Env::new(Some(env));
        let mut cleanups: Vec<Value> = Vec::new();
        let mut result: Result<Value, RuntimeError> = Ok(Value::Unit);
        let trace_effect = std::env::var("AIVI_TRACE_EFFECT").is_ok_and(|v| v == "1");

        for (index, item) in items.iter().enumerate() {
            let last = index + 1 == items.len();
            if trace_effect {
                eprintln!("[AIVI_TRACE_EFFECT] step {} / {}", index + 1, items.len());
            }
            if let Err(err) = self.check_cancelled() {
                result = Err(err);
                break;
            }
            let step = match item {
                HirBlockItem::Bind { pattern, expr } => {
                    let value = self.eval_expr(expr, &local_env)?;
                    match value {
                        Value::Resource(resource) => {
                            let (res_value, cleanup) =
                                self.acquire_resource(resource, &local_env)?;
                            let bindings = collect_pattern_bindings(pattern, &res_value)
                                .ok_or_else(|| {
                                    RuntimeError::Message(
                                        "pattern match failed in resource bind".to_string(),
                                    )
                                })?;
                            for (name, value) in bindings {
                                local_env.set(name, value);
                            }
                            cleanups.push(cleanup);
                            Ok(Value::Unit)
                        }
                        Value::Effect(_) => {
                            let value = self.run_effect_value(value)?;
                            let bindings =
                                collect_pattern_bindings(pattern, &value).ok_or_else(|| {
                                    RuntimeError::Message("pattern match failed".to_string())
                                })?;
                            for (name, value) in bindings {
                                local_env.set(name, value);
                            }
                            Ok(Value::Unit)
                        }
                        other => {
                            let bindings =
                                collect_pattern_bindings(pattern, &other).ok_or_else(|| {
                                    RuntimeError::Message("pattern match failed".to_string())
                                })?;
                            for (name, value) in bindings {
                                local_env.set(name, value);
                            }
                            Ok(Value::Unit)
                        }
                    }
                }
                HirBlockItem::Expr { expr } => {
                    let value = self.eval_expr(expr, &local_env)?;
                    if last {
                        match value {
                            Value::Effect(_) => self.run_effect_value(value),
                            _ => Err(RuntimeError::Message(
                                "final expression in effect block must be Effect".to_string(),
                            )),
                        }
                    } else {
                        match value {
                            Value::Effect(_) => {
                                let _ = self.run_effect_value(value)?;
                                Ok(Value::Unit)
                            }
                            _ => Err(RuntimeError::Message(
                                "expression in effect block must be Effect".to_string(),
                            )),
                        }
                    }
                }
                HirBlockItem::Filter { .. }
                | HirBlockItem::Yield { .. }
                | HirBlockItem::Recurse { .. } => Err(RuntimeError::Message(
                    "unsupported block item in effect block".to_string(),
                )),
            };
            match step {
                Ok(value) => {
                    if last {
                        result = Ok(value);
                    }
                }
                Err(err) => {
                    result = Err(err);
                    break;
                }
            }
        }

        let cleanup_result = self.run_cleanups(cleanups);
        match (result, cleanup_result) {
            (Err(err), _) => Err(err),
            (Ok(_), Err(err)) => Err(err),
            (Ok(value), Ok(())) => Ok(value),
        }
    }

    fn acquire_resource(
        &mut self,
        resource: Arc<ResourceValue>,
        env: &Env,
    ) -> Result<(Value, Value), RuntimeError> {
        let local_env = Env::new(Some(env.clone()));
        let items = resource.items.as_ref();
        let mut yielded = None;
        let mut cleanup_start = None;

        for (index, item) in items.iter().enumerate() {
            self.check_cancelled()?;
            match item {
                HirBlockItem::Bind { pattern, expr } => {
                    let value = self.eval_expr(expr, &local_env)?;
                    match value {
                        Value::Effect(_) => {
                            let value = self.run_effect_value(value)?;
                            let bindings =
                                collect_pattern_bindings(pattern, &value).ok_or_else(|| {
                                    RuntimeError::Message("pattern match failed".to_string())
                                })?;
                            for (name, value) in bindings {
                                local_env.set(name, value);
                            }
                        }
                        _ => {
                            return Err(RuntimeError::Message(
                                "resource bind expects Effect".to_string(),
                            ))
                        }
                    }
                }
                HirBlockItem::Yield { expr } => {
                    let value = self.eval_expr(expr, &local_env)?;
                    yielded = Some(value);
                    cleanup_start = Some(index + 1);
                    break;
                }
                HirBlockItem::Expr { expr } => {
                    let value = self.eval_expr(expr, &local_env)?;
                    if let Value::Effect(_) = value {
                        let _ = self.run_effect_value(value)?;
                    }
                }
                HirBlockItem::Filter { .. } | HirBlockItem::Recurse { .. } => {
                    return Err(RuntimeError::Message(
                        "unsupported block item in resource block".to_string(),
                    ));
                }
            }
        }

        let value = yielded
            .ok_or_else(|| RuntimeError::Message("resource block missing yield".to_string()))?;
        let cleanup_items = if let Some(start) = cleanup_start {
            items[start..].to_vec()
        } else {
            Vec::new()
        };
        let cleanup_effect = Value::Effect(Arc::new(EffectValue::Block {
            env: local_env,
            items: Arc::new(cleanup_items),
        }));
        Ok((value, cleanup_effect))
    }

    fn run_cleanups(&mut self, cleanups: Vec<Value>) -> Result<(), RuntimeError> {
        for cleanup in cleanups.into_iter().rev() {
            let _ = self.uncancelable(|runtime| runtime.run_effect_value(cleanup))?;
        }
        Ok(())
    }
}

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

fn record_get_path<'a>(record: &'a HashMap<String, Value>, path: &[String]) -> Option<&'a Value> {
    let mut current = record;
    let mut value = None;
    for (index, segment) in path.iter().enumerate() {
        value = current.get(segment);
        if index + 1 == path.len() {
            return value;
        }
        match value {
            Some(Value::Record(map)) => current = map.as_ref(),
            _ => return None,
        }
    }
    value
}

fn insert_record_path(
    record: &mut HashMap<String, Value>,
    path: &[HirPathSegment],
    value: Value,
) -> Result<(), RuntimeError> {
    if path.is_empty() {
        return Err(RuntimeError::Message(
            "record path must contain at least one segment".to_string(),
        ));
    }
    let mut current = record;
    for (index, segment) in path.iter().enumerate() {
        match segment {
            HirPathSegment::Field(name) => {
                if index + 1 == path.len() {
                    current.insert(name.clone(), value);
                    return Ok(());
                }
                let entry = current
                    .entry(name.clone())
                    .or_insert_with(|| Value::Record(Arc::new(HashMap::new())));
                match entry {
                    Value::Record(map) => {
                        current = Arc::make_mut(map);
                    }
                    _ => {
                        return Err(RuntimeError::Message(format!(
                            "record path conflict at {name}"
                        )))
                    }
                }
            }
            HirPathSegment::Index(_) | HirPathSegment::All => {
                return Err(RuntimeError::Message(
                    "record index paths are not supported in native runtime yet".to_string(),
                ))
            }
        }
    }
    Ok(())
}

fn eval_binary_builtin(op: &str, left: &Value, right: &Value) -> Option<Value> {
    match (op, left, right) {
        ("+", Value::Int(a), Value::Int(b)) => Some(Value::Int(a + b)),
        ("-", Value::Int(a), Value::Int(b)) => Some(Value::Int(a - b)),
        ("*", Value::Int(a), Value::Int(b)) => Some(Value::Int(a * b)),
        ("/", Value::Int(a), Value::Int(b)) => Some(Value::Int(a / b)),
        ("+", Value::Float(a), Value::Float(b)) => Some(Value::Float(a + b)),
        ("-", Value::Float(a), Value::Float(b)) => Some(Value::Float(a - b)),
        ("*", Value::Float(a), Value::Float(b)) => Some(Value::Float(a * b)),
        ("/", Value::Float(a), Value::Float(b)) => Some(Value::Float(a / b)),
        ("==", a, b) => Some(Value::Bool(values_equal(a, b))),
        ("!=", a, b) => Some(Value::Bool(!values_equal(a, b))),
        ("<", Value::Int(a), Value::Int(b)) => Some(Value::Bool(a < b)),
        ("<=", Value::Int(a), Value::Int(b)) => Some(Value::Bool(a <= b)),
        (">", Value::Int(a), Value::Int(b)) => Some(Value::Bool(a > b)),
        (">=", Value::Int(a), Value::Int(b)) => Some(Value::Bool(a >= b)),
        ("<", Value::Float(a), Value::Float(b)) => Some(Value::Bool(a < b)),
        ("<=", Value::Float(a), Value::Float(b)) => Some(Value::Bool(a <= b)),
        (">", Value::Float(a), Value::Float(b)) => Some(Value::Bool(a > b)),
        (">=", Value::Float(a), Value::Float(b)) => Some(Value::Bool(a >= b)),
        ("&&", Value::Bool(a), Value::Bool(b)) => Some(Value::Bool(*a && *b)),
        ("||", Value::Bool(a), Value::Bool(b)) => Some(Value::Bool(*a || *b)),
        _ => None,
    }
}

fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Unit, Value::Unit) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Text(a), Value::Text(b)) => a == b,
        (Value::DateTime(a), Value::DateTime(b)) => a == b,
        (Value::Bytes(a), Value::Bytes(b)) => a == b,
        (Value::Regex(a), Value::Regex(b)) => a.as_str() == b.as_str(),
        (Value::BigInt(a), Value::BigInt(b)) => a == b,
        (Value::Rational(a), Value::Rational(b)) => a == b,
        (Value::Decimal(a), Value::Decimal(b)) => a == b,
        (Value::Map(a), Value::Map(b)) => {
            a.len() == b.len()
                && a.iter().all(|(key, value)| {
                    b.get(key)
                        .map(|other| values_equal(value, other))
                        .unwrap_or(false)
                })
        }
        (Value::Set(a), Value::Set(b)) => a.len() == b.len() && a.iter().all(|key| b.contains(key)),
        (Value::Queue(a), Value::Queue(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|(left, right)| values_equal(left, right))
        }
        (Value::Deque(a), Value::Deque(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|(left, right)| values_equal(left, right))
        }
        (Value::List(a), Value::List(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|(left, right)| values_equal(left, right))
        }
        (Value::Tuple(a), Value::Tuple(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|(left, right)| values_equal(left, right))
        }
        (Value::Record(a), Value::Record(b)) => {
            a.len() == b.len()
                && a.iter().all(|(key, value)| {
                    b.get(key)
                        .map(|other| values_equal(value, other))
                        .unwrap_or(false)
                })
        }
        (Value::Heap(a), Value::Heap(b)) => {
            if a.len() != b.len() {
                return false;
            }
            let mut left: Vec<_> = a.iter().cloned().collect();
            let mut right: Vec<_> = b.iter().cloned().collect();
            left.sort();
            right.sort();
            left == right
        }
        (Value::Constructor { name: a, args: aa }, Value::Constructor { name: b, args: bb }) => {
            a == b
                && aa.len() == bb.len()
                && aa.iter().zip(bb.iter()).all(|(x, y)| values_equal(x, y))
        }
        _ => false,
    }
}

fn parse_number_literal(text: &str) -> Option<i64> {
    if text.contains('.') {
        return None;
    }
    if text.chars().any(|ch| !(ch.is_ascii_digit() || ch == '-')) {
        return None;
    }
    text.parse::<i64>().ok()
}

fn parse_number_value(text: &str) -> Option<Value> {
    if let Some(int) = parse_number_literal(text) {
        Some(Value::Int(int))
    } else if let Ok(float) = text.parse::<f64>() {
        Some(Value::Float(float))
    } else {
        None
    }
}

fn constructor_segment(name: &str) -> Option<&str> {
    let seg = name.rsplit('.').next().unwrap_or(name);
    let ok = seg
        .chars()
        .next()
        .map(|ch| ch.is_ascii_uppercase())
        .unwrap_or(false);
    if ok { Some(seg) } else { None }
}

fn is_callable(value: &Value) -> bool {
    matches!(
        value,
        Value::Closure(_) | Value::Builtin(_) | Value::MultiClause(_)
    )
}

fn is_match_failure_message(message: &str) -> bool {
    message == "non-exhaustive match"
}

const DEBUG_MAX_CHARS: usize = 200;
const DEBUG_MAX_DEPTH: usize = 3;
const DEBUG_MAX_LIST_ITEMS: usize = 20;

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|dur| dur.as_millis() as u64)
        .unwrap_or(0)
}

fn emit_debug_event(event: serde_json::Value) {
    // Emit JSONL-friendly structured logs to stderr by default.
    if let Ok(line) = serde_json::to_string(&event) {
        eprintln!("{line}");
    }
}

fn debug_shape_tag(value: &Value) -> Option<String> {
    match value {
        Value::Constructor { name, args } if args.is_empty() => match name.as_str() {
            "None" | "Some" | "Ok" | "Err" => Some(name.clone()),
            _ => None,
        },
        Value::Constructor { name, args } if args.len() == 1 => match name.as_str() {
            "Some" | "Ok" | "Err" => Some(name.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn debug_value_to_json(value: &Value, depth: usize) -> serde_json::Value {
    if let Value::Constructor { name, args } = value {
        if name == "Sensitive" && args.len() == 1 {
            return serde_json::Value::String("<redacted>".to_string());
        }
    }

    if depth >= DEBUG_MAX_DEPTH {
        return debug_summary_json(value);
    }

    match value {
        Value::Unit => serde_json::Value::String("Unit".to_string()),
        Value::Bool(true) => serde_json::Value::String("True".to_string()),
        Value::Bool(false) => serde_json::Value::String("False".to_string()),
        Value::Int(v) => serde_json::Value::String(v.to_string()),
        Value::Float(v) => serde_json::Value::String(v.to_string()),
        Value::Text(t) => serde_json::Value::String(truncate_debug_text(t)),
        Value::DateTime(t) => serde_json::Value::String(truncate_debug_text(t)),
        Value::Bytes(bytes) => serde_json::Value::String(format!("<bytes:{}>", bytes.len())),
        Value::Regex(regex) => serde_json::Value::String(format!("<regex:{}>", regex.as_str())),
        Value::BigInt(v) => serde_json::Value::String(v.to_string()),
        Value::Rational(v) => serde_json::Value::String(v.to_string()),
        Value::Decimal(v) => serde_json::Value::String(v.to_string()),
        Value::Map(entries) => serde_json::Value::Object(
            [
                (
                    "type".to_string(),
                    serde_json::Value::String("Map".to_string()),
                ),
                (
                    "summary".to_string(),
                    serde_json::Value::String("<opaque>".to_string()),
                ),
                (
                    "size".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(entries.len())),
                ),
            ]
            .into_iter()
            .collect(),
        ),
        Value::Set(entries) => serde_json::Value::Object(
            [
                (
                    "type".to_string(),
                    serde_json::Value::String("Set".to_string()),
                ),
                (
                    "summary".to_string(),
                    serde_json::Value::String("<opaque>".to_string()),
                ),
                (
                    "size".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(entries.len())),
                ),
            ]
            .into_iter()
            .collect(),
        ),
        Value::Queue(items) => serde_json::Value::String(format!("<queue:{}>", items.len())),
        Value::Deque(items) => serde_json::Value::String(format!("<deque:{}>", items.len())),
        Value::Heap(items) => serde_json::Value::String(format!("<heap:{}>", items.len())),
        Value::List(items) => {
            let mut parts = Vec::new();
            for item in items.iter().take(DEBUG_MAX_LIST_ITEMS) {
                parts.push(debug_value_to_json(item, depth + 1));
            }
            let mut out = serde_json::Map::new();
            out.insert("type".to_string(), serde_json::Value::String("List".to_string()));
            out.insert(
                "size".to_string(),
                serde_json::Value::Number(serde_json::Number::from(items.len())),
            );
            out.insert("summary".to_string(), serde_json::Value::Array(parts));
            serde_json::Value::Object(out)
        }
        Value::Tuple(items) => {
            let mut parts = Vec::new();
            for item in items.iter().take(DEBUG_MAX_LIST_ITEMS) {
                parts.push(debug_value_to_json(item, depth + 1));
            }
            let mut out = serde_json::Map::new();
            out.insert("type".to_string(), serde_json::Value::String("Tuple".to_string()));
            out.insert(
                "size".to_string(),
                serde_json::Value::Number(serde_json::Number::from(items.len())),
            );
            out.insert("summary".to_string(), serde_json::Value::Array(parts));
            serde_json::Value::Object(out)
        }
        Value::Record(fields) => {
            let mut keys: Vec<&String> = fields.keys().collect();
            keys.sort();
            let mut out_fields = serde_json::Map::new();
            for key in keys.into_iter().take(DEBUG_MAX_LIST_ITEMS) {
                if let Some(val) = fields.get(key) {
                    out_fields.insert(key.clone(), debug_value_to_json(val, depth + 1));
                }
            }
            let mut out = serde_json::Map::new();
            out.insert(
                "type".to_string(),
                serde_json::Value::String("Record".to_string()),
            );
            out.insert(
                "size".to_string(),
                serde_json::Value::Number(serde_json::Number::from(fields.len())),
            );
            out.insert("summary".to_string(), serde_json::Value::Object(out_fields));
            serde_json::Value::Object(out)
        }
        Value::Constructor { name, args } => {
            if args.is_empty() {
                serde_json::Value::String(name.clone())
            } else {
                let mut out = serde_json::Map::new();
                out.insert("type".to_string(), serde_json::Value::String(name.clone()));
                out.insert(
                    "size".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(args.len())),
                );
                out.insert(
                    "summary".to_string(),
                    serde_json::Value::Array(
                        args.iter()
                            .take(DEBUG_MAX_LIST_ITEMS)
                            .map(|arg| debug_value_to_json(arg, depth + 1))
                            .collect(),
                    ),
                );
                serde_json::Value::Object(out)
            }
        }
        Value::Closure(_) => debug_summary_json(value),
        Value::Builtin(builtin) => serde_json::Value::Object(
            [
                (
                    "type".to_string(),
                    serde_json::Value::String("Builtin".to_string()),
                ),
                (
                    "summary".to_string(),
                    serde_json::Value::String(format!("<builtin:{}>", builtin.imp.name)),
                ),
            ]
            .into_iter()
            .collect(),
        ),
        Value::Effect(_) => debug_summary_json(value),
        Value::Resource(_) => debug_summary_json(value),
        Value::Thunk(_) => debug_summary_json(value),
        Value::MultiClause(_) => debug_summary_json(value),
        Value::ChannelSend(_) => debug_summary_json(value),
        Value::ChannelRecv(_) => debug_summary_json(value),
        Value::FileHandle(_) => debug_summary_json(value),
        Value::Listener(_) => debug_summary_json(value),
        Value::Connection(_) => debug_summary_json(value),
        Value::Stream(_) => debug_summary_json(value),
        Value::HttpServer(_) => debug_summary_json(value),
        Value::WebSocket(_) => debug_summary_json(value),
    }
}

fn truncate_debug_text(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars().take(DEBUG_MAX_CHARS) {
        out.push(ch);
    }
    if text.chars().count() > DEBUG_MAX_CHARS {
        out.push_str("...");
    }
    out
}

fn debug_summary_json(value: &Value) -> serde_json::Value {
    let (ty, size) = match value {
        Value::Unit => ("Unit", None),
        Value::Bool(_) => ("Bool", None),
        Value::Int(_) => ("Int", None),
        Value::Float(_) => ("Float", None),
        Value::Text(_) => ("Text", None),
        Value::DateTime(_) => ("DateTime", None),
        Value::Bytes(bytes) => ("Bytes", Some(bytes.len())),
        Value::Regex(_) => ("Regex", None),
        Value::BigInt(_) => ("BigInt", None),
        Value::Rational(_) => ("Rational", None),
        Value::Decimal(_) => ("Decimal", None),
        Value::Map(entries) => ("Map", Some(entries.len())),
        Value::Set(entries) => ("Set", Some(entries.len())),
        Value::Queue(items) => ("Queue", Some(items.len())),
        Value::Deque(items) => ("Deque", Some(items.len())),
        Value::Heap(items) => ("Heap", Some(items.len())),
        Value::List(items) => ("List", Some(items.len())),
        Value::Tuple(items) => ("Tuple", Some(items.len())),
        Value::Record(fields) => ("Record", Some(fields.len())),
        Value::Constructor { name, args } => (name.as_str(), Some(args.len())),
        Value::Closure(_) => ("Closure", None),
        Value::Builtin(_) => ("Builtin", None),
        Value::Effect(_) => ("Effect", None),
        Value::Resource(_) => ("Resource", None),
        Value::Thunk(_) => ("Thunk", None),
        Value::MultiClause(_) => ("MultiClause", None),
        Value::ChannelSend(_) => ("Send", None),
        Value::ChannelRecv(_) => ("Recv", None),
        Value::FileHandle(_) => ("File", None),
        Value::Listener(_) => ("Listener", None),
        Value::Connection(_) => ("Connection", None),
        Value::Stream(_) => ("Stream", None),
        Value::HttpServer(_) => ("HttpServer", None),
        Value::WebSocket(_) => ("WebSocket", None),
    };

    let mut out = serde_json::Map::new();
    out.insert("type".to_string(), serde_json::Value::String(ty.to_string()));
    out.insert(
        "summary".to_string(),
        serde_json::Value::String("<opaque>".to_string()),
    );
    if let Some(size) = size {
        out.insert(
            "size".to_string(),
            serde_json::Value::Number(serde_json::Number::from(size)),
        );
    }
    serde_json::Value::Object(out)
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Unit => "Unit".to_string(),
        Value::Bool(value) => {
            if *value {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Value::Int(value) => value.to_string(),
        Value::Float(value) => value.to_string(),
        Value::Text(value) => value.clone(),
        Value::DateTime(value) => value.clone(),
        Value::Bytes(bytes) => format!("<bytes:{}>", bytes.len()),
        Value::Regex(regex) => format!("<regex:{}>", regex.as_str()),
        Value::BigInt(value) => value.to_string(),
        Value::Rational(value) => value.to_string(),
        Value::Decimal(value) => value.to_string(),
        Value::Map(entries) => format!("<map:{}>", entries.len()),
        Value::Set(entries) => format!("<set:{}>", entries.len()),
        Value::Queue(items) => format!("<queue:{}>", items.len()),
        Value::Deque(items) => format!("<deque:{}>", items.len()),
        Value::Heap(items) => format!("<heap:{}>", items.len()),
        Value::List(items) => format!(
            "[{}]",
            items
                .iter()
                .map(format_value)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(format_value)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Record(_) => "{...}".to_string(),
        Value::Constructor { name, args } => {
            if args.is_empty() {
                name.clone()
            } else {
                format!(
                    "{} {}",
                    name,
                    args.iter().map(format_value).collect::<Vec<_>>().join(" ")
                )
            }
        }
        Value::Closure(_) => "<closure>".to_string(),
        Value::Builtin(builtin) => format!("<builtin:{}>", builtin.imp.name),
        Value::Effect(_) => "<effect>".to_string(),
        Value::Resource(_) => "<resource>".to_string(),
        Value::Thunk(_) => "<thunk>".to_string(),
        Value::MultiClause(_) => "<multi-clause>".to_string(),
        Value::ChannelSend(_) => "<send>".to_string(),
        Value::ChannelRecv(_) => "<recv>".to_string(),
        Value::FileHandle(_) => "<file>".to_string(),
        Value::Listener(_) => "<listener>".to_string(),
        Value::Connection(_) => "<connection>".to_string(),
        Value::Stream(_) => "<stream>".to_string(),
        Value::HttpServer(_) => "<http-server>".to_string(),
        Value::WebSocket(_) => "<websocket>".to_string(),
    }
}

fn date_to_record(date: NaiveDate) -> HashMap<String, Value> {
    let mut map = HashMap::new();
    map.insert("year".to_string(), Value::Int(date.year() as i64));
    map.insert("month".to_string(), Value::Int(date.month() as i64));
    map.insert("day".to_string(), Value::Int(date.day() as i64));
    map
}

fn url_to_record(url: &Url) -> HashMap<String, Value> {
    let mut map = HashMap::new();
    map.insert(
        "protocol".to_string(),
        Value::Text(url.scheme().to_string()),
    );
    map.insert(
        "host".to_string(),
        Value::Text(url.host_str().unwrap_or("").to_string()),
    );
    let port = match url.port() {
        Some(port) => Value::Constructor {
            name: "Some".to_string(),
            args: vec![Value::Int(port as i64)],
        },
        None => Value::Constructor {
            name: "None".to_string(),
            args: Vec::new(),
        },
    };
    map.insert("port".to_string(), port);
    map.insert("path".to_string(), Value::Text(url.path().to_string()));
    let mut query_items = Vec::new();
    for (key, value) in url.query_pairs() {
        query_items.push(Value::Tuple(vec![
            Value::Text(key.to_string()),
            Value::Text(value.to_string()),
        ]));
    }
    map.insert("query".to_string(), Value::List(Arc::new(query_items)));
    let hash = match url.fragment() {
        Some(fragment) => Value::Constructor {
            name: "Some".to_string(),
            args: vec![Value::Text(fragment.to_string())],
        },
        None => Value::Constructor {
            name: "None".to_string(),
            args: Vec::new(),
        },
    };
    map.insert("hash".to_string(), hash);
    map
}

fn i18n_message_parts_value(parts: &[MessagePart]) -> Value {
    let mut out = Vec::with_capacity(parts.len());
    for part in parts {
        match part {
            MessagePart::Lit(text) => {
                out.push(Value::Record(Arc::new(HashMap::from([
                    ("kind".to_string(), Value::Text("lit".to_string())),
                    ("text".to_string(), Value::Text(text.clone())),
                ]))));
            }
            MessagePart::Hole { name, ty } => {
                let ty_value = match ty {
                    Some(t) => Value::Constructor {
                        name: "Some".to_string(),
                        args: vec![Value::Text(t.clone())],
                    },
                    None => Value::Constructor {
                        name: "None".to_string(),
                        args: Vec::new(),
                    },
                };
                out.push(Value::Record(Arc::new(HashMap::from([
                    ("kind".to_string(), Value::Text("hole".to_string())),
                    ("name".to_string(), Value::Text(name.clone())),
                    ("ty".to_string(), ty_value),
                ]))));
            }
        }
    }
    Value::List(Arc::new(out))
}
