//! `extern "C"` runtime helpers callable from Cranelift JIT-compiled code.
//!
//! Every helper receives `*mut JitRuntimeCtx` as its first argument.
//! Non-scalar values are passed/returned as `*mut Value` (heap-boxed).

use std::collections::HashMap;
use std::sync::Arc;

use crate::runtime::values::Value;
use crate::runtime::environment::Env;

use super::abi::{self, JitRuntimeCtx};

// ---------------------------------------------------------------------------
// Boxing / unboxing helpers
// ---------------------------------------------------------------------------

/// Box an i64 integer into a heap-allocated `Value::Int`.
pub(crate) extern "C" fn rt_box_int(_ctx: *mut JitRuntimeCtx, value: i64) -> *mut Value {
    abi::box_value(Value::Int(value))
}

/// Box an f64 float into a heap-allocated `Value::Float`.
/// The f64 is passed as raw i64 bits.
pub(crate) extern "C" fn rt_box_float(_ctx: *mut JitRuntimeCtx, bits: i64) -> *mut Value {
    let f = f64::from_bits(bits as u64);
    abi::box_value(Value::Float(f))
}

/// Box a bool into a heap-allocated `Value::Bool`.
pub(crate) extern "C" fn rt_box_bool(_ctx: *mut JitRuntimeCtx, value: i64) -> *mut Value {
    abi::box_value(Value::Bool(value != 0))
}

/// Unbox `Value::Int` → i64.  Panics on type mismatch.
pub(crate) extern "C" fn rt_unbox_int(_ctx: *mut JitRuntimeCtx, ptr: *const Value) -> i64 {
    let value = unsafe { &*ptr };
    match value {
        Value::Int(v) => *v,
        _ => panic!("rt_unbox_int: expected Int"),
    }
}

/// Unbox `Value::Float` → i64 (f64 bit pattern).  Panics on type mismatch.
pub(crate) extern "C" fn rt_unbox_float(_ctx: *mut JitRuntimeCtx, ptr: *const Value) -> i64 {
    let value = unsafe { &*ptr };
    match value {
        Value::Float(v) => v.to_bits() as i64,
        _ => panic!("rt_unbox_float: expected Float"),
    }
}

/// Unbox `Value::Bool` → i64 (0 or 1).  Panics on type mismatch.
pub(crate) extern "C" fn rt_unbox_bool(_ctx: *mut JitRuntimeCtx, ptr: *const Value) -> i64 {
    let value = unsafe { &*ptr };
    match value {
        Value::Bool(v) => i64::from(*v),
        _ => panic!("rt_unbox_bool: expected Bool"),
    }
}

// ---------------------------------------------------------------------------
// Value allocation helpers
// ---------------------------------------------------------------------------

/// Allocate a `Value::Unit`.
pub(crate) extern "C" fn rt_alloc_unit(_ctx: *mut JitRuntimeCtx) -> *mut Value {
    abi::box_value(Value::Unit)
}

/// Allocate a `Value::Text` from a raw UTF-8 pointer + length.
///
/// # Safety
/// `ptr` must point to valid UTF-8 of `len` bytes.
pub(crate) extern "C" fn rt_alloc_string(
    _ctx: *mut JitRuntimeCtx,
    ptr: *const u8,
    len: usize,
) -> *mut Value {
    let s = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len)) };
    abi::box_value(Value::Text(s.to_string()))
}

/// Allocate a `Value::List` from an array of `*const Value` pointers.
///
/// # Safety
/// `items` must point to `len` valid `*const Value` pointers.
pub(crate) extern "C" fn rt_alloc_list(
    _ctx: *mut JitRuntimeCtx,
    items: *const *const Value,
    len: usize,
) -> *mut Value {
    let values: Vec<Value> = (0..len)
        .map(|i| unsafe { (*items.add(i)).as_ref().unwrap().clone() })
        .collect();
    abi::box_value(Value::List(Arc::new(values)))
}

