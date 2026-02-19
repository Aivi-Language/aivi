use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use im::{HashMap as ImHashMap, HashSet as ImHashSet, Vector as ImVector};
use num_bigint::BigInt;
use num_rational::BigRational;
use regex::Regex;
use rust_decimal::Decimal;

use aivi_http_server::{ServerHandle, WebSocketHandle};

#[derive(Clone)]
pub enum RuntimeError {
    Error(Value),
    Cancelled,
    Message(String),
}

impl std::fmt::Debug for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::Cancelled => f.debug_tuple("Cancelled").finish(),
            RuntimeError::Message(message) => f.debug_tuple("Message").field(message).finish(),
            RuntimeError::Error(value) => {
                f.debug_tuple("Error").field(&format_value(value)).finish()
            }
        }
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::Cancelled => write!(f, "execution cancelled"),
            RuntimeError::Message(message) => write!(f, "{message}"),
            RuntimeError::Error(value) => write!(f, "runtime error: {}", format_value(value)),
        }
    }
}

impl std::error::Error for RuntimeError {}

impl From<String> for RuntimeError {
    fn from(value: String) -> Self {
        RuntimeError::Message(value)
    }
}

impl From<&str> for RuntimeError {
    fn from(value: &str) -> Self {
        RuntimeError::Message(value.to_string())
    }
}

pub type R = Result<Value, RuntimeError>;

pub type BuiltinFunc = dyn Fn(Vec<Value>, &mut Runtime) -> R + Send + Sync;
pub type ThunkFunc = dyn Fn(&mut Runtime) -> R + Send + Sync;
pub type ClosureFunc = dyn Fn(Value, &mut Runtime) -> R + Send + Sync;
pub type ResourceAcquireFunc =
    dyn FnOnce(&mut Runtime) -> Result<(Value, Value), RuntimeError> + Send;

#[derive(Clone)]
pub struct SourceValue {
    pub kind: String,
    pub effect: Arc<EffectValue>,
}

#[derive(Clone)]
pub enum Value {
    Unit,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    DateTime(String),
    Bytes(Arc<Vec<u8>>),
    Regex(Arc<Regex>),
    BigInt(Arc<BigInt>),
    Rational(Arc<BigRational>),
    Decimal(Decimal),
    Map(Arc<ImHashMap<KeyValue, Value>>),
    Set(Arc<ImHashSet<KeyValue>>),
    Queue(Arc<ImVector<Value>>),
    Deque(Arc<ImVector<Value>>),
    Heap(Arc<BinaryHeap<std::cmp::Reverse<KeyValue>>>),
    List(Arc<Vec<Value>>),
    Tuple(Vec<Value>),
    Record(Arc<HashMap<String, Value>>),
    Constructor { name: String, args: Vec<Value> },
    Closure(Arc<ClosureValue>),
    Builtin(BuiltinValue),
    Effect(Arc<EffectValue>),
    Source(Arc<SourceValue>),
    Resource(Arc<ResourceValue>),
    Thunk(Arc<ThunkValue>),
    MultiClause(Vec<Value>),
    ChannelSend(Arc<ChannelSend>),
    ChannelRecv(Arc<ChannelRecv>),
    FileHandle(Arc<Mutex<std::fs::File>>),
    Listener(Arc<TcpListener>),
    Connection(Arc<Mutex<TcpStream>>),
    Stream(Arc<StreamHandle>),
    HttpServer(Arc<ServerHandle>),
    WebSocket(Arc<WebSocketHandle>),
    MutableMap(Arc<Mutex<ImHashMap<KeyValue, Value>>>),
}

#[derive(Clone)]
pub struct BuiltinValue {
    pub imp: Arc<BuiltinImpl>,
    pub args: Vec<Value>,
}

pub struct BuiltinImpl {
    pub name: String,
    pub arity: usize,
    pub func: Arc<BuiltinFunc>,
}

#[derive(Clone)]
pub struct ClosureValue {
    pub func: Arc<ClosureFunc>,
}

pub enum EffectValue {
    Thunk { func: Arc<ThunkFunc> },
}

