use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use im::{HashMap as ImHashMap, HashSet as ImHashSet, Vector as ImVector};
use num_bigint::BigInt;
use num_rational::BigRational;
use regex::Regex;
use rust_decimal::Decimal;

use aivi_http_server::{ServerHandle, WebSocketHandle};

pub type RuntimeError = String;
pub type R = Result<Value, RuntimeError>;

pub type BuiltinFunc = dyn Fn(Vec<Value>, &mut Runtime) -> R + Send + Sync;
pub type ThunkFunc = dyn Fn(&mut Runtime) -> R + Send + Sync;

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
    pub func: Arc<dyn Fn(Value, &mut Runtime) -> R + Send + Sync>,
}

pub enum EffectValue {
    Thunk { func: Arc<ThunkFunc> },
}

pub struct ResourceValue {
    pub items: Arc<Vec<()>>,
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

#[derive(Clone)]
pub struct RuntimeContext {}

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
}

pub struct Runtime {
    pub ctx: Arc<RuntimeContext>,
    pub cancel: Arc<CancelToken>,
    rng_state: u64,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            ctx: Arc::new(RuntimeContext {}),
            cancel: CancelToken::root(),
            rng_state: seed_rng_state(),
        }
    }

    pub fn with_cancel(ctx: Arc<RuntimeContext>, cancel: Arc<CancelToken>) -> Self {
        Self {
            ctx,
            cancel,
            rng_state: seed_rng_state(),
        }
    }

    pub fn check_cancelled(&self) -> Result<(), RuntimeError> {
        if self.cancel.is_cancelled() {
            return Err("execution cancelled".to_string());
        }
        Ok(())
    }

    pub fn apply(&mut self, func: Value, arg: Value) -> R {
        self.check_cancelled()?;
        match func {
            Value::Closure(closure) => (closure.func)(arg, self),
            Value::Builtin(builtin) => self.apply_builtin(builtin, arg),
            Value::MultiClause(_) => Err("multiclause call is not supported yet".to_string()),
            other => Err(format!("expected function, got {}", format_value(&other))),
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
            other => Err(format!("expected Effect, got {}", format_value(&other))),
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
            return Err(format!(
                "builtin {} expects {} args",
                builtin.imp.name, builtin.imp.arity
            ));
        }
        (builtin.imp.func)(args, self)
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
            let inner = items.iter().map(format_value).collect::<Vec<_>>().join(", ");
            format!("[{}]", inner)
        }
        Value::Tuple(items) => {
            let inner = items.iter().map(format_value).collect::<Vec<_>>().join(", ");
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