/// Allocate a `Value::Tuple` from an array of `*const Value` pointers.
///
/// # Safety
/// `items` must point to `len` valid `*const Value` pointers.
pub(crate) extern "C" fn rt_alloc_tuple(
    _ctx: *mut JitRuntimeCtx,
    items: *const *const Value,
    len: usize,
) -> *mut Value {
    let values: Vec<Value> = (0..len)
        .map(|i| unsafe { (*items.add(i)).as_ref().unwrap().clone() })
        .collect();
    abi::box_value(Value::Tuple(values))
}

/// Allocate a `Value::Record` from parallel arrays of field-name pointers and value pointers.
///
/// # Safety
/// `names` and `values` must each point to `len` valid entries.
/// Each name entry is a `(*const u8, usize)` pair packed as two consecutive pointer-sized values.
pub(crate) extern "C" fn rt_alloc_record(
    _ctx: *mut JitRuntimeCtx,
    names: *const *const u8,
    name_lens: *const usize,
    values: *const *const Value,
    len: usize,
) -> *mut Value {
    let mut map = HashMap::with_capacity(len);
    for i in 0..len {
        let name = unsafe {
            let ptr = *names.add(i);
            let l = *name_lens.add(i);
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, l)).to_string()
        };
        let val = unsafe { (*values.add(i)).as_ref().unwrap().clone() };
        map.insert(name, val);
    }
    abi::box_value(Value::Record(Arc::new(map)))
}

/// Allocate a `Value::Constructor { name, args }`.
///
/// # Safety
/// `name_ptr`/`name_len` must describe valid UTF-8.
/// `args` must point to `args_len` valid `*const Value` pointers.
pub(crate) extern "C" fn rt_alloc_constructor(
    _ctx: *mut JitRuntimeCtx,
    name_ptr: *const u8,
    name_len: usize,
    args: *const *const Value,
    args_len: usize,
) -> *mut Value {
    let name = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(name_ptr, name_len)).to_string()
    };
    let arg_values: Vec<Value> = (0..args_len)
        .map(|i| unsafe { (*args.add(i)).as_ref().unwrap().clone() })
        .collect();
    abi::box_value(Value::Constructor {
        name,
        args: arg_values,
    })
}

// ---------------------------------------------------------------------------
// Value access helpers
// ---------------------------------------------------------------------------

/// Access a record field by name.  Returns `*mut Value` (Unit if missing).
///
/// # Safety
/// `value_ptr` must be a valid `Value::Record`.
pub(crate) extern "C" fn rt_record_field(
    _ctx: *mut JitRuntimeCtx,
    value_ptr: *const Value,
    name_ptr: *const u8,
    name_len: usize,
) -> *mut Value {
    let value = unsafe { &*value_ptr };
    let name = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(name_ptr, name_len))
    };
    match value {
        Value::Record(rec) => {
            let v = rec.get(name).cloned().unwrap_or(Value::Unit);
            abi::box_value(v)
        }
        _ => abi::box_value(Value::Unit),
    }
}

/// Access a list element by index.  Returns `*mut Value` (Unit if out of bounds).
///
/// # Safety
/// `value_ptr` must be a valid `Value::List`.
pub(crate) extern "C" fn rt_list_index(
    _ctx: *mut JitRuntimeCtx,
    value_ptr: *const Value,
    index: i64,
) -> *mut Value {
    let value = unsafe { &*value_ptr };
    match value {
        Value::List(list) => {
            let idx = if index < 0 {
                (list.len() as i64 + index) as usize
            } else {
                index as usize
            };
            let v = list.get(idx).cloned().unwrap_or(Value::Unit);
            abi::box_value(v)
        }
        _ => abi::box_value(Value::Unit),
    }
}

// ---------------------------------------------------------------------------
// Value lifecycle helpers
// ---------------------------------------------------------------------------

/// Clone a heap-boxed `Value`, returning a new heap-boxed copy.
///
/// # Safety
/// `ptr` must point to a valid `Value`.
pub(crate) extern "C" fn rt_clone_value(
    _ctx: *mut JitRuntimeCtx,
    ptr: *const Value,
) -> *mut Value {
    unsafe { abi::clone_boxed_value(ptr) }
}

