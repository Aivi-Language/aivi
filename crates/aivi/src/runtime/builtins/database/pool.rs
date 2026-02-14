use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use super::util::{expect_int, make_err, make_ok};

#[derive(Clone, Copy, Debug)]
enum QueuePolicy {
    Fifo,
    Lifo,
}

#[derive(Clone, Copy, Debug)]
enum BackoffPolicy {
    Fixed(Duration),
    Exponential { base: Duration, max: Duration },
}

#[derive(Clone)]
struct ConnEntry {
    conn: Value,
    last_used_at: Instant,
    last_checked_at: Instant,
}

struct PoolState {
    closed: bool,

    max_size: usize,
    acquire_timeout: Duration,
    idle_timeout: Option<Duration>,
    health_check_interval: Option<Duration>,
    backoff_policy: BackoffPolicy,
    queue_policy: QueuePolicy,

    creating: usize,
    in_use: usize,
    waiters: usize,

    idle: Vec<ConnEntry>,

    acquire_fn: Value,
    release_fn: Value,
    health_check_fn: Value,
}

struct PoolInner {
    state: Mutex<PoolState>,
    cvar: Condvar,
}

fn pool_error(name: &str) -> Value {
    Value::Constructor {
        name: name.to_string(),
        args: Vec::new(),
    }
}

fn pool_error_invalid_config(message: String) -> Value {
    Value::Constructor {
        name: "InvalidConfig".to_string(),
        args: vec![Value::Text(message)],
    }
}

fn result_ok(value: Value) -> Value {
    make_ok(value)
}

fn result_err(value: Value) -> Value {
    make_err(value)
}

fn span_millis(value: Value, ctx: &str) -> Result<i64, RuntimeError> {
    let fields = expect_record(value, ctx)?;
    let millis = fields
        .get("millis")
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Span.millis")))?;
    expect_int(millis.clone(), ctx)
}

fn option_span_millis(value: Value, ctx: &str) -> Result<Option<i64>, RuntimeError> {
    match value {
        Value::Constructor { name, args } if name == "None" && args.is_empty() => Ok(None),
        Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
            span_millis(args[0].clone(), ctx).map(Some)
        }
        other => Err(RuntimeError::Message(format!(
            "{ctx} expects Option Span, got {}",
            crate::runtime::format_value(&other)
        ))),
    }
}

fn backoff_policy(value: Value, ctx: &str) -> Result<BackoffPolicy, RuntimeError> {
    match value {
        Value::Constructor { name, args } if name == "Fixed" && args.len() == 1 => {
            let ms = span_millis(args[0].clone(), ctx)?;
            Ok(BackoffPolicy::Fixed(Duration::from_millis(ms.max(0) as u64)))
        }
        Value::Constructor { name, args } if name == "Exponential" && args.len() == 1 => {
            let rec = expect_record(args[0].clone(), ctx)?;
            let base = rec
                .get("base")
                .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Exponential.base")))?;
            let max = rec
                .get("max")
                .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Exponential.max")))?;
            let base_ms = span_millis(base.clone(), ctx)?;
            let max_ms = span_millis(max.clone(), ctx)?;
            Ok(BackoffPolicy::Exponential {
                base: Duration::from_millis(base_ms.max(0) as u64),
                max: Duration::from_millis(max_ms.max(0) as u64),
            })
        }
        other => Err(RuntimeError::Message(format!(
            "{ctx} expects BackoffPolicy (Fixed|Exponential), got {}",
            crate::runtime::format_value(&other)
        ))),
    }
}

fn queue_policy(value: Value, ctx: &str) -> Result<QueuePolicy, RuntimeError> {
    match value {
        Value::Constructor { name, args } if args.is_empty() => match name.as_str() {
            "Fifo" => Ok(QueuePolicy::Fifo),
            "Lifo" => Ok(QueuePolicy::Lifo),
            _ => Err(RuntimeError::Message(format!(
                "{ctx} expects QueuePolicy (Fifo|Lifo), got {name}"
            ))),
        },
        other => Err(RuntimeError::Message(format!(
            "{ctx} expects QueuePolicy, got {}",
            crate::runtime::format_value(&other)
        ))),
    }
}