pub struct ResourceValue {
    pub acquire: Mutex<Option<Box<ResourceAcquireFunc>>>,
}

pub struct ThunkValue {
    pub thunk: Arc<dyn Fn(&mut Runtime) -> R + Send + Sync>,
    pub cached: Mutex<Option<Value>>,
    pub in_progress: AtomicBool,
}

pub struct ChannelInner {
    pub sender: Mutex<Option<mpsc::Sender<Value>>>,
    pub receiver: Mutex<mpsc::Receiver<Value>>,
    pub closed: AtomicBool,
}

pub struct ChannelSend {
    pub inner: Arc<ChannelInner>,
}

pub struct ChannelRecv {
    pub inner: Arc<ChannelInner>,
}

pub struct StreamHandle {
    pub state: Mutex<StreamState>,
}

pub enum StreamState {
    Socket {
        stream: Arc<Mutex<TcpStream>>,
        chunk_size: usize,
    },
    Chunks {
        source: Arc<StreamHandle>,
        size: usize,
        buffer: Vec<u8>,
    },
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub enum KeyValue {
    Unit,
    Bool(bool),
    Int(i64),
    Float(u64),
    Text(String),
    DateTime(String),
    Bytes(Arc<Vec<u8>>),
    BigInt(Arc<BigInt>),
    Rational(Arc<BigRational>),
    Decimal(Decimal),
}

impl KeyValue {
    pub fn try_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Unit => Some(KeyValue::Unit),
            Value::Bool(value) => Some(KeyValue::Bool(*value)),
            Value::Int(value) => Some(KeyValue::Int(*value)),
            Value::Float(value) => Some(KeyValue::Float(value.to_bits())),
            Value::Text(value) => Some(KeyValue::Text(value.clone())),
            Value::DateTime(value) => Some(KeyValue::DateTime(value.clone())),
            Value::Bytes(value) => Some(KeyValue::Bytes(value.clone())),
            Value::BigInt(value) => Some(KeyValue::BigInt(value.clone())),
            Value::Rational(value) => Some(KeyValue::Rational(value.clone())),
            Value::Decimal(value) => Some(KeyValue::Decimal(*value)),
            _ => None,
        }
    }

    pub fn to_value(&self) -> Value {
        match self {
            KeyValue::Unit => Value::Unit,
            KeyValue::Bool(value) => Value::Bool(*value),
            KeyValue::Int(value) => Value::Int(*value),
            KeyValue::Float(value) => Value::Float(f64::from_bits(*value)),
            KeyValue::Text(value) => Value::Text(value.clone()),
            KeyValue::DateTime(value) => Value::DateTime(value.clone()),
            KeyValue::Bytes(value) => Value::Bytes(value.clone()),
            KeyValue::BigInt(value) => Value::BigInt(value.clone()),
            KeyValue::Rational(value) => Value::Rational(value.clone()),
            KeyValue::Decimal(value) => Value::Decimal(*value),
        }
    }
}

impl Ord for KeyValue {
    fn cmp(&self, other: &Self) -> Ordering {
        use KeyValue::*;
        let tag = |value: &KeyValue| match value {
            Unit => 0,
            Bool(_) => 1,
            Int(_) => 2,
            Float(_) => 3,
            Text(_) => 4,
            DateTime(_) => 5,
            Bytes(_) => 6,
            BigInt(_) => 7,
            Rational(_) => 8,
            Decimal(_) => 9,
        };
        let tag_cmp = tag(self).cmp(&tag(other));
        if tag_cmp != Ordering::Equal {
            return tag_cmp;
        }
        match (self, other) {
            (Unit, Unit) => Ordering::Equal,
            (Bool(a), Bool(b)) => a.cmp(b),
            (Int(a), Int(b)) => a.cmp(b),
            (Float(a), Float(b)) => a.cmp(b),
            (Text(a), Text(b)) => a.cmp(b),
            (DateTime(a), DateTime(b)) => a.cmp(b),
            (Bytes(a), Bytes(b)) => a.as_slice().cmp(b.as_slice()),
            (BigInt(a), BigInt(b)) => a.cmp(b),
            (Rational(a), Rational(b)) => a.cmp(b),
            (Decimal(a), Decimal(b)) => a.cmp(b),
            _ => Ordering::Equal,
        }
    }
}