/// Drop (deallocate) a heap-boxed `Value`.
///
/// # Safety
/// `ptr` must have been created by one of the `rt_alloc_*` / `rt_box_*` helpers
/// and must not be used afterwards.
pub(crate) extern "C" fn rt_drop_value(_ctx: *mut JitRuntimeCtx, ptr: *mut Value) {
    unsafe { abi::unbox_value(ptr); }
}

// ---------------------------------------------------------------------------
// Runtime interaction helpers
// ---------------------------------------------------------------------------

/// Look up a global definition by name, forcing thunks.
///
/// # Safety
/// `ctx` must be a valid `JitRuntimeCtx` pointer.
pub(crate) extern "C" fn rt_get_global(
    ctx: *mut JitRuntimeCtx,
    name_ptr: *const u8,
    name_len: usize,
) -> *mut Value {
    let name = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(name_ptr, name_len))
    };
    let runtime = unsafe { (*ctx).runtime_mut() };
    let val = runtime
        .ctx
        .globals
        .get(name)
        .unwrap_or(Value::Unit);
    let forced = runtime.force_value(val).unwrap_or(Value::Unit);
    abi::box_value(forced)
}

/// Apply a closure/builtin value to one argument.
///
/// # Safety
/// `ctx` must be a valid `JitRuntimeCtx` pointer.
/// `func_ptr` and `arg_ptr` must be valid `Value` pointers.
pub(crate) extern "C" fn rt_apply(
    ctx: *mut JitRuntimeCtx,
    func_ptr: *const Value,
    arg_ptr: *const Value,
) -> *mut Value {
    let func = unsafe { &*func_ptr };
    let arg = unsafe { &*arg_ptr };
    let runtime = unsafe { (*ctx).runtime_mut() };
    match runtime.apply(func.clone(), arg.clone()) {
        Ok(val) => abi::box_value(val),
        Err(_) => abi::box_value(Value::Unit),
    }
}

/// Force a thunk value (or return it unchanged if not a thunk).
///
/// # Safety
/// `ctx` must be valid.  `ptr` must be a valid `Value` pointer.
pub(crate) extern "C" fn rt_force_thunk(
    ctx: *mut JitRuntimeCtx,
    ptr: *const Value,
) -> *mut Value {
    let value = unsafe { (*ptr).clone() };
    let runtime = unsafe { (*ctx).runtime_mut() };
    let forced = runtime.force_value(value).unwrap_or(Value::Unit);
    abi::box_value(forced)
}

/// Run an effect value (for the `main` entrypoint).
///
/// # Safety
/// `ctx` must be valid.  `ptr` must be a valid `Value::Effect`.
pub(crate) extern "C" fn rt_run_effect(
    ctx: *mut JitRuntimeCtx,
    ptr: *const Value,
) -> *mut Value {
    let value = unsafe { (*ptr).clone() };
    let runtime = unsafe { (*ctx).runtime_mut() };
    match runtime.run_effect_value(value) {
        Ok(val) => abi::box_value(val),
        Err(_) => abi::box_value(Value::Unit),
    }
}

/// Bind an effect: given an effect and a continuation function, produce a
/// new effect that chains them.
///
/// # Safety
/// `ctx` must be valid.
pub(crate) extern "C" fn rt_bind_effect(
    ctx: *mut JitRuntimeCtx,
    effect_ptr: *const Value,
    cont_ptr: *const Value,
) -> *mut Value {
    let effect = unsafe { (*effect_ptr).clone() };
    let cont = unsafe { (*cont_ptr).clone() };
    let runtime = unsafe { (*ctx).runtime_mut() };
    // Execute the effect, then apply the continuation to the result
    match runtime.run_effect_value(effect) {
        Ok(result) => match runtime.apply(cont, result) {
            Ok(val) => abi::box_value(val),
            Err(_) => abi::box_value(Value::Unit),
        },
        Err(_) => abi::box_value(Value::Unit),
    }
}

// ---------------------------------------------------------------------------
// Pattern matching helpers
// ---------------------------------------------------------------------------

