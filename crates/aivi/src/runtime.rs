use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Datelike, NaiveDate};
use regex::RegexBuilder;
use url::Url;

use rudo_gc::GcMutex;

use crate::hir::{
    HirBlockItem, HirExpr, HirListItem, HirLiteral, HirMatchArm, HirPathSegment, HirPattern,
    HirProgram, HirRecordField, HirTextPart,
};
use crate::AiviError;

mod environment;
mod builtins;
mod http;
mod values;
#[cfg(test)]
mod tests;

use self::builtins::register_builtins;
use self::environment::{Env, RuntimeContext};
use self::values::{BuiltinValue, ClosureValue, EffectValue, ResourceValue, ThunkValue, Value};

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
    rng_state: u64,
}

#[derive(Clone)]
enum RuntimeError {
    Error(Value),
    Cancelled,
    Message(String),
}

pub fn run_native(program: HirProgram) -> Result<(), AiviError> {
    if program.modules.is_empty() {
        return Err(AiviError::Runtime("no modules to run".to_string()));
    }

    let mut grouped: HashMap<String, Vec<HirExpr>> = HashMap::new();
    for module in program.modules {
        for def in module.defs {
            grouped.entry(def.name).or_default().push(def.expr);
        }
    }
    if grouped.is_empty() {
        return Err(AiviError::Runtime("no definitions to run".to_string()));
    }

    let globals = Env::new(None);
    register_builtins(&globals);
    for (name, exprs) in grouped {
        if exprs.len() == 1 {
            let thunk = ThunkValue {
                expr: Arc::new(exprs.into_iter().next().unwrap()),
                env: globals.clone(),
                cached: GcMutex::new(None),
                in_progress: AtomicBool::new(false),
            };
            globals.set(name, Value::Thunk(Arc::new(thunk)));
        } else {
            let mut clauses = Vec::new();
            for expr in exprs {
                let thunk = ThunkValue {
                    expr: Arc::new(expr),
                    env: globals.clone(),
                    cached: GcMutex::new(None),
                    in_progress: AtomicBool::new(false),
                };
                clauses.push(Value::Thunk(Arc::new(thunk)));
            }
            globals.set(name, Value::MultiClause(clauses));
        }
    }

    let ctx = Arc::new(RuntimeContext { globals });
    let cancel = CancelToken::root();
    let mut runtime = Runtime::new(ctx, cancel);

    let main = runtime
        .ctx
        .globals
        .get("main")
        .ok_or_else(|| AiviError::Runtime("missing main definition".to_string()))?;
    let main_value = match runtime.force_value(main) {
        Ok(value) => value,
        Err(RuntimeError::Cancelled) => {
            return Err(AiviError::Runtime("execution cancelled".to_string()))
        }
        Err(RuntimeError::Message(message)) => return Err(AiviError::Runtime(message)),
        Err(RuntimeError::Error(value)) => {
            return Err(AiviError::Runtime(format!(
                "runtime error: {}",
                format_value(&value)
            )))
        }
    };
    let effect = match main_value {
        Value::Effect(effect) => Value::Effect(effect),
        _ => {
            return Err(AiviError::Runtime(
                "main must be an Effect value".to_string(),
            ))
        }
    };

    match runtime.run_effect_value(effect) {
        Ok(_) => Ok(()),
        Err(RuntimeError::Cancelled) => Err(AiviError::Runtime("execution cancelled".to_string())),
        Err(RuntimeError::Message(message)) => Err(AiviError::Runtime(message)),
        Err(RuntimeError::Error(value)) => Err(AiviError::Runtime(format!(
            "runtime error: {}",
            format_value(&value)
        ))),
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
            rng_state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    fn check_cancelled(&self) -> Result<(), RuntimeError> {
        if self.cancel_mask > 0 {
            return Ok(());
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
        let cached = thunk.cached.lock();
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
        let mut cached = thunk.cached.lock();
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
                if is_constructor_name(name) {
                    return Ok(Value::Constructor {
                        name: name.clone(),
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
            } => {
                match tag.as_str() {
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
                        let date = NaiveDate::parse_from_str(body, "%Y-%m-%d")
                            .map_err(|err| {
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
                    _ => {
                        let mut map = HashMap::new();
                        map.insert("tag".to_string(), Value::Text(tag.clone()));
                        map.insert("body".to_string(), Value::Text(body.clone()));
                        map.insert("flags".to_string(), Value::Text(flags.clone()));
                        Ok(Value::Record(Arc::new(map)))
                    }
                }
            }
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
                let idx = match index_value {
                    Value::Int(value) => value,
                    _ => return Err(RuntimeError::Message("index expects an Int".to_string())),
                };
                match base_value {
                    Value::List(items) => {
                        let idx = idx as usize;
                        items
                            .get(idx)
                            .cloned()
                            .ok_or_else(|| RuntimeError::Message("index out of bounds".to_string()))
                    }
                    Value::Tuple(items) => {
                        let idx = idx as usize;
                        items
                            .get(idx)
                            .cloned()
                            .ok_or_else(|| RuntimeError::Message("index out of bounds".to_string()))
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
                crate::hir::HirBlockKind::Generate => Err(RuntimeError::Message(
                    "generator blocks are not supported in native runtime yet".to_string(),
                )),
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
            _ => Err(RuntimeError::Message(
                "attempted to call a non-function".to_string(),
            )),
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
                HirPathSegment::Index(_) => {
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
            HirPathSegment::Index(_) => Err(RuntimeError::Message(
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

        for (index, item) in items.iter().enumerate() {
            let last = index + 1 == items.len();
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
            HirPathSegment::Index(_) => {
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
            a == b && aa.iter().zip(bb.iter()).all(|(x, y)| values_equal(x, y))
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

fn is_constructor_name(name: &str) -> bool {
    name.chars()
        .next()
        .map(|ch| ch.is_ascii_uppercase())
        .unwrap_or(false)
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
    map.insert("protocol".to_string(), Value::Text(url.scheme().to_string()));
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
