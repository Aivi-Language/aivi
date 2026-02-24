use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Datelike, NaiveDate, Timelike, TimeZone as ChronoTimeZone};
use regex::RegexBuilder;
use url::Url;

use crate::hir::{
    HirBlockItem, HirExpr, HirListItem, HirLiteral, HirMatchArm, HirPathSegment, HirPattern,
    HirProgram, HirRecordField, HirTextPart,
};
use crate::i18n::{parse_message_template, validate_key_text, MessagePart};
use crate::rust_ir;
use crate::AiviError;

mod builtins;
pub(crate) mod environment;
mod http;
#[cfg(test)]
mod tests;
pub(crate) mod values;

use self::builtins::register_builtins;
use self::environment::{Env, MachineEdge, RuntimeContext};
use self::values::{
    BuiltinImpl, BuiltinValue, ClosureValue, EffectValue, KeyValue, ResourceValue, SourceValue,
    TaggedValue, ThunkValue, Value, shape_record,
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
        self.local.store(true, Ordering::Release);
    }

    fn parent(&self) -> Option<Arc<CancelToken>> {
        self.parent.clone()
    }

    fn is_cancelled(&self) -> bool {
        if self.local.load(Ordering::Relaxed) {
            return true;
        }
        self.parent
            .as_ref()
            .is_some_and(|parent| parent.is_cancelled())
    }
}

pub(crate) struct Runtime {
    pub(crate) ctx: Arc<RuntimeContext>,
    cancel: Arc<CancelToken>,
    cancel_mask: usize,
    pub(crate) fuel: Option<u64>,
    rng_state: u64,
    debug_stack: Vec<DebugFrame>,
    /// Counter used to amortize cancel-token checks (checked every 64 evals).
    check_counter: u32,
    #[cfg(test)]
    eval_expr_call_count: usize,
}

#[derive(Clone)]
struct DebugFrame {
    fn_name: String,
    call_id: u64,
    start: Option<std::time::Instant>,
}

#[derive(Clone)]
pub(crate) enum RuntimeError {
    Error(Value),
    Cancelled,
    Message(String),
}

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