/// Check if a value is a Constructor with the given name.
/// Returns 1 if match, 0 otherwise.
pub(crate) extern "C" fn rt_constructor_name_eq(
    _ctx: *mut JitRuntimeCtx,
    ptr: *const Value,
    name_ptr: *const u8,
    name_len: usize,
) -> i64 {
    let value = unsafe { &*ptr };
    let name = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(name_ptr, name_len))
    };
    match value {
        Value::Constructor { name: n, .. } => i64::from(n == name),
        _ => 0,
    }
}

/// Get the number of arguments of a Constructor value.
pub(crate) extern "C" fn rt_constructor_arity(
    _ctx: *mut JitRuntimeCtx,
    ptr: *const Value,
) -> i64 {
    let value = unsafe { &*ptr };
    match value {
        Value::Constructor { args, .. } => args.len() as i64,
        _ => 0,
    }
}

/// Get a Constructor argument by index.
pub(crate) extern "C" fn rt_constructor_arg(
    _ctx: *mut JitRuntimeCtx,
    ptr: *const Value,
    index: i64,
) -> *mut Value {
    let value = unsafe { &*ptr };
    match value {
        Value::Constructor { args, .. } => args
            .get(index as usize)
            .cloned()
            .map(abi::box_value)
            .unwrap_or_else(|| abi::box_value(Value::Unit)),
        _ => abi::box_value(Value::Unit),
    }
}

/// Get the length of a Tuple value.
pub(crate) extern "C" fn rt_tuple_len(
    _ctx: *mut JitRuntimeCtx,
    ptr: *const Value,
) -> i64 {
    let value = unsafe { &*ptr };
    match value {
        Value::Tuple(items) => items.len() as i64,
        _ => 0,
    }
}

/// Get a Tuple element by index.
pub(crate) extern "C" fn rt_tuple_item(
    _ctx: *mut JitRuntimeCtx,
    ptr: *const Value,
    index: i64,
) -> *mut Value {
    let value = unsafe { &*ptr };
    match value {
        Value::Tuple(items) => items
            .get(index as usize)
            .cloned()
            .map(abi::box_value)
            .unwrap_or_else(|| abi::box_value(Value::Unit)),
        _ => abi::box_value(Value::Unit),
    }
}

/// Get the length of a List value.
pub(crate) extern "C" fn rt_list_len(
    _ctx: *mut JitRuntimeCtx,
    ptr: *const Value,
) -> i64 {
    let value = unsafe { &*ptr };
    match value {
        Value::List(items) => items.len() as i64,
        _ => 0,
    }
}

/// Get a sub-list (tail) starting from `start` index.
pub(crate) extern "C" fn rt_list_tail(
    _ctx: *mut JitRuntimeCtx,
    ptr: *const Value,
    start: i64,
) -> *mut Value {
    let value = unsafe { &*ptr };
    match value {
        Value::List(items) => {
            let s = start as usize;
            let tail = if s < items.len() {
                items[s..].to_vec()
            } else {
                Vec::new()
            };
            abi::box_value(Value::List(Arc::new(tail)))
        }
        _ => abi::box_value(Value::List(Arc::new(Vec::new()))),
    }
}

/// Check structural equality of two values.
/// Returns 1 if equal, 0 otherwise.
pub(crate) extern "C" fn rt_value_equals(
    _ctx: *mut JitRuntimeCtx,
    a: *const Value,
    b: *const Value,
) -> i64 {
    let va = unsafe { &*a };
    let vb = unsafe { &*b };
    i64::from(crate::runtime::values_equal(va, vb))
}

// ---------------------------------------------------------------------------
// Record patching helper
// ---------------------------------------------------------------------------

