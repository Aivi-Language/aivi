use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc, Mutex};

use rudo_gc::{GcMutex, Trace, Visitor};

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

unsafe impl Trace for Value {
    fn trace(&self, visitor: &mut impl Visitor) {
        match self {
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
            | Value::ChannelSend(_)
            | Value::ChannelRecv(_)
            | Value::FileHandle(_)
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