pub(crate) fn run_main_effect(runtime: &mut Runtime) -> Result<(), AiviError> {
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


// RustIR interpreter: retained for existing test coverage. Production execution
// routes through the Cranelift JIT backend (cranelift_backend/).
#[allow(dead_code)]
fn eval_runtime_rust_ir_expr(
    runtime: &mut Runtime,
    expr: &rust_ir::RustIrExpr,
    env: &Env,
) -> Result<Value, RuntimeError> {
    runtime.check_cancelled()?;
    match expr {
        rust_ir::RustIrExpr::Local { name, .. } => {
            let value = env
                .get(name)
                .ok_or_else(|| RuntimeError::Message(format!("unknown local {name}")))?;
            runtime.force_value(value)
        }
        rust_ir::RustIrExpr::Global { name, .. } => {
            let value = runtime
                .ctx
                .globals
                .get(name)
                .ok_or_else(|| RuntimeError::Message(format!("unknown global {name}")))?;
            runtime.force_value(value)
        }
        rust_ir::RustIrExpr::Builtin { builtin, .. } => {
            let value = runtime
                .ctx
                .globals
                .get(builtin)
                .ok_or_else(|| RuntimeError::Message(format!("unknown builtin {builtin}")))?;
            runtime.force_value(value)
        }
        rust_ir::RustIrExpr::ConstructorValue { name, .. } => Ok(Value::Constructor {
            name: name.clone(),
            args: Vec::new(),
        }),
        rust_ir::RustIrExpr::LitNumber { text, .. } => {
            if let Some(value) = parse_number_value(text) {
                return Ok(value);
            }
            let value = env.get(text).ok_or_else(|| {
                RuntimeError::Message(format!("unknown numeric literal {text}"))
            })?;
            runtime.force_value(value)
        }
        rust_ir::RustIrExpr::LitString { text, .. } => Ok(Value::Text(text.clone())),
        rust_ir::RustIrExpr::TextInterpolate { parts, .. } => {
            let mut out = String::new();
            for part in parts {
                match part {
                    rust_ir::RustIrTextPart::Text { text } => out.push_str(text),
                    rust_ir::RustIrTextPart::Expr { expr } => {
                        let value = eval_runtime_rust_ir_expr(runtime, expr, env)?;
                        out.push_str(&format_value(&value));
                    }
                }
            }
            Ok(Value::Text(out))
        }
        rust_ir::RustIrExpr::LitSigil {
            tag, body, flags, ..
        } => eval_runtime_sigil_literal(tag, body, flags),
        rust_ir::RustIrExpr::LitBool { value, .. } => Ok(Value::Bool(*value)),
        rust_ir::RustIrExpr::LitDateTime { text, .. } => Ok(Value::DateTime(text.clone())),
        rust_ir::RustIrExpr::Lambda {
            id, param, body, ..
        } => {
            let lambda_name = format!("__jit|lambda|{id}");
            let param = param.clone();
            let body = Arc::new((**body).clone());
            let captured_env = env.clone();
            Ok(runtime_builtin(&lambda_name, 1, move |mut args, runtime| {
                let arg = args.pop().unwrap_or(Value::Unit);
                let lambda_env = Env::new(Some(captured_env.clone()));
                lambda_env.set(param.clone(), arg);
                eval_runtime_rust_ir_expr(runtime, body.as_ref(), &lambda_env)
            }))
        }
        rust_ir::RustIrExpr::App { func, arg, .. } => {
            let func_value = eval_runtime_rust_ir_expr(runtime, func, env)?;
            let arg_value = eval_runtime_rust_ir_expr(runtime, arg, env)?;
            runtime.apply(func_value, arg_value)
        }
        rust_ir::RustIrExpr::Call { func, args, .. } => {
            let mut func_value = eval_runtime_rust_ir_expr(runtime, func, env)?;
            for arg in args {
                let arg_value = eval_runtime_rust_ir_expr(runtime, arg, env)?;
                func_value = runtime.apply(func_value, arg_value)?;
            }
            Ok(func_value)
        }
        rust_ir::RustIrExpr::DebugFn {
            fn_name,
            arg_vars,
            log_args,
            log_return,
            log_time,
            body,
            ..
        } => {
            let call_id = runtime.ctx.next_debug_call_id();
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

            runtime.debug_stack.push(DebugFrame {
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

            let result = eval_runtime_rust_ir_expr(runtime, body, env);

            let frame = runtime.debug_stack.pop();
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
        rust_ir::RustIrExpr::Pipe {
            pipe_id,
            step,
            label,
            log_time,
            func,
            arg,
            ..
        } => {
            let func_value = eval_runtime_rust_ir_expr(runtime, func, env)?;
            let arg_value = eval_runtime_rust_ir_expr(runtime, arg, env)?;

            let Some(frame) = runtime.debug_stack.last().cloned() else {
                return runtime.apply(func_value, arg_value);
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
            let out_value = runtime.apply(func_value, arg_value)?;

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
        rust_ir::RustIrExpr::List { items, .. } => {
            let mut values = Vec::new();
            for item in items {
                let value = eval_runtime_rust_ir_expr(runtime, &item.expr, env)?;
                if item.spread {
                    let Value::List(inner) = value else {
                        return Err(RuntimeError::Message(
                            "list spread expects a list".to_string(),
                        ));
                    };
                    values.extend(inner.iter().cloned());
                } else {
                    values.push(value);
                }
            }
            Ok(Value::List(Arc::new(values)))
        }
        rust_ir::RustIrExpr::Tuple { items, .. } => {
            let mut values = Vec::with_capacity(items.len());
            for item in items {
                values.push(eval_runtime_rust_ir_expr(runtime, item, env)?);
            }
            Ok(Value::Tuple(values))
        }
        rust_ir::RustIrExpr::Record { fields, .. } => {
            eval_runtime_rust_ir_record(runtime, fields, env)
        }
        rust_ir::RustIrExpr::Patch { target, fields, .. } => {
            eval_runtime_rust_ir_patch(runtime, target, fields, env)
        }
        rust_ir::RustIrExpr::FieldAccess { base, field, .. } => {
            let base_value = eval_runtime_rust_ir_expr(runtime, base, env)?;
            match base_value {
                Value::Record(map) => shape_record(map.as_ref())
                    .get(field)
                    .cloned()
                    .ok_or_else(|| RuntimeError::Message(format!("missing field {field}"))),
                _ => Err(RuntimeError::Message(format!(
                    "field access on non-record {field}"
                ))),
            }
        }
        rust_ir::RustIrExpr::Index { base, index, .. } => {
            let base_value = eval_runtime_rust_ir_expr(runtime, base, env)?;
            let index_value = eval_runtime_rust_ir_expr(runtime, index, env)?;
            read_indexed_value(base_value, index_value)
        }
        rust_ir::RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            let value = eval_runtime_rust_ir_expr(runtime, scrutinee, env)?;
            for arm in arms {
                let pattern = lower_runtime_rust_ir_pattern(&arm.pattern);
                let Some(bindings) = collect_pattern_bindings(&pattern, &value) else {
                    continue;
                };
                if let Some(guard) = &arm.guard {
                    let guard_env = Env::new(Some(env.clone()));
                    for (name, value) in bindings.clone() {
                        guard_env.set(name, value);
                    }
                    let guard_value = eval_runtime_rust_ir_expr(runtime, guard, &guard_env)?;
                    if !matches!(guard_value, Value::Bool(true)) {
                        continue;
                    }
                }
                let arm_env = Env::new(Some(env.clone()));
                for (name, value) in bindings {
                    arm_env.set(name, value);
                }
                return eval_runtime_rust_ir_expr(runtime, &arm.body, &arm_env);
            }
            Err(RuntimeError::Message("non-exhaustive match".to_string()))
        }
        rust_ir::RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            let cond_value = eval_runtime_rust_ir_expr(runtime, cond, env)?;
            if matches!(cond_value, Value::Bool(true)) {
                eval_runtime_rust_ir_expr(runtime, then_branch, env)
            } else {
                eval_runtime_rust_ir_expr(runtime, else_branch, env)
            }
        }
        rust_ir::RustIrExpr::Binary {
            op, left, right, ..
        } => {
            if op == "&&" {
                let left_value = eval_runtime_rust_ir_expr(runtime, left, env)?;
                return match left_value {
                    Value::Bool(false) => Ok(Value::Bool(false)),
                    Value::Bool(true) => eval_runtime_rust_ir_expr(runtime, right, env),
                    _ => {
                        let right_value = eval_runtime_rust_ir_expr(runtime, right, env)?;
                        runtime.eval_binary(op, left_value, right_value, env)
                    }
                };
            }
            if op == "||" {
                let left_value = eval_runtime_rust_ir_expr(runtime, left, env)?;
                return match left_value {
                    Value::Bool(true) => Ok(Value::Bool(true)),
                    Value::Bool(false) => eval_runtime_rust_ir_expr(runtime, right, env),
                    _ => {
                        let right_value = eval_runtime_rust_ir_expr(runtime, right, env)?;
                        runtime.eval_binary(op, left_value, right_value, env)
                    }
                };
            }
            let left_value = eval_runtime_rust_ir_expr(runtime, left, env)?;
            let right_value = eval_runtime_rust_ir_expr(runtime, right, env)?;
            runtime.eval_binary(op, left_value, right_value, env)
        }
        rust_ir::RustIrExpr::Block {
            block_kind, items, ..
        } => match block_kind {
            rust_ir::RustIrBlockKind::Plain => eval_runtime_rust_ir_plain_block(runtime, items, env),
            rust_ir::RustIrBlockKind::Do { monad } if monad == "Effect" => {
                let lowered = lower_runtime_rust_ir_block_items(items)?;
                Ok(Value::Effect(Arc::new(EffectValue::Block {
                    env: env.clone(),
                    items: Arc::new(lowered),
                })))
            }
            rust_ir::RustIrBlockKind::Do { monad } => {
                let lowered = lower_runtime_rust_ir_block_items(items)?;
                runtime.eval_generic_do_block(monad, &lowered, env)
            }
            rust_ir::RustIrBlockKind::Generate => {
                let lowered = lower_runtime_rust_ir_block_items(items)?;
                runtime.eval_generate_block(&lowered, env)
            }
            rust_ir::RustIrBlockKind::Resource => {
                let lowered = lower_runtime_rust_ir_block_items(items)?;
                Ok(Value::Resource(Arc::new(ResourceValue {
                    items: Arc::new(lowered),
                })))
            }
        },
        rust_ir::RustIrExpr::Raw { text, .. } => Ok(Value::Text(text.clone())),
    }
}

fn eval_runtime_sigil_literal(tag: &str, body: &str, flags: &str) -> Result<Value, RuntimeError> {
    match tag {
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
        "u" | "url" => {
            let parsed = Url::parse(body).map_err(|err| {
                RuntimeError::Message(format!("invalid url literal: {err}"))
            })?;
            Ok(Value::Record(Arc::new(url_to_record(&parsed))))
        }
        "p" | "path" => {
            let cleaned = body.trim().replace('\\', "/");
            if cleaned.contains('\0') {
                return Err(RuntimeError::Message(
                    "invalid path literal: contains NUL byte".to_string(),
                ));
            }
            let absolute = cleaned.starts_with('/');
            let mut segments: Vec<String> = Vec::new();
            for raw in cleaned.split('/') {
                if raw.is_empty() || raw == "." {
                    continue;
                }
                if raw == ".." {
                    if let Some(last) = segments.last() {
                        if last != ".." {
                            segments.pop();
                            continue;
                        }
                    }
                    if !absolute {
                        segments.push("..".to_string());
                    }
                    continue;
                }
                segments.push(raw.to_string());
            }

            let mut map = HashMap::new();
            map.insert("absolute".to_string(), Value::Bool(absolute));
            map.insert(
                "segments".to_string(),
                Value::List(Arc::new(
                    segments.into_iter().map(Value::Text).collect::<Vec<_>>(),
                )),
            );
            Ok(Value::Record(Arc::new(map)))
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
            Ok(Value::DateTime(body.to_string()))
        }
        "tz" => {
            let zone_id = body.trim();
            let _: chrono_tz::Tz = zone_id.parse().map_err(|_| {
                RuntimeError::Message(format!("invalid timezone id: {zone_id}"))
            })?;
            let mut map = HashMap::new();
            map.insert("id".to_string(), Value::Text(zone_id.to_string()));
            Ok(Value::Record(Arc::new(map)))
        }
        "zdt" => {
            let text = body.trim();
            let (dt_text, zone_id) = parse_zdt_parts(text)?;
            let tz: chrono_tz::Tz = zone_id.parse().map_err(|_| {
                RuntimeError::Message(format!("invalid timezone id: {zone_id}"))
            })?;

            let zdt = if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(dt_text) {
                parsed.with_timezone(&tz)
            } else {
                let naive = parse_naive_datetime(dt_text)?;
                tz.from_local_datetime(&naive)
                    .single()
                    .ok_or_else(|| {
                        RuntimeError::Message("ambiguous or invalid local time".to_string())
                    })?
            };

            let offset_millis = i64::from(chrono::offset::Offset::fix(zdt.offset()).local_minus_utc()) * 1000;

            let mut dt_map = HashMap::new();
            dt_map.insert("year".to_string(), Value::Int(zdt.year() as i64));
            dt_map.insert("month".to_string(), Value::Int(zdt.month() as i64));
            dt_map.insert("day".to_string(), Value::Int(zdt.day() as i64));
            dt_map.insert("hour".to_string(), Value::Int(zdt.hour() as i64));
            dt_map.insert("minute".to_string(), Value::Int(zdt.minute() as i64));
            dt_map.insert("second".to_string(), Value::Int(zdt.second() as i64));
            dt_map.insert(
                "millisecond".to_string(),
                Value::Int(zdt.timestamp_subsec_millis() as i64),
            );

            let mut zone_map = HashMap::new();
            zone_map.insert("id".to_string(), Value::Text(zone_id.to_string()));

            let mut offset_map = HashMap::new();
            offset_map.insert("millis".to_string(), Value::Int(offset_millis));

            let mut map = HashMap::new();
            map.insert("dateTime".to_string(), Value::Record(Arc::new(dt_map)));
            map.insert("zone".to_string(), Value::Record(Arc::new(zone_map)));
            map.insert("offset".to_string(), Value::Record(Arc::new(offset_map)));
            Ok(Value::Record(Arc::new(map)))
        }
        "k" => {
            validate_key_text(body).map_err(|msg| {
                RuntimeError::Message(format!("invalid i18n key literal: {msg}"))
            })?;
            let mut map = HashMap::new();
            map.insert("tag".to_string(), Value::Text(tag.to_string()));
            map.insert("body".to_string(), Value::Text(body.trim().to_string()));
            map.insert("flags".to_string(), Value::Text(flags.to_string()));
            Ok(Value::Record(Arc::new(map)))
        }
        "m" => {
            let parsed = parse_message_template(body).map_err(|msg| {
                RuntimeError::Message(format!("invalid i18n message literal: {msg}"))
            })?;
            let mut map = HashMap::new();
            map.insert("tag".to_string(), Value::Text(tag.to_string()));
            map.insert("body".to_string(), Value::Text(body.to_string()));
            map.insert("flags".to_string(), Value::Text(flags.to_string()));
            map.insert("parts".to_string(), i18n_message_parts_value(&parsed.parts));
            Ok(Value::Record(Arc::new(map)))
        }
        _ => {
            let mut map = HashMap::new();
            map.insert("tag".to_string(), Value::Text(tag.to_string()));
            map.insert("body".to_string(), Value::Text(body.to_string()));
            map.insert("flags".to_string(), Value::Text(flags.to_string()));
            Ok(Value::Record(Arc::new(map)))
        }
    }
}

fn eval_runtime_rust_ir_plain_block(
    runtime: &mut Runtime,
    items: &[rust_ir::RustIrBlockItem],
    env: &Env,
) -> Result<Value, RuntimeError> {
    if items.is_empty() {
        return Ok(Value::Unit);
    }
    let local_env = Env::new(Some(env.clone()));
    let mut result = Value::Unit;
    for item in items {
        match item {
            rust_ir::RustIrBlockItem::Bind { pattern, expr } => {
                let value = eval_runtime_rust_ir_expr(runtime, expr, &local_env)?;
                let pattern = lower_runtime_rust_ir_pattern(pattern);
                let bindings = collect_pattern_bindings(&pattern, &value)
                    .ok_or_else(|| RuntimeError::Message("pattern match failed".to_string()))?;
                for (name, value) in bindings {
                    local_env.set(name, value);
                }
                result = Value::Unit;
            }
            rust_ir::RustIrBlockItem::Expr { expr } => {
                result = eval_runtime_rust_ir_expr(runtime, expr, &local_env)?;
            }
            rust_ir::RustIrBlockItem::Filter { .. } => {
                return Err(RuntimeError::Message(
                    "unsupported block item in plain block: Filter".to_string(),
                ));
            }
            rust_ir::RustIrBlockItem::Yield { .. } => {
                return Err(RuntimeError::Message(
                    "unsupported block item in plain block: Yield".to_string(),
                ));
            }
            rust_ir::RustIrBlockItem::Recurse { .. } => {
                return Err(RuntimeError::Message(
                    "unsupported block item in plain block: Recurse".to_string(),
                ));
            }
        }
    }
    Ok(result)
}

pub(crate) fn lower_runtime_rust_ir_block_items(
    items: &[rust_ir::RustIrBlockItem],
) -> Result<Vec<HirBlockItem>, RuntimeError> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            lower_runtime_rust_ir_block_item(item).ok_or_else(|| {
                RuntimeError::Message(format!(
                    "failed to lower jitted block item at index {index}"
                ))
            })
        })
        .collect()
}

fn eval_runtime_rust_ir_record(
    runtime: &mut Runtime,
    fields: &[rust_ir::RustIrRecordField],
    env: &Env,
) -> Result<Value, RuntimeError> {
    let mut map = HashMap::new();
    for field in fields {
        let value = eval_runtime_rust_ir_expr(runtime, &field.value, env)?;
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
        if field
            .path
            .iter()
            .all(|seg| matches!(seg, rust_ir::RustIrPathSegment::Field(_)))
        {
            insert_runtime_rust_ir_record_path(&mut map, &field.path, value)?;
            continue;
        }

        let resolved_path = resolve_runtime_rust_ir_path_segments(runtime, &field.path, env)?;
        let current = std::mem::take(&mut map);
        let updated = apply_value_path_update(
            runtime,
            Value::Record(Arc::new(current)),
            &resolved_path,
            value,
            PathUpdateMode::Assign,
        )?;
        let Value::Record(updated) = updated else {
            return Err(RuntimeError::Message(format!(
                "record update expected Record, got {}",
                format_value(&updated)
            )));
        };
        map = updated.as_ref().clone();
    }
    Ok(Value::Record(Arc::new(map)))
}

fn insert_runtime_rust_ir_record_path(
    record: &mut HashMap<String, Value>,
    path: &[rust_ir::RustIrPathSegment],
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
            rust_ir::RustIrPathSegment::Field(name) => {
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
            rust_ir::RustIrPathSegment::IndexValue(_)
            | rust_ir::RustIrPathSegment::IndexFieldBool(_)
            | rust_ir::RustIrPathSegment::IndexPredicate(_)
            | rust_ir::RustIrPathSegment::IndexAll => {
                return Err(RuntimeError::Message(
                    "record index path reached field-only insert path".to_string(),
                ))
            }
        }
    }
    Ok(())
}

