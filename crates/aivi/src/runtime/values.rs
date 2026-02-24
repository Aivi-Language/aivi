use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc, Mutex, OnceLock};

use im::{HashMap as ImHashMap, HashSet as ImHashSet, Vector as ImVector};
use num_bigint::BigInt;
use num_rational::BigRational;
use regex::Regex;
use rust_decimal::Decimal;

use crate::hir::{HirBlockItem, HirExpr};
use aivi_http_server::{ServerHandle, WebSocketHandle};

use super::environment::Env;
use super::{Runtime, RuntimeError};

pub(crate) type BuiltinFunc =
    dyn Fn(Vec<Value>, &mut Runtime) -> Result<Value, RuntimeError> + Send + Sync;
pub(crate) type ThunkFunc = dyn Fn(&mut Runtime) -> Result<Value, RuntimeError> + Send + Sync;

#[derive(Clone)]
pub(crate) struct SourceValue {
    pub(crate) kind: String,
    pub(crate) effect: Arc<EffectValue>,
}

#[derive(Clone, Debug)]
pub(crate) struct RecordShape {
    fields: Arc<Vec<String>>,
    offsets: Arc<HashMap<String, usize>>,
}

#[derive(Clone)]
pub(crate) struct ShapedRecord {
    pub(crate) shape: Arc<RecordShape>,
    pub(crate) values: Arc<Vec<Value>>,
}

#[derive(Default)]
struct RecordShapeRegistry {
    by_fields: HashMap<Vec<String>, Arc<RecordShape>>,
}

static RECORD_SHAPES: OnceLock<Mutex<RecordShapeRegistry>> = OnceLock::new();

fn record_shape_registry() -> &'static Mutex<RecordShapeRegistry> {
    RECORD_SHAPES.get_or_init(|| Mutex::new(RecordShapeRegistry::default()))
}

fn intern_record_shape(mut fields: Vec<String>) -> Arc<RecordShape> {
    fields.sort();
    let mut registry = record_shape_registry()
        .lock()
        .expect("record shape registry lock poisoned");
    if let Some(shape) = registry.by_fields.get(&fields) {
        return shape.clone();
    }
    let offsets: HashMap<String, usize> = fields
        .iter()
        .enumerate()
        .map(|(idx, name)| (name.clone(), idx))
        .collect();
    let shape = Arc::new(RecordShape {
        fields: Arc::new(fields.clone()),
        offsets: Arc::new(offsets),
    });
    registry.by_fields.insert(fields, shape.clone());
    shape
}

pub(crate) fn shape_record(record: &HashMap<String, Value>) -> ShapedRecord {
    let mut names: Vec<String> = record.keys().cloned().collect();
    names.sort();
    let shape = intern_record_shape(names);
    let values = shape
        .fields
        .iter()
        .map(|field| record.get(field).cloned().unwrap_or(Value::Unit))
        .collect();
    ShapedRecord {
        shape,
        values: Arc::new(values),
    }
}

impl ShapedRecord {
    pub(crate) fn get(&self, name: &str) -> Option<&Value> {
        self.shape
            .offsets
            .get(name)
            .and_then(|idx| self.values.get(*idx))
    }

    pub(crate) fn has_field(&self, name: &str) -> bool {
        self.shape.offsets.contains_key(name)
    }
}

/// Transitional compact scalar container for future NaN-tagged values.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TaggedValue(u64);

#[allow(dead_code)]
impl TaggedValue {
    const TAG_INT: u64 = 0b01;
    const TAG_BOOL_FALSE: u64 = 0b10;
    const TAG_BOOL_TRUE: u64 = 0b11;

    pub(crate) fn from_int(value: i64) -> Self {
        Self(((value as u64) << 2) | Self::TAG_INT)
    }

    pub(crate) fn from_bool(value: bool) -> Self {
        if value {
            Self(Self::TAG_BOOL_TRUE)
        } else {
            Self(Self::TAG_BOOL_FALSE)
        }
    }

    pub(crate) fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Int(value) => Some(Self::from_int(*value)),
            Value::Bool(value) => Some(Self::from_bool(*value)),
            _ => None,
        }
    }

    pub(crate) fn to_value(self) -> Value {
        match self.0 {
            Self::TAG_BOOL_FALSE => Value::Bool(false),
            Self::TAG_BOOL_TRUE => Value::Bool(true),
            bits if bits & 0b11 == Self::TAG_INT => Value::Int((bits as i64) >> 2),
            _ => Value::Unit,
        }
    }
}