fn stats_value(state: &PoolState) -> Value {
    let size = state.idle.len() + state.in_use + state.creating;
    let mut fields = std::collections::HashMap::new();
    fields.insert("size".to_string(), Value::Int(size as i64));
    fields.insert("idle".to_string(), Value::Int(state.idle.len() as i64));
    fields.insert("inUse".to_string(), Value::Int(state.in_use as i64));
    fields.insert("waiters".to_string(), Value::Int(state.waiters as i64));
    fields.insert("closed".to_string(), Value::Bool(state.closed));
    Value::Record(std::sync::Arc::new(fields))
}

fn acquire_effect(acquire_fn: &Value, runtime: &mut Runtime) -> Result<Value, RuntimeError> {
    let applied = runtime.apply(acquire_fn.clone(), Value::Unit)?;
    runtime.run_effect_value(applied)
}

fn release_effect(release_fn: &Value, conn: Value, runtime: &mut Runtime) -> Result<(), RuntimeError> {
    let applied = runtime.apply(release_fn.clone(), conn)?;
    let _ = runtime.run_effect_value(applied)?;
    Ok(())
}

fn health_check_effect(
    health_fn: &Value,
    conn: Value,
    runtime: &mut Runtime,
) -> Result<bool, RuntimeError> {
    let applied = runtime.apply(health_fn.clone(), conn)?;
    let value = runtime.run_effect_value(applied)?;
    match value {
        Value::Bool(b) => Ok(b),
        other => Err(RuntimeError::Message(format!(
            "database.pool healthCheck expects Bool, got {}",
            crate::runtime::format_value(&other)
        ))),
    }
}

fn backoff_wait(policy: BackoffPolicy, attempt: usize) -> Duration {
    match policy {
        BackoffPolicy::Fixed(d) => d,
        BackoffPolicy::Exponential { base, max } => {
            let factor = 1u64 << (attempt.min(16) as u32);
            let dur = base.saturating_mul(factor as u32);
            std::cmp::min(dur, max)
        }
    }
}

fn pop_idle(state: &mut PoolState) -> Option<ConnEntry> {
    match state.queue_policy {
        QueuePolicy::Fifo => {
            if state.idle.is_empty() {
                None
            } else {
                Some(state.idle.remove(0))
            }
        }
        QueuePolicy::Lifo => state.idle.pop(),
    }
}

