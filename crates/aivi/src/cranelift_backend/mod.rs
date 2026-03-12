mod abi;
mod compile;
mod inline;
mod jit_module;
pub(crate) mod lower;
mod object_module;
mod runtime_helpers;
pub(crate) mod use_analysis;

pub use compile::compile_to_object;
pub use compile::evaluate_binding_jit;
pub use compile::evaluate_binding_jit_detailed;
pub use compile::run_cranelift_jit;
pub(crate) use compile::run_cranelift_jit_cancellable;
pub use compile::run_test_suite_jit;
pub use compile::EvaluatedBinding;
pub use compile::ReplJitSession;

pub fn init_aot_runtime(program: crate::hir::HirProgram) -> usize {
    let ptr = Box::into_raw(Box::new(program));
    runtime_helpers::aivi_rt_init(ptr) as usize
}

/// Create a minimal AOT runtime with only builtins (no user program needed).
/// AOT-compiled functions are registered by `__aivi_main` at entry.
pub fn init_aot_runtime_base() -> usize {
    runtime_helpers::aivi_rt_init_base() as usize
}

pub fn destroy_aot_runtime(ctx: usize) {
    if ctx != 0 {
        runtime_helpers::aivi_rt_destroy(ctx as *mut abi::JitRuntimeCtx);
    }
}

/// Register crate-native bridge functions on an AOT runtime context.
///
/// The `register_fn` callback receives a mutable reference to a registration
/// helper. Call `reg.add(name, arity, func)` for each bridge function.
///
/// This is used by the auto-generated AOT harness to install crate-native
/// bindings before `__aivi_main` runs.
pub fn register_crate_natives_on_ctx(ctx: usize, register_fn: fn(&mut CrateNativeRegistrar)) {
    if ctx == 0 {
        return;
    }
    let ctx_ptr = ctx as *mut abi::JitRuntimeCtx;
    let runtime = unsafe { &mut *(*ctx_ptr).runtime };
    let mut registrar = CrateNativeRegistrar { runtime };
    register_fn(&mut registrar);
}

/// Opaque handle for registering crate-native bridge functions.
pub struct CrateNativeRegistrar<'a> {
    runtime: &'a mut crate::runtime::Runtime,
}

impl<'a> CrateNativeRegistrar<'a> {
    /// Register a bridge function by name.
    ///
    /// - `name`: the global name (e.g., `__crate_native__quick_xml__de__from_str`)
    /// - `arity`: number of parameters
    /// - `func`: the bridge function that takes `Vec<CrateNativeValue>` and returns
    ///   `Result<CrateNativeValue, String>`
    pub fn add(
        &mut self,
        name: &str,
        arity: usize,
        func: impl Fn(Vec<CrateNativeValue>) -> Result<CrateNativeValue, String> + Send + Sync + 'static,
    ) {
        use crate::runtime::values::{BuiltinImpl, BuiltinValue, Value};
        use std::sync::Arc;
        let value = Value::Builtin(BuiltinValue {
            imp: Arc::new(BuiltinImpl {
                name: name.to_string(),
                arity,
                func: Arc::new(move |args: Vec<Value>, _rt: &mut crate::runtime::Runtime| {
                    let native_args: Vec<CrateNativeValue> = args
                        .into_iter()
                        .map(CrateNativeValue::from_runtime)
                        .collect();
                    match func(native_args) {
                        Ok(result) => Ok(result.into_runtime()),
                        Err(msg) => Err(crate::runtime::RuntimeError::Message(msg)),
                    }
                }),
            }),
            args: Vec::new(),
            tagged_args: Some(Vec::new()),
        });
        self.runtime.ctx.globals.set(name.to_string(), value);
    }
}

/// Public value type for crate-native bridge functions.
/// Mirrors `Value` but only exposes types relevant to FFI bridging.
#[derive(Debug, Clone)]
pub enum CrateNativeValue {
    Text(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Unit,
    List(Vec<CrateNativeValue>),
    Record(Vec<(String, CrateNativeValue)>),
    Constructor(String, Vec<CrateNativeValue>),
}

impl CrateNativeValue {
    fn from_runtime(v: crate::runtime::values::Value) -> Self {
        use crate::runtime::values::Value;
        match v {
            Value::Text(s) => CrateNativeValue::Text(s),
            Value::Int(n) => CrateNativeValue::Int(n),
            Value::Float(f) => CrateNativeValue::Float(f),
            Value::Bool(b) => CrateNativeValue::Bool(b),
            Value::Unit => CrateNativeValue::Unit,
            Value::List(items) => {
                let items: Vec<CrateNativeValue> = items
                    .iter()
                    .map(|item| CrateNativeValue::from_runtime(item.clone()))
                    .collect();
                CrateNativeValue::List(items)
            }
            Value::Record(fields) => {
                let fields: Vec<(String, CrateNativeValue)> = fields
                    .iter()
                    .map(|(k, v)| (k.clone(), CrateNativeValue::from_runtime(v.clone())))
                    .collect();
                CrateNativeValue::Record(fields)
            }
            Value::Constructor { name, args } => {
                let args: Vec<CrateNativeValue> = args
                    .into_iter()
                    .map(CrateNativeValue::from_runtime)
                    .collect();
                CrateNativeValue::Constructor(name, args)
            }
            _ => CrateNativeValue::Text(format!("{v:?}")),
        }
    }

    fn into_runtime(self) -> crate::runtime::values::Value {
        use crate::runtime::values::Value;
        use std::collections::HashMap;
        use std::sync::Arc;
        match self {
            CrateNativeValue::Text(s) => Value::Text(s),
            CrateNativeValue::Int(n) => Value::Int(n),
            CrateNativeValue::Float(f) => Value::Float(f),
            CrateNativeValue::Bool(b) => Value::Bool(b),
            CrateNativeValue::Unit => Value::Unit,
            CrateNativeValue::List(items) => {
                let items: Vec<Value> = items.into_iter().map(|i| i.into_runtime()).collect();
                Value::List(Arc::new(items))
            }
            CrateNativeValue::Record(fields) => {
                let map: HashMap<String, Value> = fields
                    .into_iter()
                    .map(|(k, v)| (k, v.into_runtime()))
                    .collect();
                Value::Record(Arc::new(map))
            }
            CrateNativeValue::Constructor(name, args) => {
                let args: Vec<Value> = args.into_iter().map(|a| a.into_runtime()).collect();
                Value::Constructor { name, args }
            }
        }
    }
}
