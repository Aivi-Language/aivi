use std::collections::HashMap;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aivi_http_server::{
    AiviHttpError, AiviRequest, AiviResponse, AiviWsMessage, Handler, ServerReply,
    WebSocketHandle,
};
use crate::hir::{
    HirBlockItem, HirExpr, HirListItem, HirLiteral, HirMatchArm, HirPathSegment, HirPattern,
    HirProgram, HirRecordField, HirTextPart,
};
use crate::AiviError;

mod environment;
mod values;

use self::environment::{Env, RuntimeContext};
use self::values::{
    BuiltinImpl, BuiltinValue, ChannelInner, ChannelRecv, ChannelSend, ClosureValue, EffectValue,
    ResourceValue, ThunkValue, Value,
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
        if let Ok(cached) = thunk.cached.lock() {
            if let Some(value) = cached.clone() {
                return Ok(value);
            }
        }
        if thunk.in_progress.swap(true, Ordering::SeqCst) {
            return Err(RuntimeError::Message(
                "recursive definition detected".to_string(),
            ));
        }
        let value = self.eval_expr(&thunk.expr, &thunk.env)?;
        if let Ok(mut cached) = thunk.cached.lock() {
            *cached = Some(value.clone());
        }
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
                let mut map = HashMap::new();
                map.insert("tag".to_string(), Value::Text(tag.clone()));
                map.insert("body".to_string(), Value::Text(body.clone()));
                map.insert("flags".to_string(), Value::Text(flags.clone()));
                Ok(Value::Record(map))
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
                let new_env = Env::new(Some(Arc::new(closure.env.clone())));
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
        let local_env = Env::new(Some(Arc::new(env.clone())));
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
                    let guard_env = Env::new(Some(Arc::new(env.clone())));
                    for (name, value) in bindings.clone() {
                        guard_env.set(name, value);
                    }
                    let guard_value = self.eval_expr(guard, &guard_env)?;
                    if !matches!(guard_value, Value::Bool(true)) {
                        continue;
                    }
                }
                let arm_env = Env::new(Some(Arc::new(env.clone())));
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
                    Value::List(mut inner) => values.append(&mut inner),
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
        Ok(Value::List(values))
    }

    fn eval_record(&mut self, fields: &[HirRecordField], env: &Env) -> Result<Value, RuntimeError> {
        let mut map = HashMap::new();
        for field in fields {
            let value = self.eval_expr(&field.value, env)?;
            insert_record_path(&mut map, &field.path, value)?;
        }
        Ok(Value::Record(map))
    }

    fn eval_patch(
        &mut self,
        target: &HirExpr,
        fields: &[HirRecordField],
        env: &Env,
    ) -> Result<Value, RuntimeError> {
        let base_value = self.eval_expr(target, env)?;
        let Value::Record(mut map) = base_value else {
            return Err(RuntimeError::Message(
                "patch target must be a record".to_string(),
            ));
        };
        for field in fields {
            self.apply_patch_field(&mut map, &field.path, &field.value, env)?;
        }
        Ok(Value::Record(map))
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
                        .or_insert_with(|| Value::Record(HashMap::new()));
                    match entry {
                        Value::Record(map) => {
                            current = map;
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
        let local_env = Env::new(Some(Arc::new(env)));
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
                        _ => Err(RuntimeError::Message(
                            "effect bind expects Effect or Resource".to_string(),
                        )),
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
        let local_env = Env::new(Some(Arc::new(env.clone())));
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
                    match_pattern(rest, &Value::List(tail), bindings)
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
            Some(Value::Record(map)) => current = map,
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
                    .or_insert_with(|| Value::Record(HashMap::new()));
                match entry {
                    Value::Record(map) => {
                        current = map;
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
        ("==", a, b) => Some(Value::Bool(values_equal(a, b))),
        ("!=", a, b) => Some(Value::Bool(!values_equal(a, b))),
        ("<", Value::Int(a), Value::Int(b)) => Some(Value::Bool(a < b)),
        ("<=", Value::Int(a), Value::Int(b)) => Some(Value::Bool(a <= b)),
        (">", Value::Int(a), Value::Int(b)) => Some(Value::Bool(a > b)),
        (">=", Value::Int(a), Value::Int(b)) => Some(Value::Bool(a >= b)),
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

fn register_builtins(env: &Env) {
    env.set("Unit".to_string(), Value::Unit);
    env.set("True".to_string(), Value::Bool(true));
    env.set("False".to_string(), Value::Bool(false));
    env.set(
        "None".to_string(),
        Value::Constructor {
            name: "None".to_string(),
            args: Vec::new(),
        },
    );
    env.set("Some".to_string(), builtin_constructor("Some", 1));
    env.set("Ok".to_string(), builtin_constructor("Ok", 1));
    env.set("Err".to_string(), builtin_constructor("Err", 1));
    env.set(
        "Closed".to_string(),
        Value::Constructor {
            name: "Closed".to_string(),
            args: Vec::new(),
        },
    );

    env.set(
        "pure".to_string(),
        builtin("pure", 1, |mut args, _| {
            let value = args.remove(0);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| Ok(value.clone())),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    env.set(
        "fail".to_string(),
        builtin("fail", 1, |mut args, _| {
            let value = args.remove(0);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| Err(RuntimeError::Error(value.clone()))),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    env.set(
        "bind".to_string(),
        builtin("bind", 2, |mut args, _| {
            let func = args.pop().unwrap();
            let effect = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let value = runtime.run_effect_value(effect.clone())?;
                    let applied = runtime.apply(func.clone(), value)?;
                    runtime.run_effect_value(applied)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    env.set(
        "attempt".to_string(),
        builtin("attempt", 1, |mut args, _| {
            let effect = args.remove(0);
            let effect = EffectValue::Thunk {
                func: Arc::new(
                    move |runtime| match runtime.run_effect_value(effect.clone()) {
                        Ok(value) => Ok(Value::Constructor {
                            name: "Ok".to_string(),
                            args: vec![value],
                        }),
                        Err(RuntimeError::Error(value)) => Ok(Value::Constructor {
                            name: "Err".to_string(),
                            args: vec![value],
                        }),
                        Err(err) => Err(err),
                    },
                ),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    env.set(
        "print".to_string(),
        builtin("print", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    print!("{text}");
                    let mut out = std::io::stdout();
                    let _ = out.flush();
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    env.set(
        "println".to_string(),
        builtin("println", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    println!("{text}");
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    env.set(
        "load".to_string(),
        builtin("load", 1, |mut args, _| {
            let value = args.remove(0);
            match value {
                Value::Effect(_) => Ok(value),
                _ => Err(RuntimeError::Message("load expects an Effect".to_string())),
            }
        }),
    );

    env.set("file".to_string(), build_file_record());
    env.set("clock".to_string(), build_clock_record());
    env.set("random".to_string(), build_random_record());
    env.set("channel".to_string(), build_channel_record());
    env.set("concurrent".to_string(), build_concurrent_record());
    env.set("httpServer".to_string(), build_http_server_record());
}

fn builtin(
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
    })
}

fn builtin_constructor(name: &str, arity: usize) -> Value {
    let name_owned = name.to_string();
    builtin(name, arity, move |args, _| {
        Ok(Value::Constructor {
            name: name_owned.clone(),
            args,
        })
    })
}

fn build_file_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "read".to_string(),
        builtin("file.read", 1, |mut args, _| {
            let path = match args.remove(0) {
                Value::Text(text) => text,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.read expects Text path".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| match std::fs::read_to_string(&path) {
                    Ok(text) => Ok(Value::Text(text)),
                    Err(err) => Err(RuntimeError::Error(Value::Text(err.to_string()))),
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "open".to_string(),
        builtin("file.open", 1, |mut args, _| {
            let path = match args.remove(0) {
                Value::Text(text) => text,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.open expects Text path".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| match std::fs::File::open(&path) {
                    Ok(file) => Ok(Value::FileHandle(Arc::new(Mutex::new(file)))),
                    Err(err) => Err(RuntimeError::Error(Value::Text(err.to_string()))),
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "close".to_string(),
        builtin("file.close", 1, |mut args, _| {
            let _handle = match args.remove(0) {
                Value::FileHandle(handle) => handle,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.close expects a file handle".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| Ok(Value::Unit)),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "readAll".to_string(),
        builtin("file.readAll", 1, |mut args, _| {
            let handle = match args.remove(0) {
                Value::FileHandle(handle) => handle,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.readAll expects a file handle".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut file = handle
                        .lock()
                        .map_err(|_| RuntimeError::Message("file handle poisoned".to_string()))?;
                    let _ = std::io::Seek::seek(&mut *file, std::io::SeekFrom::Start(0));
                    let mut buffer = String::new();
                    std::io::Read::read_to_string(&mut *file, &mut buffer)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.to_string())))?;
                    Ok(Value::Text(buffer))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(fields)
}

fn build_clock_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "now".to_string(),
        builtin("clock.now", 1, |_, _| {
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or(Duration::from_secs(0));
                    let text = format!("{}.{:09}Z", now.as_secs(), now.subsec_nanos());
                    Ok(Value::DateTime(text))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(fields)
}

fn build_random_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "int".to_string(),
        builtin("random.int", 2, |mut args, _runtime| {
            let max = match args.pop().unwrap() {
                Value::Int(value) => value,
                _ => {
                    return Err(RuntimeError::Message(
                        "random.int expects Int bounds".to_string(),
                    ))
                }
            };
            let min = match args.pop().unwrap() {
                Value::Int(value) => value,
                _ => {
                    return Err(RuntimeError::Message(
                        "random.int expects Int bounds".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let (low, high) = if min <= max { (min, max) } else { (max, min) };
                    let span = (high - low + 1) as u64;
                    let value = (runtime.next_u64() % span) as i64 + low;
                    Ok(Value::Int(value))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(fields)
}

fn build_channel_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "make".to_string(),
        builtin("channel.make", 1, |_, _| {
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let (sender, receiver) = mpsc::channel();
                    let inner = Arc::new(ChannelInner {
                        sender: Mutex::new(Some(sender)),
                        receiver: Mutex::new(receiver),
                        closed: AtomicBool::new(false),
                    });
                    let send = Value::ChannelSend(Arc::new(ChannelSend {
                        inner: inner.clone(),
                    }));
                    let recv = Value::ChannelRecv(Arc::new(ChannelRecv { inner }));
                    Ok(Value::Tuple(vec![send, recv]))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "send".to_string(),
        builtin("channel.send", 2, |mut args, _| {
            let value = args.pop().unwrap();
            let sender = match args.pop().unwrap() {
                Value::ChannelSend(handle) => handle,
                _ => {
                    return Err(RuntimeError::Message(
                        "channel.send expects a send handle".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    if sender.inner.closed.load(Ordering::SeqCst) {
                        return Err(RuntimeError::Error(Value::Constructor {
                            name: "Closed".to_string(),
                            args: Vec::new(),
                        }));
                    }
                    let sender_guard = sender
                        .inner
                        .sender
                        .lock()
                        .map_err(|_| RuntimeError::Message("channel poisoned".to_string()))?;
                    if let Some(sender) = sender_guard.as_ref() {
                        sender.send(value.clone()).map_err(|_| {
                            RuntimeError::Error(Value::Constructor {
                                name: "Closed".to_string(),
                                args: Vec::new(),
                            })
                        })?;
                        Ok(Value::Unit)
                    } else {
                        Err(RuntimeError::Error(Value::Constructor {
                            name: "Closed".to_string(),
                            args: Vec::new(),
                        }))
                    }
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "recv".to_string(),
        builtin("channel.recv", 1, |mut args, _| {
            let receiver = match args.pop().unwrap() {
                Value::ChannelRecv(handle) => handle,
                _ => {
                    return Err(RuntimeError::Message(
                        "channel.recv expects a recv handle".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| loop {
                    runtime.check_cancelled()?;
                    let recv_guard = receiver
                        .inner
                        .receiver
                        .lock()
                        .map_err(|_| RuntimeError::Message("channel poisoned".to_string()))?;
                    match recv_guard.recv_timeout(Duration::from_millis(25)) {
                        Ok(value) => {
                            return Ok(Value::Constructor {
                                name: "Ok".to_string(),
                                args: vec![value],
                            });
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            return Ok(Value::Constructor {
                                name: "Err".to_string(),
                                args: vec![Value::Constructor {
                                    name: "Closed".to_string(),
                                    args: Vec::new(),
                                }],
                            })
                        }
                    }
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "close".to_string(),
        builtin("channel.close", 1, |mut args, _| {
            let sender = match args.pop().unwrap() {
                Value::ChannelSend(handle) => handle,
                _ => {
                    return Err(RuntimeError::Message(
                        "channel.close expects a send handle".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    sender.inner.closed.store(true, Ordering::SeqCst);
                    if let Ok(mut guard) = sender.inner.sender.lock() {
                        guard.take();
                    }
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(fields)
}

fn build_concurrent_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "scope".to_string(),
        builtin("concurrent.scope", 1, |mut args, runtime| {
            let effect = args.pop().unwrap();
            let ctx = runtime.ctx.clone();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let cancel = CancelToken::child(runtime.cancel.clone());
                    let mut child = Runtime::new(ctx.clone(), cancel.clone());
                    let result = child.run_effect_value(effect.clone());
                    cancel.cancel();
                    result
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "par".to_string(),
        builtin("concurrent.par", 2, |mut args, runtime| {
            let right = args.pop().unwrap();
            let left = args.pop().unwrap();
            let ctx = runtime.ctx.clone();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let left_cancel = CancelToken::child(runtime.cancel.clone());
                    let right_cancel = CancelToken::child(runtime.cancel.clone());
                    let (tx, rx) = mpsc::channel();
                    spawn_effect(
                        0,
                        left.clone(),
                        ctx.clone(),
                        left_cancel.clone(),
                        tx.clone(),
                    );
                    spawn_effect(
                        1,
                        right.clone(),
                        ctx.clone(),
                        right_cancel.clone(),
                        tx.clone(),
                    );

                    let mut left_result = None;
                    let mut right_result = None;
                    let mut cancelled = false;
                    while left_result.is_none() || right_result.is_none() {
                        if runtime.check_cancelled().is_err() {
                            cancelled = true;
                            left_cancel.cancel();
                            right_cancel.cancel();
                        }
                        let (id, result) = match rx.recv_timeout(Duration::from_millis(25)) {
                            Ok(value) => value,
                            Err(mpsc::RecvTimeoutError::Timeout) => continue,
                            Err(mpsc::RecvTimeoutError::Disconnected) => {
                                return Err(RuntimeError::Message("worker stopped".to_string()))
                            }
                        };
                        if id == 0 {
                            if result.is_err() {
                                right_cancel.cancel();
                            }
                            left_result = Some(result);
                        } else {
                            if result.is_err() {
                                left_cancel.cancel();
                            }
                            right_result = Some(result);
                        }
                    }

                    if cancelled {
                        return Err(RuntimeError::Cancelled);
                    }

                    let left_result = left_result.unwrap();
                    let right_result = right_result.unwrap();
                    match (left_result, right_result) {
                        (Ok(left_value), Ok(right_value)) => {
                            Ok(Value::Tuple(vec![left_value, right_value]))
                        }
                        (Err(err), _) => Err(err),
                        (_, Err(err)) => Err(err),
                    }
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "race".to_string(),
        builtin("concurrent.race", 2, |mut args, runtime| {
            let right = args.pop().unwrap();
            let left = args.pop().unwrap();
            let ctx = runtime.ctx.clone();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let left_cancel = CancelToken::child(runtime.cancel.clone());
                    let right_cancel = CancelToken::child(runtime.cancel.clone());
                    let (tx, rx) = mpsc::channel();
                    spawn_effect(
                        0,
                        left.clone(),
                        ctx.clone(),
                        left_cancel.clone(),
                        tx.clone(),
                    );
                    spawn_effect(
                        1,
                        right.clone(),
                        ctx.clone(),
                        right_cancel.clone(),
                        tx.clone(),
                    );

                    let mut cancelled = false;
                    let (winner, result) = loop {
                        if runtime.check_cancelled().is_err() {
                            cancelled = true;
                            left_cancel.cancel();
                            right_cancel.cancel();
                        }
                        match rx.recv_timeout(Duration::from_millis(25)) {
                            Ok(value) => break value,
                            Err(mpsc::RecvTimeoutError::Timeout) => continue,
                            Err(mpsc::RecvTimeoutError::Disconnected) => {
                                return Err(RuntimeError::Message("worker stopped".to_string()))
                            }
                        }
                    };
                    if winner == 0 {
                        right_cancel.cancel();
                    } else {
                        left_cancel.cancel();
                    }
                    while rx.recv_timeout(Duration::from_millis(25)).is_err() {
                        if runtime.check_cancelled().is_err() {
                            cancelled = true;
                            left_cancel.cancel();
                            right_cancel.cancel();
                        }
                    }
                    if cancelled {
                        return Err(RuntimeError::Cancelled);
                    }
                    result
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "spawnDetached".to_string(),
        builtin("concurrent.spawnDetached", 1, |mut args, runtime| {
            let effect_value = args.pop().unwrap();
            let ctx = runtime.ctx.clone();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let parent = runtime
                        .cancel
                        .parent()
                        .unwrap_or_else(|| runtime.cancel.clone());
                    let cancel = CancelToken::child(parent);
                    let (tx, _rx) = mpsc::channel();
                    spawn_effect(0, effect_value.clone(), ctx.clone(), cancel, tx);
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(fields)
}

fn build_http_server_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "listen".to_string(),
        builtin("httpServer.listen", 2, |mut args, runtime| {
            let handler = args.pop().unwrap();
            let config = args.pop().unwrap();
            let addr = parse_server_config(config)?;
            let ctx = runtime.ctx.clone();
            let handler_value = handler.clone();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let handler: Handler = Arc::new(move |req: AiviRequest| {
                        let handler_value = handler_value.clone();
                        let ctx = ctx.clone();
                        Box::pin(async move {
                            let req_value = request_to_value(req);
                            let ctx_for_reply = ctx.clone();
                            let result = tokio::task::spawn_blocking(move || {
                                let cancel = CancelToken::root();
                                let mut runtime = Runtime::new(ctx.clone(), cancel);
                                let applied = runtime.apply(handler_value, req_value)?;
                                runtime.run_effect_value(applied)
                            })
                            .await
                            .map_err(|err| AiviHttpError {
                                message: err.to_string(),
                            })?;
                            match result {
                                Ok(value) => server_reply_from_value(value, ctx_for_reply),
                                Err(err) => Err(runtime_error_to_http_error(err)),
                            }
                        })
                    });
                    let server = aivi_http_server::start_server(addr, handler)
                        .map_err(|err| RuntimeError::Error(http_error_value(err.message)))?;
                    Ok(Value::HttpServer(Arc::new(server)))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "stop".to_string(),
        builtin("httpServer.stop", 1, |mut args, _| {
            let server = match args.pop().unwrap() {
                Value::HttpServer(handle) => handle,
                _ => {
                    return Err(RuntimeError::Message(
                        "httpServer.stop expects a server handle".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    server
                        .stop()
                        .map_err(|err| RuntimeError::Error(http_error_value(err.message)))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "ws_recv".to_string(),
        builtin("httpServer.ws_recv", 1, |mut args, _| {
            let socket = match args.pop().unwrap() {
                Value::WebSocket(handle) => handle,
                _ => {
                    return Err(RuntimeError::Message(
                        "httpServer.ws_recv expects a websocket".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let msg = socket
                        .recv()
                        .map_err(|err| RuntimeError::Error(ws_error_value(err.message)))?;
                    Ok(ws_message_to_value(msg))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "ws_send".to_string(),
        builtin("httpServer.ws_send", 2, |mut args, _| {
            let message = args.pop().unwrap();
            let socket = match args.pop().unwrap() {
                Value::WebSocket(handle) => handle,
                _ => {
                    return Err(RuntimeError::Message(
                        "httpServer.ws_send expects a websocket".to_string(),
                    ))
                }
            };
            let ws_message = value_to_ws_message(message)?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    socket
                        .send(ws_message.clone())
                        .map_err(|err| RuntimeError::Error(ws_error_value(err.message)))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "ws_close".to_string(),
        builtin("httpServer.ws_close", 1, |mut args, _| {
            let socket = match args.pop().unwrap() {
                Value::WebSocket(handle) => handle,
                _ => {
                    return Err(RuntimeError::Message(
                        "httpServer.ws_close expects a websocket".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    socket
                        .close()
                        .map_err(|err| RuntimeError::Error(ws_error_value(err.message)))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(fields)
}

fn parse_server_config(value: Value) -> Result<SocketAddr, RuntimeError> {
    let record = expect_record(value, "httpServer.listen expects ServerConfig")?;
    let address = match record.get("address") {
        Some(Value::Text(text)) => text.clone(),
        _ => {
            return Err(RuntimeError::Message(
                "httpServer.listen expects ServerConfig.address Text".to_string(),
            ))
        }
    };
    address.parse().map_err(|_| {
        RuntimeError::Message("httpServer.listen address must be host:port".to_string())
    })
}

fn request_to_value(req: AiviRequest) -> Value {
    let mut fields = HashMap::new();
    fields.insert("method".to_string(), Value::Text(req.method));
    fields.insert("path".to_string(), Value::Text(req.path));
    fields.insert("headers".to_string(), headers_to_value(req.headers));
    fields.insert("body".to_string(), bytes_to_list_value(req.body));
    fields.insert(
        "remote_addr".to_string(),
        match req.remote_addr {
            Some(value) => Value::Constructor {
                name: "Some".to_string(),
                args: vec![Value::Text(value)],
            },
            None => Value::Constructor {
                name: "None".to_string(),
                args: Vec::new(),
            },
        },
    );
    Value::Record(fields)
}

fn server_reply_from_value(
    value: Value,
    ctx: Arc<RuntimeContext>,
) -> Result<ServerReply, AiviHttpError> {
    match value {
        Value::Constructor { name, mut args } if name == "Http" => {
            if args.len() != 1 {
                return Err(AiviHttpError {
                    message: "Http response expects 1 argument".to_string(),
                });
            }
            let response_value = args.pop().unwrap();
            let response = response_from_value(response_value)?;
            Ok(ServerReply::Http(response))
        }
        Value::Constructor { name, mut args } if name == "Ws" => {
            if args.len() != 1 {
                return Err(AiviHttpError {
                    message: "Ws response expects 1 argument".to_string(),
                });
            }
            let ws_handler_value = args.pop().unwrap();
            let ws_handler = Arc::new(move |socket: WebSocketHandle| {
                let ctx = ctx.clone();
                let ws_handler_value = ws_handler_value.clone();
                Box::pin(async move {
                    let socket_value = Value::WebSocket(Arc::new(socket));
                    let result = tokio::task::spawn_blocking(move || {
                        let cancel = CancelToken::root();
                        let mut runtime = Runtime::new(ctx.clone(), cancel);
                        let applied = runtime.apply(ws_handler_value, socket_value)?;
                        runtime.run_effect_value(applied)
                    })
                    .await
                    .map_err(|err| AiviHttpError {
                        message: err.to_string(),
                    })?;
                    match result {
                        Ok(Value::Unit) => Ok(()),
                        Ok(_) => Ok(()),
                        Err(err) => Err(runtime_error_to_http_error(err)),
                    }
                })
            });
            Ok(ServerReply::Ws(ws_handler))
        }
        other => Err(AiviHttpError {
            message: format!(
                "expected ServerReply (Http|Ws), got {}",
                format_value(&other)
            ),
        }),
    }
}

fn response_from_value(value: Value) -> Result<AiviResponse, AiviHttpError> {
    let record = expect_record(value, "Response must be a record")
        .map_err(runtime_error_to_http_error)?;
    let status = match record.get("status") {
        Some(Value::Int(value)) => *value,
        _ => {
            return Err(AiviHttpError {
                message: "Response.status must be Int".to_string(),
            })
        }
    };
    let headers = match record.get("headers") {
        Some(Value::List(items)) => headers_from_value(items)?,
        _ => {
            return Err(AiviHttpError {
                message: "Response.headers must be List".to_string(),
            })
        }
    };
    let body = match record.get("body") {
        Some(Value::List(items)) => list_value_to_bytes(items)?,
        _ => {
            return Err(AiviHttpError {
                message: "Response.body must be List Int".to_string(),
            })
        }
    };
    Ok(AiviResponse {
        status: status as u16,
        headers,
        body,
    })
}

fn headers_to_value(headers: Vec<(String, String)>) -> Value {
    let values = headers
        .into_iter()
        .map(|(name, value)| {
            let mut fields = HashMap::new();
            fields.insert("name".to_string(), Value::Text(name));
            fields.insert("value".to_string(), Value::Text(value));
            Value::Record(fields)
        })
        .collect();
    Value::List(values)
}

fn headers_from_value(values: &[Value]) -> Result<Vec<(String, String)>, AiviHttpError> {
    let mut headers = Vec::new();
    for value in values {
        let record = match value {
            Value::Record(record) => record,
            _ => {
                return Err(AiviHttpError {
                    message: "Header must be a record".to_string(),
                })
            }
        };
        let name = match record.get("name") {
            Some(Value::Text(text)) => text.clone(),
            _ => {
                return Err(AiviHttpError {
                    message: "Header.name must be Text".to_string(),
                })
            }
        };
        let value = match record.get("value") {
            Some(Value::Text(text)) => text.clone(),
            _ => {
                return Err(AiviHttpError {
                    message: "Header.value must be Text".to_string(),
                })
            }
        };
        headers.push((name, value));
    }
    Ok(headers)
}

fn bytes_to_list_value(bytes: Vec<u8>) -> Value {
    let items = bytes
        .into_iter()
        .map(|value| Value::Int(value as i64))
        .collect();
    Value::List(items)
}

fn list_value_to_bytes(values: &[Value]) -> Result<Vec<u8>, AiviHttpError> {
    let mut bytes = Vec::with_capacity(values.len());
    for value in values {
        let int = match value {
            Value::Int(value) => *value,
            _ => {
                return Err(AiviHttpError {
                    message: "expected List Int for bytes".to_string(),
                })
            }
        };
        if !(0..=255).contains(&int) {
            return Err(AiviHttpError {
                message: "byte out of range".to_string(),
            });
        }
        bytes.push(int as u8);
    }
    Ok(bytes)
}

fn ws_message_to_value(msg: AiviWsMessage) -> Value {
    match msg {
        AiviWsMessage::Text(text) => Value::Constructor {
            name: "TextMsg".to_string(),
            args: vec![Value::Text(text)],
        },
        AiviWsMessage::Binary(bytes) => Value::Constructor {
            name: "BinaryMsg".to_string(),
            args: vec![bytes_to_list_value(bytes)],
        },
        AiviWsMessage::Ping => Value::Constructor {
            name: "Ping".to_string(),
            args: Vec::new(),
        },
        AiviWsMessage::Pong => Value::Constructor {
            name: "Pong".to_string(),
            args: Vec::new(),
        },
        AiviWsMessage::Close => Value::Constructor {
            name: "Close".to_string(),
            args: Vec::new(),
        },
    }
}

fn value_to_ws_message(value: Value) -> Result<AiviWsMessage, RuntimeError> {
    match value {
        Value::Constructor { name, mut args } if name == "TextMsg" => {
            if args.len() != 1 {
                return Err(RuntimeError::Message(
                    "TextMsg expects 1 argument".to_string(),
                ));
            }
            match args.pop().unwrap() {
                Value::Text(text) => Ok(AiviWsMessage::Text(text)),
                _ => Err(RuntimeError::Message(
                    "TextMsg expects Text".to_string(),
                )),
            }
        }
        Value::Constructor { name, mut args } if name == "BinaryMsg" => {
            if args.len() != 1 {
                return Err(RuntimeError::Message(
                    "BinaryMsg expects 1 argument".to_string(),
                ));
            }
            match args.pop().unwrap() {
                Value::List(items) => list_value_to_bytes(&items)
                    .map(AiviWsMessage::Binary)
                    .map_err(|err| RuntimeError::Message(err.message)),
                _ => Err(RuntimeError::Message(
                    "BinaryMsg expects List Int".to_string(),
                )),
            }
        }
        Value::Constructor { name, args } if name == "Ping" && args.is_empty() => {
            Ok(AiviWsMessage::Ping)
        }
        Value::Constructor { name, args } if name == "Pong" && args.is_empty() => {
            Ok(AiviWsMessage::Pong)
        }
        Value::Constructor { name, args } if name == "Close" && args.is_empty() => {
            Ok(AiviWsMessage::Close)
        }
        other => Err(RuntimeError::Message(format!(
            "invalid WsMessage value {}",
            format_value(&other)
        ))),
    }
}

fn expect_record(value: Value, message: &str) -> Result<HashMap<String, Value>, RuntimeError> {
    match value {
        Value::Record(record) => Ok(record),
        _ => Err(RuntimeError::Message(message.to_string())),
    }
}

fn http_error_value(message: String) -> Value {
    let mut fields = HashMap::new();
    fields.insert("message".to_string(), Value::Text(message));
    Value::Record(fields)
}

fn ws_error_value(message: String) -> Value {
    let mut fields = HashMap::new();
    fields.insert("message".to_string(), Value::Text(message));
    Value::Record(fields)
}

fn runtime_error_to_http_error(err: RuntimeError) -> AiviHttpError {
    match err {
        RuntimeError::Error(value) => AiviHttpError {
            message: format_value(&value),
        },
        RuntimeError::Cancelled => AiviHttpError {
            message: "cancelled".to_string(),
        },
        RuntimeError::Message(message) => AiviHttpError { message },
    }
}

fn spawn_effect(
    id: usize,
    effect: Value,
    ctx: Arc<RuntimeContext>,
    cancel: Arc<CancelToken>,
    sender: mpsc::Sender<(usize, Result<Value, RuntimeError>)>,
) {
    std::thread::spawn(move || {
        let mut runtime = Runtime::new(ctx, cancel);
        let result = runtime.run_effect_value(effect);
        let _ = sender.send((id, result));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanups_run_even_when_cancelled() {
        let globals = Env::new(None);
        register_builtins(&globals);
        let ctx = Arc::new(RuntimeContext { globals });
        let cancel = CancelToken::root();
        let mut runtime = Runtime::new(ctx, cancel.clone());

        let ran = Arc::new(AtomicBool::new(false));
        let ran_clone = ran.clone();
        let cleanup = Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new(move |_| {
                ran_clone.store(true, Ordering::SeqCst);
                Ok(Value::Unit)
            }),
        }));

        cancel.cancel();
        assert!(runtime.run_cleanups(vec![cleanup]).is_ok());
        assert!(ran.load(Ordering::SeqCst));
    }

    #[test]
    fn text_interpolation_evaluates() {
        let source = r#"
module test.interpolation = {
  s = "Count: {1 + 2}"
  n = -1
  t = "negative{n}"
  u = "brace \{x\}"
}
"#;

        let (modules, diags) = crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

        let program = crate::hir::desugar_modules(&modules);
        let module = program.modules.into_iter().next().expect("expected module");

        let globals = Env::new(None);
        register_builtins(&globals);
        assert!(globals.get("println").is_some());

        let mut grouped: HashMap<String, Vec<HirExpr>> = HashMap::new();
        for def in module.defs {
            grouped.entry(def.name).or_default().push(def.expr);
        }
        for (name, exprs) in grouped {
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

        let ctx = Arc::new(RuntimeContext { globals });
        let cancel = CancelToken::root();
        let mut runtime = Runtime::new(ctx, cancel);

        let s = runtime.ctx.globals.get("s").unwrap();
        let t = runtime.ctx.globals.get("t").unwrap();
        let u = runtime.ctx.globals.get("u").unwrap();

        let s = match runtime.force_value(s) {
            Ok(Value::Text(value)) => value,
            Ok(_) => panic!("expected Text for s"),
            Err(_) => panic!("failed to evaluate s"),
        };
        let t = match runtime.force_value(t) {
            Ok(Value::Text(value)) => value,
            Ok(_) => panic!("expected Text for t"),
            Err(_) => panic!("failed to evaluate t"),
        };
        let u = match runtime.force_value(u) {
            Ok(Value::Text(value)) => value,
            Ok(_) => panic!("expected Text for u"),
            Err(_) => panic!("failed to evaluate u"),
        };

        assert_eq!(s, "Count: 3");
        assert_eq!(t, "negative-1");
        assert_eq!(u, "brace {x}");
    }

    #[test]
    fn concurrent_par_observes_parent_cancellation() {
        let globals = Env::new(None);
        register_builtins(&globals);
        let ctx = Arc::new(RuntimeContext { globals });
        let cancel = CancelToken::root();

        let (started_left_tx, started_left_rx) = mpsc::channel();
        let (started_right_tx, started_right_rx) = mpsc::channel();

        let left = Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new(move |runtime| {
                let _ = started_left_tx.send(());
                loop {
                    runtime.check_cancelled()?;
                    std::hint::spin_loop();
                }
            }),
        }));
        let right = Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new(move |runtime| {
                let _ = started_right_tx.send(());
                loop {
                    runtime.check_cancelled()?;
                    std::hint::spin_loop();
                }
            }),
        }));

        let (result_tx, result_rx) = mpsc::channel();
        let ctx_clone = ctx.clone();
        let cancel_clone = cancel.clone();
        std::thread::spawn(move || {
            let mut runtime = Runtime::new(ctx_clone, cancel_clone);
            let concurrent = build_concurrent_record();
            let Value::Record(fields) = concurrent else {
                panic!("expected concurrent record");
            };
            let par = fields.get("par").expect("par").clone();
            let applied = match runtime.apply(par, left) {
                Ok(value) => value,
                Err(_) => panic!("apply left failed"),
            };
            let applied = match runtime.apply(applied, right) {
                Ok(value) => value,
                Err(_) => panic!("apply right failed"),
            };
            let result = runtime.run_effect_value(applied);
            let _ = result_tx.send(result);
        });

        started_left_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("left started");
        started_right_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("right started");

        cancel.cancel();

        let result = result_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("par returned");
        assert!(matches!(result, Err(RuntimeError::Cancelled)));
    }
}