fn acquire_impl(inner: &std::sync::Arc<PoolInner>, runtime: &mut Runtime) -> Result<Value, RuntimeError> {
    let start = Instant::now();
    let mut attempt = 0usize;

    loop {
        runtime.check_cancelled()?;
        let now = Instant::now();

        // Retire expired idle connections lazily.
        let expired: Vec<Value> = {
            let mut guard = inner
                .state
                .lock()
                .map_err(|_| RuntimeError::Message("pool poisoned".to_string()))?;
            let mut expired = Vec::new();
            if let Some(timeout) = guard.idle_timeout {
                guard.idle.retain(|entry| {
                    if now.duration_since(entry.last_used_at) > timeout {
                        expired.push(entry.conn.clone());
                        false
                    } else {
                        true
                    }
                });
            }
            expired
        };
        if !expired.is_empty() {
            let release_fn = {
                let guard = inner
                    .state
                    .lock()
                    .map_err(|_| RuntimeError::Message("pool poisoned".to_string()))?;
                guard.release_fn.clone()
            };
            for conn in expired {
                release_effect(&release_fn, conn, runtime)?;
            }
        }

        // Try idle.
        if let Some(mut entry) = {
            let mut guard = inner
                .state
                .lock()
                .map_err(|_| RuntimeError::Message("pool poisoned".to_string()))?;
            if guard.closed {
                return Ok(result_err(pool_error("Closed")));
            }
            let entry = pop_idle(&mut guard);
            if entry.is_some() {
                guard.in_use += 1;
            }
            entry
        } {
            let should_check = {
                let guard = inner
                    .state
                    .lock()
                    .map_err(|_| RuntimeError::Message("pool poisoned".to_string()))?;
                match guard.health_check_interval {
                    None => false,
                    Some(interval) => now.duration_since(entry.last_checked_at) >= interval,
                }
            };
            if should_check {
                let (health_fn, release_fn) = {
                    let guard = inner
                        .state
                        .lock()
                        .map_err(|_| RuntimeError::Message("pool poisoned".to_string()))?;
                    (guard.health_check_fn.clone(), guard.release_fn.clone())
                };
                entry.last_checked_at = now;
                if !health_check_effect(&health_fn, entry.conn.clone(), runtime)? {
                    // Retire unhealthy and try again.
                    release_effect(&release_fn, entry.conn, runtime)?;
                    let mut guard = inner
                        .state
                        .lock()
                        .map_err(|_| RuntimeError::Message("pool poisoned".to_string()))?;
                    guard.in_use = guard.in_use.saturating_sub(1);
                    inner.cvar.notify_one();
                    if now.duration_since(start) >= guard.acquire_timeout {
                        return Ok(result_err(pool_error("HealthFailed")));
                    }
                    attempt += 1;
                    continue;
                }
            }

            entry.last_used_at = now;
            return Ok(result_ok(entry.conn));
        }

        // No idle; try create if room.
        let (acquire_fn, release_fn, health_fn, timeout, backoff, can_create) = {
            let mut guard = inner
                .state
                .lock()
                .map_err(|_| RuntimeError::Message("pool poisoned".to_string()))?;
            if guard.closed {
                return Ok(result_err(pool_error("Closed")));
            }
            let size = guard.idle.len() + guard.in_use + guard.creating;
            if size < guard.max_size {
                guard.creating += 1;
                (
                    guard.acquire_fn.clone(),
                    guard.release_fn.clone(),
                    guard.health_check_fn.clone(),
                    guard.acquire_timeout,
                    guard.backoff_policy,
                    true,
                )
            } else {
                (
                    Value::Unit,
                    Value::Unit,
                    Value::Unit,
                    guard.acquire_timeout,
                    guard.backoff_policy,
                    false,
                )
            }
        };

        if can_create {
            let conn = match acquire_effect(&acquire_fn, runtime) {
                Ok(conn) => conn,
                Err(err) => {
                    let mut guard = inner
                        .state
                        .lock()
                        .map_err(|_| RuntimeError::Message("pool poisoned".to_string()))?;
                    guard.creating = guard.creating.saturating_sub(1);
                    inner.cvar.notify_one();
                    return Err(err);
                }
            };
            let healthy = health_check_effect(&health_fn, conn.clone(), runtime)?;
            {
                let mut guard = inner
                    .state
                    .lock()
                    .map_err(|_| RuntimeError::Message("pool poisoned".to_string()))?;
                guard.creating = guard.creating.saturating_sub(1);
                if healthy {
                    guard.in_use += 1;
                    inner.cvar.notify_one();
                    return Ok(result_ok(conn));
                }
                inner.cvar.notify_one();
            }
            let _ = release_effect(&release_fn, conn, runtime);
            if Instant::now().duration_since(start) >= timeout {
                return Ok(result_err(pool_error("HealthFailed")));
            }
            attempt += 1;
            continue;
        }

        // Full: wait.
        let (timeout, wait_step) = {
            let guard = inner
                .state
                .lock()
                .map_err(|_| RuntimeError::Message("pool poisoned".to_string()))?;
            let elapsed = now.duration_since(start);
            if elapsed >= guard.acquire_timeout {
                (true, Duration::from_millis(0))
            } else {
                let remaining = guard.acquire_timeout - elapsed;
                let step = std::cmp::min(
                    remaining,
                    std::cmp::max(Duration::from_millis(10), backoff_wait(backoff, attempt)),
                );
                (false, step)
            }
        };
        if timeout {
            return Ok(result_err(pool_error("Timeout")));
        }

        {
            let mut guard = inner
                .state
                .lock()
                .map_err(|_| RuntimeError::Message("pool poisoned".to_string()))?;
            guard.waiters += 1;
            let (mut guard, _) = inner
                .cvar
                .wait_timeout(guard, wait_step)
                .map_err(|_| RuntimeError::Message("pool poisoned".to_string()))?;
            guard.waiters = guard.waiters.saturating_sub(1);
        }
        attempt += 1;
    }
}