fn eval_runtime_rust_ir_patch(
    runtime: &mut Runtime,
    target: &rust_ir::RustIrExpr,
    fields: &[rust_ir::RustIrRecordField],
    env: &Env,
) -> Result<Value, RuntimeError> {
    let base_value = eval_runtime_rust_ir_expr(runtime, target, env)?;
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
        if field
            .path
            .iter()
            .all(|seg| matches!(seg, rust_ir::RustIrPathSegment::Field(_)))
        {
            apply_runtime_rust_ir_patch_field(runtime, &mut map, &field.path, &field.value, env)?;
            continue;
        }

        let resolved_path = resolve_runtime_rust_ir_path_segments(runtime, &field.path, env)?;
        let updater = eval_runtime_rust_ir_expr(runtime, &field.value, env)?;
        let current = std::mem::take(&mut map);
        let updated = apply_value_path_update(
            runtime,
            Value::Record(Arc::new(current)),
            &resolved_path,
            updater,
            PathUpdateMode::Patch,
        )?;
        let Value::Record(updated) = updated else {
            return Err(RuntimeError::Message(format!(
                "patch update expected Record, got {}",
                format_value(&updated)
            )));
        };
        map = updated.as_ref().clone();
    }
    Ok(Value::Record(Arc::new(map)))
}