/// Patch a record: clone the base record and overlay new fields.
///
/// # Safety
/// `base_ptr` must be a valid `Value::Record` (or any Value — non-records
/// produce a fresh record).  `names`, `name_lens`, `values` arrays must
/// each have `len` entries.
pub(crate) extern "C" fn rt_patch_record(
    _ctx: *mut JitRuntimeCtx,
    base_ptr: *const Value,
    names: *const *const u8,
    name_lens: *const usize,
    values: *const *const Value,
    len: usize,
) -> *mut Value {
    let base = unsafe { &*base_ptr };
    let mut map = match base {
        Value::Record(rec) => (**rec).clone(),
        _ => HashMap::new(),
    };
    for i in 0..len {
        let name = unsafe {
            let ptr = *names.add(i);
            let l = *name_lens.add(i);
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, l)).to_string()
        };
        let val = unsafe { (*values.add(i)).as_ref().unwrap().clone() };
        map.insert(name, val);
    }
    abi::box_value(Value::Record(Arc::new(map)))
}

// ---------------------------------------------------------------------------
// Closure creation helper
// ---------------------------------------------------------------------------

/// Create a `Value::Builtin` closure that wraps a JIT-compiled lambda function.
///
/// The lambda function has the fixed signature `(ctx, env, param) -> result`
/// where `env` is a `*const *const Value` pointing to the captured values.
///
/// # Safety
/// `func_ptr` must point to valid JIT code with the signature above.
/// `captured` must point to `captured_count` valid `*const Value` pointers.
pub(crate) extern "C" fn rt_make_closure(
    _ctx: *mut JitRuntimeCtx,
    func_ptr: i64,
    captured: *const *const Value,
    captured_count: i64,
) -> *mut Value {
    use std::sync::Arc;
    use crate::runtime::values::{BuiltinImpl, BuiltinValue};
    use crate::runtime::Runtime;

    let count = captured_count as usize;
    let captured_values: Vec<Value> = (0..count)
        .map(|i| unsafe { (*captured.add(i)).as_ref().unwrap().clone() })
        .collect();

    let builtin = Value::Builtin(BuiltinValue {
        imp: Arc::new(BuiltinImpl {
            name: format!("__jit_closure_{:#x}", func_ptr),
            arity: 1,
            func: Arc::new(move |args: Vec<Value>, runtime: &mut Runtime| {
                let arg = args.into_iter().next().unwrap_or(Value::Unit);
                let ctx = unsafe { JitRuntimeCtx::from_runtime(runtime) };
                let ctx_ptr = &ctx as *const JitRuntimeCtx as usize;

                // Box captured values into a stack array
                let boxed_caps: Vec<*mut Value> = captured_values
                    .iter()
                    .map(|v| abi::box_value(v.clone()))
                    .collect();

                // Create the env array (array of *const Value)
                let env_ptr = if boxed_caps.is_empty() {
                    std::ptr::null::<*const Value>() as usize
                } else {
                    boxed_caps.as_ptr() as usize
                };

                let arg_ptr = abi::box_value(arg);

                // Call: func(ctx, env, param) -> result
                let f: extern "C" fn(i64, i64, i64) -> i64 =
                    unsafe { std::mem::transmute(func_ptr as *const u8) };
                let result_ptr = f(ctx_ptr as i64, env_ptr as i64, arg_ptr as i64);

                let result = if result_ptr == 0 {
                    Value::Unit
                } else {
                    let rp = result_ptr as *const Value;
                    unsafe { (*rp).clone() }
                };

                // Drop boxed captures
                for cap_ptr in boxed_caps {
                    if cap_ptr as i64 != result_ptr {
                        unsafe { drop(Box::from_raw(cap_ptr)); }
                    }
                }
                // Drop arg
                if arg_ptr as i64 != result_ptr {
                    unsafe { drop(Box::from_raw(arg_ptr)); }
                }
                // Drop result
                if result_ptr != 0 {
                    unsafe { drop(Box::from_raw(result_ptr as *mut Value)); }
                }

                Ok(result)
            }),
        }),
        args: Vec::new(),
        tagged_args: None,
    });
    abi::box_value(builtin)
}

// ---------------------------------------------------------------------------
// Binary operation helper (non-scalar fallback)
// ---------------------------------------------------------------------------