fn release_impl(inner: &std::sync::Arc<PoolInner>, conn: Value, runtime: &mut Runtime) -> Result<Value, RuntimeError> {
    let now = Instant::now();
    let (release_fn, should_drop) = {
        let mut guard = inner
            .state
            .lock()
            .map_err(|_| RuntimeError::Message("pool poisoned".to_string()))?;
        guard.in_use = guard.in_use.saturating_sub(1);
        let should_drop = guard.closed;
        if !should_drop {
            guard.idle.push(ConnEntry {
                conn: conn.clone(),
                last_used_at: now,
                last_checked_at: now,
            });
        }
        let release_fn = guard.release_fn.clone();
        inner.cvar.notify_one();
        (release_fn, should_drop)
    };

    if should_drop {
        release_effect(&release_fn, conn, runtime)?;
    }
    Ok(Value::Unit)
}

fn close_impl(inner: &std::sync::Arc<PoolInner>, runtime: &mut Runtime) -> Result<Value, RuntimeError> {
    let (release_fn, idle) = {
        let mut guard = inner
            .state
            .lock()
            .map_err(|_| RuntimeError::Message("pool poisoned".to_string()))?;
        if guard.closed {
            return Ok(Value::Unit);
        }
        guard.closed = true;
        let release_fn = guard.release_fn.clone();
        let idle = guard.idle.drain(..).map(|e| e.conn).collect::<Vec<_>>();
        inner.cvar.notify_all();
        (release_fn, idle)
    };
    for conn in idle {
        release_effect(&release_fn, conn, runtime)?;
    }
    Ok(Value::Unit)
}

fn drain_impl(inner: &std::sync::Arc<PoolInner>, runtime: &mut Runtime) -> Result<Value, RuntimeError> {
    loop {
        runtime.check_cancelled()?;
        let (release_fn, idle, done) = {
            let mut guard = inner
                .state
                .lock()
                .map_err(|_| RuntimeError::Message("pool poisoned".to_string()))?;
            if guard.in_use == 0 && guard.creating == 0 {
                let release_fn = guard.release_fn.clone();
                let idle = guard.idle.drain(..).map(|e| e.conn).collect::<Vec<_>>();
                (release_fn, idle, true)
            } else {
                let (guard, _) = inner
                    .cvar
                    .wait_timeout(guard, Duration::from_millis(25))
                    .map_err(|_| RuntimeError::Message("pool poisoned".to_string()))?;
                drop(guard);
                (Value::Unit, Vec::new(), false)
            }
        };
        if done {
            for conn in idle {
                release_effect(&release_fn, conn, runtime)?;
            }
            return Ok(Value::Unit);
        }
    }
}

fn with_conn_impl(
    inner: &std::sync::Arc<PoolInner>,
    run: Value,
    runtime: &mut Runtime,
) -> Result<Value, RuntimeError> {
    // Acquire a connection; pool-level errors are returned as `Err PoolError`,
    // while user `DbError` failures are propagated as effect failures.
    let acquired = acquire_impl(inner, runtime)?;
    let (name, args) = match &acquired {
        Value::Constructor { name, args } => (name.as_str(), args.as_slice()),
        other => {
            return Err(RuntimeError::Message(format!(
                "database.pool.withConn: unexpected acquire result {}",
                crate::runtime::format_value(other)
            )))
        }
    };
    if name == "Err" {
        return Ok(acquired);
    }
    if name != "Ok" || args.len() != 1 {
        return Err(RuntimeError::Message(format!(
            "database.pool.withConn: unexpected acquire result {}",
            crate::runtime::format_value(&acquired)
        )));
    }

    let conn = args[0].clone();
    let run_result = (|| {
        let applied = runtime.apply(run, conn.clone())?;
        runtime.run_effect_value(applied)
    })();

    // Always release, even if the user action failed or we were cancelled.
    let release_result =
        runtime.uncancelable(|runtime| release_impl(inner, conn.clone(), runtime));

    match (run_result, release_result) {
        (Ok(value), Ok(_)) => Ok(result_ok(value)),
        (Ok(_), Err(err)) => Err(err),
        (Err(err), Ok(_)) => Err(err),
        (Err(err), Err(_)) => Err(err),
    }
}