fn apply_runtime_rust_ir_patch_field(
    runtime: &mut Runtime,
    record: &mut HashMap<String, Value>,
    path: &[rust_ir::RustIrPathSegment],
    expr: &rust_ir::RustIrExpr,
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
            rust_ir::RustIrPathSegment::Field(name) => {
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
            rust_ir::RustIrPathSegment::IndexValue(_)
            | rust_ir::RustIrPathSegment::IndexFieldBool(_)
            | rust_ir::RustIrPathSegment::IndexPredicate(_)
            | rust_ir::RustIrPathSegment::IndexAll => {
                return Err(RuntimeError::Message(
                    "indexed patch segment reached field-only patch path".to_string(),
                ))
            }
        }
    }
    let segment = path.last().unwrap();
    match segment {
        rust_ir::RustIrPathSegment::Field(name) => {
            let existing = current.get(name).cloned();
            let value = eval_runtime_rust_ir_expr(runtime, expr, env)?;
            let new_value = match existing {
                Some(existing) if is_callable(&value) => runtime.apply(value, existing)?,
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
        rust_ir::RustIrPathSegment::IndexValue(_)
        | rust_ir::RustIrPathSegment::IndexFieldBool(_)
        | rust_ir::RustIrPathSegment::IndexPredicate(_)
        | rust_ir::RustIrPathSegment::IndexAll => Err(RuntimeError::Message(
            "indexed patch segment reached field-only patch leaf".to_string(),
        )),
    }
}

fn resolve_runtime_rust_ir_path_segments(
    runtime: &mut Runtime,
    path: &[rust_ir::RustIrPathSegment],
    env: &Env,
) -> Result<Vec<RuntimePathSegment>, RuntimeError> {
    let mut resolved = Vec::with_capacity(path.len());
    for segment in path {
        match segment {
            rust_ir::RustIrPathSegment::Field(name) => {
                resolved.push(RuntimePathSegment::Field(name.clone()));
            }
            rust_ir::RustIrPathSegment::IndexValue(expr) => {
                let value = eval_runtime_rust_ir_expr(runtime, expr, env)?;
                if is_callable(&value) {
                    resolved.push(RuntimePathSegment::IndexPredicate(value));
                } else {
                    resolved.push(RuntimePathSegment::IndexValue(value));
                }
            }
            rust_ir::RustIrPathSegment::IndexFieldBool(name) => {
                resolved.push(RuntimePathSegment::IndexFieldBool(name.clone()));
            }
            rust_ir::RustIrPathSegment::IndexPredicate(expr) => {
                let predicate = eval_runtime_rust_ir_expr(runtime, expr, env)?;
                resolved.push(RuntimePathSegment::IndexPredicate(predicate));
            }
            rust_ir::RustIrPathSegment::IndexAll => {
                resolved.push(RuntimePathSegment::IndexAll);
            }
        }
    }
    Ok(resolved)
}

fn lower_runtime_rust_ir_expr(expr: &rust_ir::RustIrExpr) -> Option<HirExpr> {
    Some(match expr {
        rust_ir::RustIrExpr::Local { id, name }
        | rust_ir::RustIrExpr::Global { id, name } => HirExpr::Var {
            id: *id,
            name: name.clone(),
        },
        rust_ir::RustIrExpr::Builtin { id, builtin } => HirExpr::Var {
            id: *id,
            name: builtin.clone(),
        },
        rust_ir::RustIrExpr::ConstructorValue { id, name } => HirExpr::Var {
            id: *id,
            name: name.clone(),
        },
        rust_ir::RustIrExpr::LitNumber { id, text } => HirExpr::LitNumber {
            id: *id,
            text: text.clone(),
        },
        rust_ir::RustIrExpr::LitString { id, text } => HirExpr::LitString {
            id: *id,
            text: text.clone(),
        },
        rust_ir::RustIrExpr::TextInterpolate { id, parts } => HirExpr::TextInterpolate {
            id: *id,
            parts: parts
                .iter()
                .map(lower_runtime_rust_ir_text_part)
                .collect::<Option<Vec<_>>>()?,
        },
        rust_ir::RustIrExpr::LitSigil {
            id,
            tag,
            body,
            flags,
        } => HirExpr::LitSigil {
            id: *id,
            tag: tag.clone(),
            body: body.clone(),
            flags: flags.clone(),
        },
        rust_ir::RustIrExpr::LitBool { id, value } => HirExpr::LitBool {
            id: *id,
            value: *value,
        },
        rust_ir::RustIrExpr::LitDateTime { id, text } => HirExpr::LitDateTime {
            id: *id,
            text: text.clone(),
        },
        rust_ir::RustIrExpr::Lambda { id, param, body } => HirExpr::Lambda {
            id: *id,
            param: param.clone(),
            body: Box::new(lower_runtime_rust_ir_expr(body)?),
        },
        rust_ir::RustIrExpr::App { id, func, arg } => HirExpr::App {
            id: *id,
            func: Box::new(lower_runtime_rust_ir_expr(func)?),
            arg: Box::new(lower_runtime_rust_ir_expr(arg)?),
        },
        rust_ir::RustIrExpr::Call { id, func, args } => HirExpr::Call {
            id: *id,
            func: Box::new(lower_runtime_rust_ir_expr(func)?),
            args: args
                .iter()
                .map(lower_runtime_rust_ir_expr)
                .collect::<Option<Vec<_>>>()?,
        },
        rust_ir::RustIrExpr::DebugFn {
            id,
            fn_name,
            arg_vars,
            log_args,
            log_return,
            log_time,
            body,
        } => HirExpr::DebugFn {
            id: *id,
            fn_name: fn_name.clone(),
            arg_vars: arg_vars.clone(),
            log_args: *log_args,
            log_return: *log_return,
            log_time: *log_time,
            body: Box::new(lower_runtime_rust_ir_expr(body)?),
        },
        rust_ir::RustIrExpr::Pipe {
            id,
            pipe_id,
            step,
            label,
            log_time,
            func,
            arg,
        } => HirExpr::Pipe {
            id: *id,
            pipe_id: *pipe_id,
            step: *step,
            label: label.clone(),
            log_time: *log_time,
            func: Box::new(lower_runtime_rust_ir_expr(func)?),
            arg: Box::new(lower_runtime_rust_ir_expr(arg)?),
        },
        rust_ir::RustIrExpr::List { id, items } => HirExpr::List {
            id: *id,
            items: items
                .iter()
                .map(|item| {
                    Some(HirListItem {
                        expr: lower_runtime_rust_ir_expr(&item.expr)?,
                        spread: item.spread,
                    })
                })
                .collect::<Option<Vec<_>>>()?,
        },
        rust_ir::RustIrExpr::Tuple { id, items } => HirExpr::Tuple {
            id: *id,
            items: items
                .iter()
                .map(lower_runtime_rust_ir_expr)
                .collect::<Option<Vec<_>>>()?,
        },
        rust_ir::RustIrExpr::Record { id, fields } => HirExpr::Record {
            id: *id,
            fields: fields
                .iter()
                .map(lower_runtime_rust_ir_record_field)
                .collect::<Option<Vec<_>>>()?,
        },
        rust_ir::RustIrExpr::Patch { id, target, fields } => HirExpr::Patch {
            id: *id,
            target: Box::new(lower_runtime_rust_ir_expr(target)?),
            fields: fields
                .iter()
                .map(lower_runtime_rust_ir_record_field)
                .collect::<Option<Vec<_>>>()?,
        },
        rust_ir::RustIrExpr::FieldAccess { id, base, field } => HirExpr::FieldAccess {
            id: *id,
            base: Box::new(lower_runtime_rust_ir_expr(base)?),
            field: field.clone(),
        },
        rust_ir::RustIrExpr::Index { id, base, index } => HirExpr::Index {
            id: *id,
            base: Box::new(lower_runtime_rust_ir_expr(base)?),
            index: Box::new(lower_runtime_rust_ir_expr(index)?),
        },
        rust_ir::RustIrExpr::Match {
            id,
            scrutinee,
            arms,
        } => HirExpr::Match {
            id: *id,
            scrutinee: Box::new(lower_runtime_rust_ir_expr(scrutinee)?),
            arms: arms
                .iter()
                .map(lower_runtime_rust_ir_match_arm)
                .collect::<Option<Vec<_>>>()?,
        },
        rust_ir::RustIrExpr::If {
            id,
            cond,
            then_branch,
            else_branch,
        } => HirExpr::If {
            id: *id,
            cond: Box::new(lower_runtime_rust_ir_expr(cond)?),
            then_branch: Box::new(lower_runtime_rust_ir_expr(then_branch)?),
            else_branch: Box::new(lower_runtime_rust_ir_expr(else_branch)?),
        },
        rust_ir::RustIrExpr::Binary {
            id,
            op,
            left,
            right,
        } => HirExpr::Binary {
            id: *id,
            op: op.clone(),
            left: Box::new(lower_runtime_rust_ir_expr(left)?),
            right: Box::new(lower_runtime_rust_ir_expr(right)?),
        },
        rust_ir::RustIrExpr::Block {
            id,
            block_kind,
            items,
        } => HirExpr::Block {
            id: *id,
            block_kind: lower_runtime_rust_ir_block_kind(block_kind)?,
            items: items
                .iter()
                .map(lower_runtime_rust_ir_block_item)
                .collect::<Option<Vec<_>>>()?,
        },
        rust_ir::RustIrExpr::Raw { id, text } => HirExpr::Raw {
            id: *id,
            text: text.clone(),
        },
    })
}

fn lower_runtime_rust_ir_text_part(part: &rust_ir::RustIrTextPart) -> Option<HirTextPart> {
    Some(match part {
        rust_ir::RustIrTextPart::Text { text } => HirTextPart::Text { text: text.clone() },
        rust_ir::RustIrTextPart::Expr { expr } => HirTextPart::Expr {
            expr: lower_runtime_rust_ir_expr(expr)?,
        },
    })
}

fn lower_runtime_rust_ir_record_field(field: &rust_ir::RustIrRecordField) -> Option<HirRecordField> {
    Some(HirRecordField {
        spread: field.spread,
        path: field
            .path
            .iter()
            .map(lower_runtime_rust_ir_path_segment)
            .collect::<Option<Vec<_>>>()?,
        value: lower_runtime_rust_ir_expr(&field.value)?,
    })
}

fn lower_runtime_rust_ir_path_segment(seg: &rust_ir::RustIrPathSegment) -> Option<HirPathSegment> {
    Some(match seg {
        rust_ir::RustIrPathSegment::Field(name) => HirPathSegment::Field(name.clone()),
        rust_ir::RustIrPathSegment::IndexValue(expr)
        | rust_ir::RustIrPathSegment::IndexPredicate(expr) => {
            HirPathSegment::Index(lower_runtime_rust_ir_expr(expr)?)
        }
        rust_ir::RustIrPathSegment::IndexFieldBool(name) => HirPathSegment::Index(HirExpr::Var {
            id: 0,
            name: name.clone(),
        }),
        rust_ir::RustIrPathSegment::IndexAll => HirPathSegment::All,
    })
}

fn lower_runtime_rust_ir_match_arm(arm: &rust_ir::RustIrMatchArm) -> Option<HirMatchArm> {
    Some(HirMatchArm {
        pattern: lower_runtime_rust_ir_pattern(&arm.pattern),
        guard: match arm.guard.as_ref() {
            Some(guard) => Some(lower_runtime_rust_ir_expr(guard)?),
            None => None,
        },
        body: lower_runtime_rust_ir_expr(&arm.body)?,
    })
}

fn lower_runtime_rust_ir_pattern(pattern: &rust_ir::RustIrPattern) -> HirPattern {
    match pattern {
        rust_ir::RustIrPattern::Wildcard { id } => HirPattern::Wildcard { id: *id },
        rust_ir::RustIrPattern::Var { id, name } => HirPattern::Var {
            id: *id,
            name: name.clone(),
        },
        rust_ir::RustIrPattern::At { id, name, pattern } => HirPattern::At {
            id: *id,
            name: name.clone(),
            pattern: Box::new(lower_runtime_rust_ir_pattern(pattern)),
        },
        rust_ir::RustIrPattern::Literal { id, value } => HirPattern::Literal {
            id: *id,
            value: lower_runtime_rust_ir_literal(value),
        },
        rust_ir::RustIrPattern::Constructor { id, name, args } => HirPattern::Constructor {
            id: *id,
            name: name.clone(),
            args: args
                .iter()
                .map(lower_runtime_rust_ir_pattern)
                .collect(),
        },
        rust_ir::RustIrPattern::Tuple { id, items } => HirPattern::Tuple {
            id: *id,
            items: items
                .iter()
                .map(lower_runtime_rust_ir_pattern)
                .collect(),
        },
        rust_ir::RustIrPattern::List { id, items, rest } => HirPattern::List {
            id: *id,
            items: items
                .iter()
                .map(lower_runtime_rust_ir_pattern)
                .collect(),
            rest: match rest.as_ref() {
                Some(rest) => Some(Box::new(lower_runtime_rust_ir_pattern(rest.as_ref()))),
                None => None,
            },
        },
        rust_ir::RustIrPattern::Record { id, fields } => HirPattern::Record {
            id: *id,
            fields: fields
                .iter()
                .map(|field| crate::hir::HirRecordPatternField {
                    path: field.path.clone(),
                    pattern: lower_runtime_rust_ir_pattern(&field.pattern),
                })
                .collect(),
        },
    }
}

fn lower_runtime_rust_ir_literal(literal: &rust_ir::RustIrLiteral) -> HirLiteral {
    match literal {
        rust_ir::RustIrLiteral::Number(value) => HirLiteral::Number(value.clone()),
        rust_ir::RustIrLiteral::String(value) => HirLiteral::String(value.clone()),
        rust_ir::RustIrLiteral::Sigil { tag, body, flags } => HirLiteral::Sigil {
            tag: tag.clone(),
            body: body.clone(),
            flags: flags.clone(),
        },
        rust_ir::RustIrLiteral::Bool(value) => HirLiteral::Bool(*value),
        rust_ir::RustIrLiteral::DateTime(value) => HirLiteral::DateTime(value.clone()),
    }
}

fn lower_runtime_rust_ir_block_kind(kind: &rust_ir::RustIrBlockKind) -> Option<crate::hir::HirBlockKind> {
    Some(match kind {
        rust_ir::RustIrBlockKind::Plain => crate::hir::HirBlockKind::Plain,
        rust_ir::RustIrBlockKind::Do { monad } => crate::hir::HirBlockKind::Do {
            monad: monad.clone(),
        },
        rust_ir::RustIrBlockKind::Generate => crate::hir::HirBlockKind::Generate,
        rust_ir::RustIrBlockKind::Resource => crate::hir::HirBlockKind::Resource,
    })
}

fn lower_runtime_rust_ir_block_item(item: &rust_ir::RustIrBlockItem) -> Option<HirBlockItem> {
    Some(match item {
        rust_ir::RustIrBlockItem::Bind { pattern, expr } => HirBlockItem::Bind {
            pattern: lower_runtime_rust_ir_pattern(pattern),
            expr: lower_runtime_rust_ir_expr(expr)?,
        },
        rust_ir::RustIrBlockItem::Filter { expr } => HirBlockItem::Filter {
            expr: lower_runtime_rust_ir_expr(expr)?,
        },
        rust_ir::RustIrBlockItem::Yield { expr } => HirBlockItem::Yield {
            expr: lower_runtime_rust_ir_expr(expr)?,
        },
        rust_ir::RustIrBlockItem::Recurse { expr } => HirBlockItem::Recurse {
            expr: lower_runtime_rust_ir_expr(expr)?,
        },
        rust_ir::RustIrBlockItem::Expr { expr } => HirBlockItem::Expr {
            expr: lower_runtime_rust_ir_expr(expr)?,
        },
    })
}


pub fn run_test_suite(
    program: HirProgram,
    test_entries: &[(String, String)],
    surface_modules: &[crate::surface::Module],
) -> Result<TestReport, AiviError> {
    const TEST_FUEL_BUDGET: u64 = 500_000;
    let mut runtime = build_runtime_from_program_scoped(program, surface_modules)?;
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

pub(crate) fn build_runtime_from_program(program: HirProgram) -> Result<Runtime, AiviError> {
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
    globals.set("__machine_on".to_string(), make_machine_on_builtin());
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

    let ctx = Arc::new(RuntimeContext::new_with_constructor_ordinals(
        globals,
        core_constructor_ordinals(),
    ));
    let cancel = CancelToken::root();
    Ok(Runtime::new(ctx, cancel))
}

fn build_runtime_from_program_scoped(
    program: HirProgram,
    surface_modules: &[crate::surface::Module],
) -> Result<Runtime, AiviError> {
    if program.modules.is_empty() {
        return Err(AiviError::Runtime("no modules to run".to_string()));
    }

    let globals = Env::new(None);
    register_builtins(&globals);
    globals.set("__machine_on".to_string(), make_machine_on_builtin());

    // Build a map of surface module metadata for import scoping.
    let mut surface_by_name: HashMap<String, &crate::surface::Module> = HashMap::new();
    for module in surface_modules {
        surface_by_name.insert(module.name.name.clone(), module);
    }
    let mut value_exports: HashMap<String, Vec<String>> = HashMap::new();
    let mut domain_members: HashMap<(String, String), Vec<String>> = HashMap::new();
    let mut method_names: HashSet<String> = HashSet::new();
    for module in surface_modules {
        value_exports.insert(
            module.name.name.clone(),
            module
                .exports
                .iter()
                .filter(|e| e.kind == crate::surface::ScopeItemKind::Value)
                .map(|e| e.name.name.clone())
                .collect(),
        );
        for export in &module.exports {
            if export.kind != crate::surface::ScopeItemKind::Domain {
                continue;
            }
            let domain_name = export.name.name.clone();
            let mut members = Vec::new();
            for item in &module.items {
                let crate::surface::ModuleItem::DomainDecl(domain) = item else {
                    continue;
                };
                if domain.name.name != domain_name {
                    continue;
                }
                for domain_item in &domain.items {
                    match domain_item {
                        crate::surface::DomainItem::Def(def)
                        | crate::surface::DomainItem::LiteralDef(def) => {
                            members.push(def.name.name.clone());
                        }
                        crate::surface::DomainItem::TypeAlias(_)
                        | crate::surface::DomainItem::TypeSig(_) => {}
                    }
                }
            }
            domain_members.insert((module.name.name.clone(), domain_name), members);
        }

        // Methods (class members) behave like open multi-clause functions at runtime: instances can
        // add new clauses. When importing, we merge method bindings instead of overwriting locals.
        for item in &module.items {
            let crate::surface::ModuleItem::ClassDecl(class_decl) = item else {
                continue;
            };
            for member in &class_decl.members {
                method_names.insert(member.name.name.clone());
            }
        }
    }

    fn merge_method_binding(existing: Value, imported: Value) -> Value {
        fn flatten(value: Value, out: &mut Vec<Value>) {
            match value {
                Value::MultiClause(clauses) => out.extend(clauses),
                other => out.push(other),
            }
        }

        let mut clauses = Vec::new();
        // Keep local clauses first so user-defined instances override defaults.
        flatten(existing, &mut clauses);
        flatten(imported, &mut clauses);
        Value::MultiClause(clauses)
    }

    // Create a per-module environment rooted at the global environment. Each top-level def thunk
    // captures its module env so runtime evaluation respects lexical imports and avoids global
    // collisions (especially for operator names like `(+)`).
    let mut module_envs: HashMap<String, Env> = HashMap::new();
    for module in &program.modules {
        module_envs.insert(module.name.clone(), Env::new(Some(globals.clone())));
    }

    // First pass: register qualified globals for every definition, preserving multi-clause
    // functions (same qualified name defined multiple times).
    let mut grouped: HashMap<String, (Env, Vec<HirExpr>)> = HashMap::new();
    for module in &program.modules {
        let module_name = module.name.clone();
        let module_env = module_envs
            .get(&module_name)
            .cloned()
            .unwrap_or_else(|| Env::new(Some(globals.clone())));
        for def in &module.defs {
            let name = format!("{module_name}.{}", def.name);
            grouped
                .entry(name)
                .or_insert_with(|| (module_env.clone(), Vec::new()))
                .1
                .push(def.expr.clone());
        }
    }
    for (name, (module_env, exprs)) in grouped {
        if globals.get(&name).is_some() {
            continue;
        }
        if exprs.len() == 1 {
            let thunk = ThunkValue {
                expr: Arc::new(exprs.into_iter().next().unwrap()),
                env: module_env,
                cached: Mutex::new(None),
                in_progress: AtomicBool::new(false),
            };
            globals.set(name, Value::Thunk(Arc::new(thunk)));
        } else {
            let mut clauses = Vec::new();
            for expr in exprs {
                let thunk = ThunkValue {
                    expr: Arc::new(expr),
                    env: module_env.clone(),
                    cached: Mutex::new(None),
                    in_progress: AtomicBool::new(false),
                };
                clauses.push(Value::Thunk(Arc::new(thunk)));
            }
            globals.set(name, Value::MultiClause(clauses));
        }
    }

    let mut machine_specs: Vec<(String, String, HashMap<String, Vec<MachineEdge>>)> = Vec::new();

    // Second pass: populate each module env with its local defs and imports.
    for module in &program.modules {
        let module_name = module.name.clone();
        let module_env = module_envs
            .get(&module_name)
            .cloned()
            .unwrap_or_else(|| Env::new(Some(globals.clone())));

        // Local defs in the module are always in scope unqualified.
        for def in &module.defs {
            let qualified = format!("{module_name}.{}", def.name);
            if let Some(value) = globals.get(&qualified) {
                module_env.set(def.name.clone(), value);
            }
        }

        // Import exported values and domain members.
        let Some(surface_module) = surface_by_name.get(&module_name).copied() else {
            continue;
        };
        for use_decl in &surface_module.uses {
            let imported_mod = use_decl.module.name.clone();
            if use_decl.wildcard {
                if let Some(names) = value_exports.get(&imported_mod) {
                    for name in names {
                        let qualified = format!("{imported_mod}.{name}");
                        if let Some(value) = globals.get(&qualified) {
                            if let Some(existing) = module_env.get(name) {
                                if method_names.contains(name) {
                                    module_env.set(
                                        name.clone(),
                                        merge_method_binding(existing, value),
                                    );
                                    continue;
                                }
                            }
                            // Non-methods: last import wins (allows more-specific modules to shadow)
                            module_env.set(name.clone(), value);
                        }
                    }
                }
                continue;
            }
            for item in &use_decl.items {
                match item.kind {
                    crate::surface::ScopeItemKind::Value => {
                        let name = item.name.name.clone();
                        let qualified = format!("{imported_mod}.{name}");
                        if let Some(value) = globals.get(&qualified) {
                            if let Some(existing) = module_env.get(&name) {
                                if method_names.contains(&name) {
                                    module_env.set(
                                        name.clone(),
                                        merge_method_binding(existing, value),
                                    );
                                    continue;
                                }
                            }
                            module_env.set(name, value);
                        }
                    }
                    crate::surface::ScopeItemKind::Domain => {
                        let domain_name = item.name.name.clone();
                        let key = (imported_mod.clone(), domain_name);
                        if let Some(members) = domain_members.get(&key) {
                            for member in members {
                                let qualified = format!("{imported_mod}.{member}");
                                if let Some(value) = globals.get(&qualified) {
                                    if let Some(existing) = module_env.get(member) {
                                        if method_names.contains(member) {
                                            module_env.set(
                                                member.clone(),
                                                merge_method_binding(existing, value),
                                            );
                                            continue;
                                        }
                                    }
                                    module_env.set(member.clone(), value);
                                }
                            }
                        }
                    }
                }
            }
        }

        bind_module_machine_values(
            surface_module,
            &module_name,
            &module_env,
            &globals,
            &mut machine_specs,
        );

        // Re-apply local defs after imports so that local definitions always
        // shadow imported names (including domain members).  Without this,
        // a wildcard `use` that brings in a domain method with the same name
        // as a local binding would silently overwrite the local definition.
        for def in &module.defs {
            let qualified = format!("{module_name}.{}", def.name);
            if let Some(value) = globals.get(&qualified) {
                module_env.set(def.name.clone(), value);
            }
        }

        // Re-export forwarding: a module can `export x` where `x` is brought into scope via `use`
        // (e.g. facade modules like `aivi.linalg`). Ensure qualified access `Module.x` resolves by
        // registering exported bindings that exist in the module env, even when they aren't local
        // definitions.
        for export in &surface_module.exports {
            if export.kind != crate::surface::ScopeItemKind::Value {
                continue;
            }
            let name = export.name.name.clone();
            let qualified = format!("{module_name}.{name}");
            if globals.get(&qualified).is_some() {
                continue;
            }
            if let Some(value) = module_env.get(&name) {
                globals.set(qualified, value);
            }
        }
    }

    let mut constructor_ordinals = core_constructor_ordinals();
    for (name, ordinal) in collect_surface_constructor_ordinals(surface_modules) {
        match ordinal {
            Some(idx) => insert_constructor_ordinal(&mut constructor_ordinals, name, idx),
            None => {
                constructor_ordinals.insert(name, None);
            }
        }
    }
    let ctx = Arc::new(RuntimeContext::new_with_constructor_ordinals(
        globals,
        constructor_ordinals,
    ));
    for (machine_name, initial_state, transitions) in machine_specs {
        ctx.register_machine(machine_name, initial_state, transitions);
    }
    let cancel = CancelToken::root();
    Ok(Runtime::new(ctx, cancel))
}

fn runtime_builtin(
    name: &str,
    arity: usize,
    func: impl Fn(Vec<Value>, &mut Runtime) -> Result<Value, RuntimeError> + Send + Sync + 'static,
) -> Value {
    Value::Builtin(BuiltinValue {
        imp: Arc::new(BuiltinImpl {
            name: name.to_string(),
            arity,
            func: Arc::new(func),
        }),
        args: Vec::new(),
        tagged_args: Some(Vec::new()),
    })
}

fn machine_transition_builtin_name(machine_name: &str, event: &str) -> String {
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

fn make_machine_on_builtin() -> Value {
    runtime_builtin("__machine_on", 2, |mut args, _| {
        let handler = args.pop().unwrap_or(Value::Unit);
        let transition = args.pop().unwrap_or(Value::Unit);
        if let Some((machine_name, event_name)) = parse_machine_transition_ref(&transition) {
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    runtime
                        .ctx
                        .register_machine_handler(&machine_name, &event_name, handler.clone());
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

fn make_machine_transition_builtin(machine_name: String, event_name: String) -> Value {
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

fn make_machine_current_state_builtin(machine_name: String) -> Value {
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

fn make_machine_can_builtin(machine_name: String, event_name: String) -> Value {
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

fn bind_module_machine_values(
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
            .or_else(|| machine_decl.states.first().map(|state| state.name.name.clone()))
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

pub(crate) fn format_runtime_error(err: RuntimeError) -> String {
    match err {
        RuntimeError::Cancelled => "execution cancelled".to_string(),
        RuntimeError::Message(message) => message,
        RuntimeError::Error(value) => format!("runtime error: {}", format_value(&value)),
    }
}

fn insert_constructor_ordinal(
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

fn core_constructor_ordinals() -> HashMap<String, Option<usize>> {
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

fn collect_surface_constructor_ordinals(
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

include!("runtime_impl/lifecycle_and_cancel.rs");
include!("runtime_impl/eval_and_apply.rs");
include!("runtime_impl/resources.rs");
include!("runtime_impl/trampoline.rs");

impl BuiltinValue {
    fn apply(&self, arg: Value, runtime: &mut Runtime) -> Result<Value, RuntimeError> {
        let mut args = self.args.clone();
        let mut tagged_args = self.tagged_args.clone();
        let mut pending_arg = Some(arg);
        if let Some(existing) = tagged_args.as_mut() {
            if let Some(tagged) = TaggedValue::from_value(pending_arg.as_ref().expect("pending arg")) {
                existing.push(tagged);
                pending_arg = None;
            } else {
                args = existing.iter().copied().map(TaggedValue::to_value).collect();
                tagged_args = None;
            }
        }
        if let Some(arg) = pending_arg {
            args.push(arg);
        }
        if args.is_empty() {
            if let Some(existing) = tagged_args.as_ref() {
                if existing.len() == self.imp.arity {
                    args = existing.iter().copied().map(TaggedValue::to_value).collect();
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
        HirPattern::At { name, pattern, .. } => {
            bindings.insert(name.clone(), value.clone());
            match_pattern(pattern, value, bindings)
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