#[derive(Clone)]
pub(crate) enum Value {
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
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Unit => write!(f, "Unit"),
            Value::Bool(v) => f.debug_tuple("Bool").field(v).finish(),
            Value::Int(v) => f.debug_tuple("Int").field(v).finish(),
            Value::Float(v) => f.debug_tuple("Float").field(v).finish(),
            Value::Text(v) => f.debug_tuple("Text").field(v).finish(),
            Value::DateTime(v) => f.debug_tuple("DateTime").field(v).finish(),
            Value::Bytes(v) => f.debug_tuple("Bytes").field(v).finish(),
            Value::Regex(v) => f.debug_tuple("Regex").field(&v.as_str()).finish(),
            Value::BigInt(v) => f.debug_tuple("BigInt").field(v).finish(),
            Value::Rational(v) => f.debug_tuple("Rational").field(v).finish(),
            Value::Decimal(v) => f.debug_tuple("Decimal").field(v).finish(),
            Value::Map(v) => f.debug_tuple("Map").field(v).finish(),
            Value::Set(v) => f.debug_tuple("Set").field(v).finish(),
            Value::Queue(v) => f.debug_tuple("Queue").field(v).finish(),
            Value::Deque(v) => f.debug_tuple("Deque").field(v).finish(),
            Value::Heap(v) => f.debug_tuple("Heap").field(v).finish(),
            Value::List(v) => f.debug_tuple("List").field(v).finish(),
            Value::Tuple(v) => f.debug_tuple("Tuple").field(v).finish(),
            Value::Record(v) => f.debug_tuple("Record").field(v).finish(),
            Value::Constructor { name, args } => f
                .debug_struct("Constructor")
                .field("name", name)
                .field("args", args)
                .finish(),
            Value::Closure(_) => write!(f, "Closure(<fn>)"),
            Value::Builtin(_) => write!(f, "Builtin(<fn>)"),
            Value::Effect(_) => write!(f, "Effect(<thunk>)"),
            Value::Source(_) => write!(f, "Source(<stream>)"),
            Value::Resource(_) => write!(f, "Resource(<scope>)"),
            Value::Thunk(_) => write!(f, "Thunk(<lazy>)"),
            Value::MultiClause(v) => f.debug_tuple("MultiClause").field(v).finish(),
            Value::ChannelSend(_) => write!(f, "ChannelSend(<chan>)"),
            Value::ChannelRecv(_) => write!(f, "ChannelRecv(<chan>)"),
            Value::FileHandle(_) => write!(f, "FileHandle(<fd>)"),
            Value::Listener(_) => write!(f, "Listener(<tcp>)"),
            Value::Connection(_) => write!(f, "Connection(<tcp>)"),
            Value::Stream(_) => write!(f, "Stream(<stream>)"),
            Value::HttpServer(_) => write!(f, "HttpServer(<server>)"),
            Value::WebSocket(_) => write!(f, "WebSocket(<socket>)"),
        }
    }
}

#[derive(Clone)]
pub(crate) struct BuiltinValue {
    pub(crate) imp: Arc<BuiltinImpl>,
    pub(crate) args: Vec<Value>,
    pub(crate) tagged_args: Option<Vec<TaggedValue>>,
}

pub(crate) struct BuiltinImpl {
    pub(crate) name: String,
    pub(crate) arity: usize,
    pub(crate) func: Arc<BuiltinFunc>,
}

pub(crate) struct ClosureValue {
    pub(crate) param: String,
    pub(crate) body: Arc<HirExpr>,
    pub(crate) env: Env,
}

pub(crate) enum EffectValue {
    Block {
        env: Env,
        items: Arc<Vec<HirBlockItem>>,
    },
    Thunk {
        func: Arc<ThunkFunc>,
    },
}

pub(crate) struct ResourceValue {
    pub(crate) items: Arc<Vec<HirBlockItem>>,
}

pub(crate) struct ThunkValue {
    pub(crate) expr: Arc<HirExpr>,
    pub(crate) env: Env,
    pub(crate) cached: Mutex<Option<Value>>,
    pub(crate) in_progress: AtomicBool,
}

pub(crate) struct ChannelInner {
    pub(crate) sender: Mutex<Option<ChannelSender>>,
    pub(crate) receiver: Mutex<mpsc::Receiver<Value>>,
    pub(crate) closed: AtomicBool,
}

pub(crate) enum ChannelSender {
    Unbounded(mpsc::Sender<Value>),
    Bounded(mpsc::SyncSender<Value>),
}

pub(crate) struct ChannelSend {
    pub(crate) inner: Arc<ChannelInner>,
}

pub(crate) struct ChannelRecv {
    pub(crate) inner: Arc<ChannelInner>,
}

pub(crate) struct StreamHandle {
    pub(crate) state: Mutex<StreamState>,
}

pub(crate) enum StreamState {
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

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) enum KeyValue {
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
    Tuple(Vec<KeyValue>),
    Record(Vec<(String, KeyValue)>),
}

impl KeyValue {
    pub(crate) fn try_from_value(value: &Value) -> Option<Self> {
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
            Value::Tuple(items) => {
                let keys: Option<Vec<KeyValue>> =
                    items.iter().map(KeyValue::try_from_value).collect();
                keys.map(KeyValue::Tuple)
            }
            Value::Record(fields) => {
                let mut pairs: Vec<(String, KeyValue)> = fields
                    .iter()
                    .map(|(k, v)| KeyValue::try_from_value(v).map(|kv| (k.clone(), kv)))
                    .collect::<Option<Vec<_>>>()?;
                pairs.sort_by(|a, b| a.0.cmp(&b.0));
                Some(KeyValue::Record(pairs))
            }
            _ => None,
        }
    }

    pub(crate) fn to_value(&self) -> Value {
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
            KeyValue::Tuple(items) => {
                Value::Tuple(items.iter().map(KeyValue::to_value).collect())
            }
            KeyValue::Record(pairs) => {
                let fields: HashMap<String, Value> = pairs
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_value()))
                    .collect();
                Value::Record(Arc::new(fields))
            }
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
            Tuple(_) => 10,
            Record(_) => 11,
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
            (Tuple(a), Tuple(b)) => a.cmp(b),
            (Record(a), Record(b)) => a.cmp(b),
            _ => Ordering::Equal,
        }
    }
}

impl PartialOrd for KeyValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