fn make_pool_record(inner: std::sync::Arc<PoolInner>) -> Value {
    let mut fields = std::collections::HashMap::new();

    {
        let inner = inner.clone();
        fields.insert(
            "acquire".to_string(),
            builtin("database.pool.acquire", 1, move |_args, _| {
                let effect = EffectValue::Thunk {
                    func: std::sync::Arc::new({
                        let inner = inner.clone();
                        move |runtime| acquire_impl(&inner, runtime)
                    }),
                };
                Ok(Value::Effect(std::sync::Arc::new(effect)))
            }),
        );
    }

    {
        let inner = inner.clone();
        fields.insert(
            "release".to_string(),
            builtin("database.pool.release", 1, move |mut args, _| {
                let conn = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: std::sync::Arc::new({
                        let inner = inner.clone();
                        move |runtime| release_impl(&inner, conn.clone(), runtime)
                    }),
                };
                Ok(Value::Effect(std::sync::Arc::new(effect)))
            }),
        );
    }

    {
        let inner = inner.clone();
        fields.insert(
            "stats".to_string(),
            builtin("database.pool.stats", 1, move |_args, _| {
                let effect = EffectValue::Thunk {
                    func: std::sync::Arc::new({
                        let inner = inner.clone();
                        move |_| {
                            let guard = inner
                                .state
                                .lock()
                                .map_err(|_| RuntimeError::Message("pool poisoned".to_string()))?;
                            Ok(stats_value(&guard))
                        }
                    }),
                };
                Ok(Value::Effect(std::sync::Arc::new(effect)))
            }),
        );
    }

    {
        let inner = inner.clone();
        fields.insert(
            "close".to_string(),
            builtin("database.pool.close", 1, move |_args, _| {
                let effect = EffectValue::Thunk {
                    func: std::sync::Arc::new({
                        let inner = inner.clone();
                        move |runtime| close_impl(&inner, runtime)
                    }),
                };
                Ok(Value::Effect(std::sync::Arc::new(effect)))
            }),
        );
    }

    {
        let inner = inner.clone();
        fields.insert(
            "drain".to_string(),
            builtin("database.pool.drain", 1, move |_args, _| {
                let effect = EffectValue::Thunk {
                    func: std::sync::Arc::new({
                        let inner = inner.clone();
                        move |runtime| drain_impl(&inner, runtime)
                    }),
                };
                Ok(Value::Effect(std::sync::Arc::new(effect)))
            }),
        );
    }

    {
        let inner = inner.clone();
        fields.insert(
            "withConn".to_string(),
            builtin("database.pool.withConn", 1, move |mut args, _| {
                let run = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: std::sync::Arc::new({
                        let inner = inner.clone();
                        move |runtime| with_conn_impl(&inner, run.clone(), runtime)
                    }),
                };
                Ok(Value::Effect(std::sync::Arc::new(effect)))
            }),
        );
    }

    Value::Record(std::sync::Arc::new(fields))
}

