use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc, Mutex};

use crate::hir::{HirBlockItem, HirExpr};

use super::{Env, Runtime, RuntimeError};

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
    List(Vec<Value>),
    Tuple(Vec<Value>),
    Record(HashMap<String, Value>),
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
    pub(super) cached: Mutex<Option<Value>>,
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
