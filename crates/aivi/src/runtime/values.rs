use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc, Mutex};

use im::{HashMap as ImHashMap, HashSet as ImHashSet, Vector as ImVector};
use num_bigint::BigInt;
use num_rational::BigRational;
use regex::Regex;
use rudo_gc::{GcMutex, Trace, Visitor};
use rust_decimal::Decimal;

use crate::hir::{HirBlockItem, HirExpr};
use aivi_http_server::{ServerHandle, WebSocketHandle};

use super::environment::Env;
use super::{Runtime, RuntimeError};

pub(super) type BuiltinFunc =
    dyn Fn(Vec<Value>, &mut Runtime) -> Result<Value, RuntimeError> + Send + Sync;
pub(super) type ThunkFunc = dyn Fn(&mut Runtime) -> Result<Value, RuntimeError> + Send + Sync;

#[derive(Clone)]
pub(super) enum Value {
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
pub(super) struct BuiltinValue {
    pub(super) imp: Arc<BuiltinImpl>,
    pub(super) args: Vec<Value>,
}

pub(super) struct BuiltinImpl {
    pub(super) name: String,
    pub(super) arity: usize,
    pub(super) func: Arc<BuiltinFunc>,
}

pub(super) struct ClosureValue {
    pub(super) param: String,
    pub(super) body: Arc<HirExpr>,
    pub(super) env: Env,
}

pub(super) enum EffectValue {
    Block {
        env: Env,
        items: Arc<Vec<HirBlockItem>>,
    },
    Thunk {
        func: Arc<ThunkFunc>,
    },
}

pub(super) struct ResourceValue {
    pub(super) items: Arc<Vec<HirBlockItem>>,
}

pub(super) struct ThunkValue {
    pub(super) expr: Arc<HirExpr>,
    pub(super) env: Env,
    pub(super) cached: GcMutex<Option<Value>>,
    pub(super) in_progress: AtomicBool,
}

pub(super) struct ChannelInner {
    pub(super) sender: Mutex<Option<mpsc::Sender<Value>>>,
    pub(super) receiver: Mutex<mpsc::Receiver<Value>>,
    pub(super) closed: AtomicBool,
}

pub(super) struct ChannelSend {
    pub(super) inner: Arc<ChannelInner>,
}

pub(super) struct ChannelRecv {
    pub(super) inner: Arc<ChannelInner>,
}

pub(super) struct StreamHandle {
    pub(super) state: Mutex<StreamState>,
}

pub(super) enum StreamState {
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
pub(super) enum KeyValue {
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
    pub(super) fn try_from_value(value: &Value) -> Option<Self> {
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

    pub(super) fn to_value(&self) -> Value {
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

unsafe impl Trace for Value {
    fn trace(&self, visitor: &mut impl Visitor) {
        match self {
            Value::Map(entries) => {
                for value in entries.values() {
                    value.trace(visitor);
                }
            }
            Value::Set(_) => {}
            Value::Queue(items) => {
                for value in items.iter() {
                    value.trace(visitor);
                }
            }
            Value::Deque(items) => {
                for value in items.iter() {
                    value.trace(visitor);
                }
            }
            Value::Heap(_) => {}
            Value::List(items) => items.trace(visitor),
            Value::Tuple(items) => items.trace(visitor),
            Value::Record(fields) => fields.trace(visitor),
            Value::Constructor { args, .. } => args.trace(visitor),
            Value::Closure(closure) => closure.trace(visitor),
            Value::Builtin(builtin) => builtin.trace(visitor),
            Value::Effect(effect) => effect.trace(visitor),
            Value::Resource(resource) => resource.trace(visitor),
            Value::Thunk(thunk) => thunk.trace(visitor),
            Value::MultiClause(clauses) => clauses.trace(visitor),
            Value::Unit
            | Value::Bool(_)
            | Value::Int(_)
            | Value::Float(_)
            | Value::Text(_)
            | Value::DateTime(_)
            | Value::Bytes(_)
            | Value::Regex(_)
            | Value::BigInt(_)
            | Value::Rational(_)
            | Value::Decimal(_)
            | Value::ChannelSend(_)
            | Value::ChannelRecv(_)
            | Value::FileHandle(_)
            | Value::Listener(_)
            | Value::Connection(_)
            | Value::Stream(_)
            | Value::HttpServer(_)
            | Value::WebSocket(_) => {}
        }
    }
}

unsafe impl Trace for BuiltinValue {
    fn trace(&self, visitor: &mut impl Visitor) {
        self.args.trace(visitor);
    }
}

unsafe impl Trace for ClosureValue {
    fn trace(&self, visitor: &mut impl Visitor) {
        self.env.trace(visitor);
    }
}

unsafe impl Trace for EffectValue {
    fn trace(&self, visitor: &mut impl Visitor) {
        match self {
            EffectValue::Block { env, .. } => env.trace(visitor),
            EffectValue::Thunk { .. } => {}
        }
    }
}

unsafe impl Trace for ResourceValue {
    fn trace(&self, _visitor: &mut impl Visitor) {}
}

unsafe impl Trace for ThunkValue {
    fn trace(&self, visitor: &mut impl Visitor) {
        self.env.trace(visitor);
        self.cached.trace(visitor);
    }
}