/// Evaluate a binary operation on two `Value`s, returning a new boxed `Value`.
///
/// Uses the built-in evaluation first, then falls back to looking up the
/// operator in globals if not found.
///
/// # Safety
/// `ctx` must be valid.  `op_ptr`/`op_len` must describe a valid UTF-8 operator
/// string (e.g. "+", "-", "==").  `lhs_ptr` and `rhs_ptr` must be valid.
pub(crate) extern "C" fn rt_binary_op(
    ctx: *mut JitRuntimeCtx,
    op_ptr: *const u8,
    op_len: usize,
    lhs_ptr: *const Value,
    rhs_ptr: *const Value,
) -> *mut Value {
    let op = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(op_ptr, op_len))
    };
    let lhs = unsafe { (*lhs_ptr).clone() };
    let rhs = unsafe { (*rhs_ptr).clone() };

    // Fast path: try the pure built-in evaluation
    if let Some(result) = crate::runtime::eval_binary_builtin(op, &lhs, &rhs) {
        return abi::box_value(result);
    }

    // Slow path: look up operator in globals and apply curried
    let runtime = unsafe { (*ctx).runtime_mut() };
    let op_name = format!("({})", op);
    if let Some(op_value) = runtime.ctx.globals.get(&op_name) {
        if let Ok(applied) = runtime.apply(op_value, lhs) {
            if let Ok(result) = runtime.apply(applied, rhs) {
                return abi::box_value(result);
            }
        }
    }
    abi::box_value(Value::Unit)
}

// ---------------------------------------------------------------------------
// Environment helpers for block delegation
// ---------------------------------------------------------------------------

/// Create a new empty `Env` for passing local scope to interpreter-delegated blocks.
/// The env's parent is the runtime's global scope so global lookups work.
pub(crate) extern "C" fn rt_env_new(ctx: *mut JitRuntimeCtx) -> *mut Env {
    let runtime = unsafe { &*(*ctx).runtime };
    let globals = runtime.ctx.globals.clone();
    Box::into_raw(Box::new(Env::new(Some(globals))))
}

/// Set a variable in an environment created by `rt_env_new`.
pub(crate) extern "C" fn rt_env_set(
    _ctx: *mut JitRuntimeCtx,
    env_ptr: *mut Env,
    name_ptr: *const u8,
    name_len: usize,
    value_ptr: *const Value,
) {
    let env = unsafe { &*env_ptr };
    let name = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(name_ptr, name_len)).to_string()
    };
    let value = unsafe { (*value_ptr).clone() };
    env.set(name, value);
}

/// Evaluate a `generate { ... }` block by delegating to the interpreter.
///
/// `items_ptr` / `items_count` point to the `&[RustIrBlockItem]` slice from the
/// live `RustIrDef` (valid for the duration of JIT execution).
/// `env_ptr` is an `Env` populated with all in-scope locals.
pub(crate) extern "C" fn rt_eval_generate(
    ctx: *mut JitRuntimeCtx,
    items_ptr: *const crate::rust_ir::RustIrBlockItem,
    items_count: usize,
    env_ptr: *mut Env,
) -> *mut Value {
    use crate::runtime::values::{BuiltinImpl, BuiltinValue};

    let items = unsafe { std::slice::from_raw_parts(items_ptr, items_count) };
    let env = unsafe { Box::from_raw(env_ptr) };
    let runtime = unsafe { &mut *(*ctx).runtime };

    // Lower RustIrBlockItems → HirBlockItems, then materialize via interpreter
    let lowered = match crate::runtime::lower_runtime_rust_ir_block_items(items) {
        Ok(l) => l,
        Err(_) => return abi::box_value(Value::List(Arc::new(Vec::new()))),
    };

    let mut values = Vec::new();
    if runtime.materialize_generate(&lowered, &env, &mut values).is_err() {
        return abi::box_value(Value::List(Arc::new(Vec::new())));
    }

    // Drop env (we consumed it above via Box::from_raw)
    // env is already dropped when Box goes out of scope

    // Wrap as fold function: \k -> \z -> foldl k z values
    let values = Arc::new(values);
    let builtin = Value::Builtin(BuiltinValue {
        imp: Arc::new(BuiltinImpl {
            name: "<jit_generator>".to_string(),
            arity: 2,
            func: Arc::new(move |mut args, runtime| {
                let z = args.pop().unwrap();
                let k = args.pop().unwrap();
                let mut acc = z;
                for val in values.iter() {
                    let partial = runtime.apply(k.clone(), acc)?;
                    acc = runtime.apply(partial, val.clone())?;
                }
                Ok(acc)
            }),
        }),
        args: Vec::new(),
        tagged_args: Some(Vec::new()),
    });
    abi::box_value(builtin)
}