impl PartialOrd for KeyValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct RuntimeContext {
    debug_call_id: AtomicU64,
}

impl Default for RuntimeContext {
    fn default() -> Self {
        Self {
            debug_call_id: AtomicU64::new(1),
        }
    }
}

impl RuntimeContext {
    pub fn next_debug_call_id(&self) -> u64 {
        self.debug_call_id.fetch_add(1, AtomicOrdering::Relaxed)
    }
}

pub struct CancelToken {
    local: AtomicBool,
    parent: Option<Arc<CancelToken>>,
}

impl CancelToken {
    pub fn root() -> Arc<Self> {
        Arc::new(Self {
            local: AtomicBool::new(false),
            parent: None,
        })
    }

    pub fn child(parent: Arc<CancelToken>) -> Arc<Self> {
        Arc::new(Self {
            local: AtomicBool::new(false),
            parent: Some(parent),
        })
    }

    pub fn cancel(&self) {
        self.local.store(true, AtomicOrdering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        if self.local.load(AtomicOrdering::SeqCst) {
            return true;
        }
        self.parent
            .as_ref()
            .is_some_and(|parent| parent.is_cancelled())
    }

    pub fn parent(&self) -> Option<Arc<CancelToken>> {
        self.parent.clone()
    }
}

pub struct Runtime {
    pub ctx: Arc<RuntimeContext>,
    pub cancel: Arc<CancelToken>,
    cancel_mask: usize,
    rng_state: u64,
    debug_stack: Vec<DebugFrame>,
}

#[derive(Clone)]
struct DebugFrame {
    fn_name: String,
    call_id: u64,
    start: Option<Instant>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            ctx: Arc::new(RuntimeContext::default()),
            cancel: CancelToken::root(),
            cancel_mask: 0,
            rng_state: seed_rng_state(),
            debug_stack: Vec::new(),
        }
    }

    pub fn with_cancel(ctx: Arc<RuntimeContext>, cancel: Arc<CancelToken>) -> Self {
        Self {
            ctx,
            cancel,
            cancel_mask: 0,
            rng_state: seed_rng_state(),
            debug_stack: Vec::new(),
        }
    }

    pub fn check_cancelled(&self) -> Result<(), RuntimeError> {
        if self.cancel_mask > 0 {
            return Ok(());
        }
        if self.cancel.is_cancelled() {
            return Err(RuntimeError::Cancelled);
        }
        Ok(())
    }

    pub fn uncancelable<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        self.cancel_mask = self.cancel_mask.saturating_add(1);
        let out = f(self);
        self.cancel_mask = self.cancel_mask.saturating_sub(1);
        out
    }

    pub fn debug_fn_enter(
        &mut self,
        fn_name: &str,
        args: Option<Vec<Value>>,
        log_time: bool,
    ) {
        let call_id = self.ctx.next_debug_call_id();
        let start = log_time.then(Instant::now);
        self.debug_stack.push(DebugFrame {
            fn_name: fn_name.to_string(),
            call_id,
            start,
        });

        let ts = log_time.then(now_unix_ms);
        let mut enter = serde_json::Map::new();
        enter.insert(
            "kind".to_string(),
            serde_json::Value::String("fn.enter".to_string()),
        );
        enter.insert("fn".to_string(), serde_json::Value::String(fn_name.to_string()));
        enter.insert(
            "callId".to_string(),
            serde_json::Value::Number(serde_json::Number::from(call_id)),
        );
        if let Some(args) = args {
            enter.insert(
                "args".to_string(),
                serde_json::Value::Array(args.iter().map(|v| debug_value_to_json(v, 0)).collect()),
            );
        }
        if let Some(ts) = ts {
            enter.insert(
                "ts".to_string(),
                serde_json::Value::Number(serde_json::Number::from(ts)),
            );
        }
        emit_debug_event(serde_json::Value::Object(enter));
    }

    pub fn debug_fn_exit(&mut self, result: &R, log_return: bool, log_time: bool) {
        let Some(frame) = self.debug_stack.pop() else {
            return;
        };
        let dur_ms = if log_time {
            frame
                .start
                .map(|s| s.elapsed().as_millis() as u64)
                .unwrap_or(0)
        } else {
            0
        };

        let mut exit = serde_json::Map::new();
        exit.insert(
            "kind".to_string(),
            serde_json::Value::String("fn.exit".to_string()),
        );
        exit.insert("fn".to_string(), serde_json::Value::String(frame.fn_name));
        exit.insert(
            "callId".to_string(),
            serde_json::Value::Number(serde_json::Number::from(frame.call_id)),
        );
        if log_return {
            if let Ok(value) = result {
                exit.insert("ret".to_string(), debug_value_to_json(value, 0));
            }
        }
        if log_time {
            exit.insert(
                "durMs".to_string(),
                serde_json::Value::Number(serde_json::Number::from(dur_ms)),
            );
        }
        emit_debug_event(serde_json::Value::Object(exit));
    }

    pub fn debug_pipe_in(
        &self,
        pipe_id: u32,
        step: u32,
        label: &str,
        value: &Value,
        log_time: bool,
    ) {
        let Some(frame) = self.debug_stack.last() else {
            return;
        };
        let ts = log_time.then(now_unix_ms);
        let mut event = serde_json::Map::new();
        event.insert(
            "kind".to_string(),
            serde_json::Value::String("pipe.in".to_string()),
        );
        event.insert("fn".to_string(), serde_json::Value::String(frame.fn_name.clone()));
        event.insert(
            "callId".to_string(),
            serde_json::Value::Number(serde_json::Number::from(frame.call_id)),
        );
        event.insert(
            "pipeId".to_string(),
            serde_json::Value::Number(serde_json::Number::from(pipe_id)),
        );
        event.insert(
            "step".to_string(),
            serde_json::Value::Number(serde_json::Number::from(step)),
        );
        event.insert("label".to_string(), serde_json::Value::String(label.to_string()));
        event.insert("value".to_string(), debug_value_to_json(value, 0));
        if let Some(ts) = ts {
            event.insert(
                "ts".to_string(),
                serde_json::Value::Number(serde_json::Number::from(ts)),
            );
        }
        emit_debug_event(serde_json::Value::Object(event));
    }

    pub fn debug_pipe_out(
        &self,
        pipe_id: u32,
        step: u32,
        label: &str,
        value: &Value,
        step_start: Option<Instant>,
        log_time: bool,
    ) {
        let Some(frame) = self.debug_stack.last() else {
            return;
        };
        let dur_ms = if log_time {
            step_start
                .map(|s| s.elapsed().as_millis() as u64)
                .unwrap_or(0)
        } else {
            0
        };
        let shape = debug_shape_tag(value);

        let mut event = serde_json::Map::new();
        event.insert(
            "kind".to_string(),
            serde_json::Value::String("pipe.out".to_string()),
        );
        event.insert("fn".to_string(), serde_json::Value::String(frame.fn_name.clone()));
        event.insert(
            "callId".to_string(),
            serde_json::Value::Number(serde_json::Number::from(frame.call_id)),
        );
        event.insert(
            "pipeId".to_string(),
            serde_json::Value::Number(serde_json::Number::from(pipe_id)),
        );
        event.insert(
            "step".to_string(),
            serde_json::Value::Number(serde_json::Number::from(step)),
        );
        event.insert("label".to_string(), serde_json::Value::String(label.to_string()));
        event.insert("value".to_string(), debug_value_to_json(value, 0));
        if log_time {
            event.insert(
                "durMs".to_string(),
                serde_json::Value::Number(serde_json::Number::from(dur_ms)),
            );
        }
        if let Some(shape) = shape {
            event.insert("shape".to_string(), serde_json::Value::String(shape));
        }
        emit_debug_event(serde_json::Value::Object(event));
    }

    pub fn apply(&mut self, func: Value, arg: Value) -> R {
        self.check_cancelled()?;
        match func {
            Value::Closure(closure) => (closure.func)(arg, self),
            Value::Builtin(builtin) => self.apply_builtin(builtin, arg),
            Value::MultiClause(clauses) => self.apply_multi_clause(clauses, arg),
            Value::Constructor { name, mut args } => {
                args.push(arg);
                Ok(Value::Constructor { name, args })
            }
            other => Err(RuntimeError::Message(format!(
                "expected function, got {}",
                format_value(&other)
            ))),
        }
    }

    pub fn call(&mut self, func: Value, args: Vec<Value>) -> R {
        let mut f = func;
        for arg in args {
            f = self.apply(f, arg)?;
        }
        Ok(f)
    }

    pub fn run_effect_value(&mut self, value: Value) -> R {
        self.check_cancelled()?;
        match value {
            Value::Effect(effect) => match effect.as_ref() {
                EffectValue::Thunk { func } => func(self),
            },
            // Sources can be executed anywhere an `Effect` is expected; `load` just makes this explicit.
            Value::Source(source) => match source.effect.as_ref() {
                EffectValue::Thunk { func } => func(self),
            },
            other => Err(RuntimeError::Message(format!(
                "expected Effect, got {}",
                format_value(&other)
            ))),
        }
    }

    pub fn acquire_resource(
        &mut self,
        resource: Arc<ResourceValue>,
    ) -> Result<(Value, Value), RuntimeError> {
        let mut guard = resource.acquire.lock().expect("resource acquire lock");
        let Some(acquire_fn) = guard.take() else {
            return Err(RuntimeError::Message(
                "resource already acquired".to_string(),
            ));
        };
        drop(guard);
        acquire_fn(self)
    }

    pub fn generator_to_vec(&mut self, gen: Value) -> Result<Vec<Value>, RuntimeError> {
        let step = Value::Builtin(BuiltinValue {
            imp: Arc::new(BuiltinImpl {
                name: "<gen_to_list_step>".to_string(),
                arity: 2,
                func: Arc::new(|mut args, _runtime| {
                    let x = args.pop().unwrap();
                    let acc = args.pop().unwrap();
                    let mut list = match acc {
                        Value::List(items) => (*items).clone(),
                        other => {
                            return Err(RuntimeError::Message(format!(
                                "generator_to_vec expects List accumulator, got {}",
                                format_value(&other)
                            )))
                        }
                    };
                    list.push(x);
                    Ok(Value::List(Arc::new(list)))
                }),
            }),
            args: Vec::new(),
        });
        let z = Value::List(Arc::new(Vec::new()));
        let with_step = self.apply(gen, step)?;
        let result = self.apply(with_step, z)?;
        match result {
            Value::List(items) => Ok((*items).clone()),
            other => Err(RuntimeError::Message(format!(
                "generator_to_vec expects generator to fold to List, got {}",
                format_value(&other)
            ))),
        }
    }

    fn apply_builtin(&mut self, builtin: BuiltinValue, arg: Value) -> R {
        let mut args = builtin.args.clone();
        args.push(arg);
        if args.len() < builtin.imp.arity {
            return Ok(Value::Builtin(BuiltinValue {
                imp: builtin.imp,
                args,
            }));
        }
        if args.len() > builtin.imp.arity {
            return Err(RuntimeError::Message(format!(
                "builtin {} expects {} args",
                builtin.imp.name, builtin.imp.arity
            )));
        }
        (builtin.imp.func)(args, self)
    }

    fn apply_multi_clause(&mut self, clauses: Vec<Value>, arg: Value) -> R {
        let mut results = Vec::new();
        let mut match_failures = 0usize;
        let mut last_error: Option<RuntimeError> = None;
        for clause in clauses {
            match self.apply(clause.clone(), arg.clone()) {
                Ok(value) => results.push(value),
                Err(err) if is_match_failure_error(&err) => {
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

    pub fn rng_next_u64(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.rng_state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng_state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.rng_next_u64()
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

fn is_callable(value: &Value) -> bool {
    matches!(
        value,
        Value::Closure(_) | Value::Builtin(_) | Value::MultiClause(_) | Value::Constructor { .. }
    )
}
