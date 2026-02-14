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

fn is_match_failure_error(err: &RuntimeError) -> bool {
    matches!(err, RuntimeError::Message(message) if message == "non-exhaustive match")
}

fn seed_rng_state() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    nanos ^ 0x9E3779B97F4A7C15
}

pub fn values_equal(left: &Value, right: &Value) -> bool {
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
        Value::Bool(v) => serde_json::Value::String(v.to_string()),
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
        _ => debug_summary_json(value),
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

pub fn format_value(value: &Value) -> String {
    match value {
        Value::Unit => "Unit".to_string(),
        Value::Bool(v) => v.to_string(),
        Value::Int(v) => v.to_string(),
        Value::Float(v) => v.to_string(),
        Value::Text(v) => v.clone(),
        Value::DateTime(v) => v.clone(),
        Value::Bytes(v) => format!("{:?}", v.as_slice()),
        Value::Regex(v) => v.as_str().to_string(),
        Value::BigInt(v) => v.to_string(),
        Value::Rational(v) => v.to_string(),
        Value::Decimal(v) => v.to_string(),
        Value::Map(map) => format!("<map:{}>", map.len()),
        Value::Set(set) => format!("<set:{}>", set.len()),
        Value::Queue(queue) => format!("<queue:{}>", queue.len()),
        Value::Deque(deque) => format!("<deque:{}>", deque.len()),
        Value::Heap(heap) => format!("<heap:{}>", heap.len()),
        Value::List(items) => {
            let inner = items
                .iter()
                .map(format_value)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{}]", inner)
        }
        Value::Tuple(items) => {
            let inner = items
                .iter()
                .map(format_value)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({})", inner)
        }
        Value::Record(map) => {
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            let inner = keys
                .into_iter()
                .map(|k| format!("{}: {}", k, format_value(&map[&k])))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{}}}", inner)
        }
        Value::Constructor { name, args } => {
            if args.is_empty() {
                name.clone()
            } else {
                let inner = args.iter().map(format_value).collect::<Vec<_>>().join(", ");
                format!("{name}({inner})")
            }
        }
        Value::Closure(_) => "<closure>".to_string(),
        Value::Builtin(b) => format!("<builtin {}>", b.imp.name),
        Value::Effect(_) => "<effect>".to_string(),
        Value::Resource(_) => "<resource>".to_string(),
        Value::Thunk(_) => "<thunk>".to_string(),
        Value::MultiClause(_) => "<multiclause>".to_string(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    Unit,
    True,
    False,
    None,
    Some,
    Ok,
    Err,
    Closed,
    Pure,
    Fail,
    Attempt,
    Load,
    Bind,
    Print,
    Println,
    Map,
    Chain,
    AssertEq,
    File,
    System,
    Clock,
    Random,
    Channel,
    Concurrent,
    HttpServer,
    Text,
    Regex,
    Math,
    Calendar,
    Color,
    Linalg,
    Signal,
    Graph,
    Bigint,
    Rational,
    Decimal,
    Url,
    Http,
    Https,
    Sockets,
    Streams,
    Collections,
    Console,
    Crypto,
    Logger,
    Database,
    MapType,
    SetType,
    QueueType,
    DequeType,
    HeapType,
}