/// Create a `Value::Resource` wrapping the given block items and env.
pub(crate) extern "C" fn rt_make_resource(
    _ctx: *mut JitRuntimeCtx,
    items_ptr: *const crate::rust_ir::RustIrBlockItem,
    items_count: usize,
    env_ptr: *mut Env,
) -> *mut Value {
    use crate::runtime::values::ResourceValue;

    let items = unsafe { std::slice::from_raw_parts(items_ptr, items_count) };
    let _env = unsafe { Box::from_raw(env_ptr) };

    let lowered = match crate::runtime::lower_runtime_rust_ir_block_items(items) {
        Ok(l) => l,
        Err(_) => return abi::box_value(Value::Unit),
    };

    abi::box_value(Value::Resource(Arc::new(ResourceValue {
        items: Arc::new(lowered),
    })))
}

// ---------------------------------------------------------------------------
// Symbol table for JITBuilder registration
// ---------------------------------------------------------------------------

/// All runtime helper symbols that need to be registered with the Cranelift
/// `JITBuilder` so JIT-compiled code can call them.
///
/// Returns pairs of `(name, fn_pointer_as_usize)`.
pub(crate) fn runtime_helper_symbols() -> Vec<(&'static str, *const u8)> {
    vec![
        ("rt_box_int", rt_box_int as *const u8),
        ("rt_box_float", rt_box_float as *const u8),
        ("rt_box_bool", rt_box_bool as *const u8),
        ("rt_unbox_int", rt_unbox_int as *const u8),
        ("rt_unbox_float", rt_unbox_float as *const u8),
        ("rt_unbox_bool", rt_unbox_bool as *const u8),
        ("rt_alloc_unit", rt_alloc_unit as *const u8),
        ("rt_alloc_string", rt_alloc_string as *const u8),
        ("rt_alloc_list", rt_alloc_list as *const u8),
        ("rt_alloc_tuple", rt_alloc_tuple as *const u8),
        ("rt_alloc_record", rt_alloc_record as *const u8),
        ("rt_alloc_constructor", rt_alloc_constructor as *const u8),
        ("rt_record_field", rt_record_field as *const u8),
        ("rt_list_index", rt_list_index as *const u8),
        ("rt_clone_value", rt_clone_value as *const u8),
        ("rt_drop_value", rt_drop_value as *const u8),
        ("rt_get_global", rt_get_global as *const u8),
        ("rt_apply", rt_apply as *const u8),
        ("rt_force_thunk", rt_force_thunk as *const u8),
        ("rt_run_effect", rt_run_effect as *const u8),
        ("rt_bind_effect", rt_bind_effect as *const u8),
        ("rt_binary_op", rt_binary_op as *const u8),
        // Pattern matching helpers
        ("rt_constructor_name_eq", rt_constructor_name_eq as *const u8),
        ("rt_constructor_arity", rt_constructor_arity as *const u8),
        ("rt_constructor_arg", rt_constructor_arg as *const u8),
        ("rt_tuple_len", rt_tuple_len as *const u8),
        ("rt_tuple_item", rt_tuple_item as *const u8),
        ("rt_list_len", rt_list_len as *const u8),
        ("rt_list_tail", rt_list_tail as *const u8),
        ("rt_value_equals", rt_value_equals as *const u8),
        // Record patching
        ("rt_patch_record", rt_patch_record as *const u8),
        // Closure creation
        ("rt_make_closure", rt_make_closure as *const u8),
        // Block delegation
        ("rt_env_new", rt_env_new as *const u8),
        ("rt_env_set", rt_env_set as *const u8),
        ("rt_eval_generate", rt_eval_generate as *const u8),
        ("rt_make_resource", rt_make_resource as *const u8),
    ]
}