pub(super) fn build_database_pool_record() -> Value {
    let mut fields = std::collections::HashMap::new();
    fields.insert(
        "create".to_string(),
        builtin("database.pool.create", 1, |mut args, _| {
            let config = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: std::sync::Arc::new(move |runtime| {
                    let cfg = expect_record(config.clone(), "database.pool.create")?;

                    let max_size = cfg
                        .get("maxSize")
                        .ok_or_else(|| RuntimeError::Message("pool.create expects maxSize".to_string()))?
                        .clone();
                    let min_idle = cfg
                        .get("minIdle")
                        .ok_or_else(|| RuntimeError::Message("pool.create expects minIdle".to_string()))?
                        .clone();
                    let acquire_timeout = cfg
                        .get("acquireTimeout")
                        .ok_or_else(|| RuntimeError::Message("pool.create expects acquireTimeout".to_string()))?
                        .clone();

                    let idle_timeout = cfg
                        .get("idleTimeout")
                        .ok_or_else(|| RuntimeError::Message("pool.create expects idleTimeout".to_string()))?
                        .clone();
                    let health_interval = cfg
                        .get("healthCheckInterval")
                        .ok_or_else(|| RuntimeError::Message("pool.create expects healthCheckInterval".to_string()))?
                        .clone();
                    let backoff = cfg
                        .get("backoffPolicy")
                        .ok_or_else(|| RuntimeError::Message("pool.create expects backoffPolicy".to_string()))?
                        .clone();
                    let queue = cfg
                        .get("queuePolicy")
                        .ok_or_else(|| RuntimeError::Message("pool.create expects queuePolicy".to_string()))?
                        .clone();

                    let acquire_fn = cfg
                        .get("acquire")
                        .ok_or_else(|| RuntimeError::Message("pool.create expects acquire".to_string()))?
                        .clone();
                    let release_fn = cfg
                        .get("release")
                        .ok_or_else(|| RuntimeError::Message("pool.create expects release".to_string()))?
                        .clone();
                    let health_fn = cfg
                        .get("healthCheck")
                        .ok_or_else(|| RuntimeError::Message("pool.create expects healthCheck".to_string()))?
                        .clone();

                    let max_size = expect_int(max_size, "pool.create.maxSize")?;
                    let min_idle = expect_int(min_idle, "pool.create.minIdle")?;
                    if max_size <= 0 {
                        return Ok(result_err(pool_error_invalid_config(
                            "maxSize must be > 0".to_string(),
                        )));
                    }
                    if min_idle < 0 {
                        return Ok(result_err(pool_error_invalid_config(
                            "minIdle must be >= 0".to_string(),
                        )));
                    }
                    if min_idle > max_size {
                        return Ok(result_err(pool_error_invalid_config(
                            "minIdle must be <= maxSize".to_string(),
                        )));
                    }
                    let timeout_ms = span_millis(acquire_timeout, "pool.create.acquireTimeout")?;
                    if timeout_ms < 0 {
                        return Ok(result_err(pool_error_invalid_config(
                            "acquireTimeout must be >= 0".to_string(),
                        )));
                    }

                    let idle_timeout_ms =
                        option_span_millis(idle_timeout, "pool.create.idleTimeout")?;
                    let health_interval_ms =
                        option_span_millis(health_interval, "pool.create.healthCheckInterval")?;

                    let backoff_policy = backoff_policy(backoff, "pool.create.backoffPolicy")?;
                    let queue_policy = queue_policy(queue, "pool.create.queuePolicy")?;

                    let inner = std::sync::Arc::new(PoolInner {
                        state: Mutex::new(PoolState {
                            closed: false,
                            max_size: max_size as usize,
                            acquire_timeout: Duration::from_millis(timeout_ms as u64),
                            idle_timeout: idle_timeout_ms
                                .map(|ms| Duration::from_millis(ms.max(0) as u64)),
                            health_check_interval: health_interval_ms
                                .map(|ms| Duration::from_millis(ms.max(0) as u64)),
                            backoff_policy,
                            queue_policy,
                            creating: 0,
                            in_use: 0,
                            waiters: 0,
                            idle: Vec::new(),
                            acquire_fn,
                            release_fn,
                            health_check_fn: health_fn,
                        }),
                        cvar: Condvar::new(),
                    });

                    // Eagerly create `min_idle` connections and release them into idle.
                    for _ in 0..(min_idle as usize) {
                        let acquired = acquire_impl(&inner, runtime)?;
                        match acquired {
                            Value::Constructor { name, args } if name == "Ok" && args.len() == 1 => {
                                let conn = args[0].clone();
                                let _ = release_impl(&inner, conn, runtime)?;
                            }
                            Value::Constructor { name, args } if name == "Err" && args.len() == 1 => {
                                return Ok(Value::Constructor { name: "Err".to_string(), args });
                            }
                            other => {
                                return Err(RuntimeError::Message(format!(
                                    "pool.create: unexpected acquire result {}",
                                    crate::runtime::format_value(&other)
                                )))
                            }
                        }
                    }

                    Ok(result_ok(make_pool_record(inner)))
                }),
            };
            Ok(Value::Effect(std::sync::Arc::new(effect)))
        }),
    );
    Value::Record(std::sync::Arc::new(fields))
}
