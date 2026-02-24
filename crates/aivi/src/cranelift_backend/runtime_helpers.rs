//! `extern "C"` runtime helpers callable from Cranelift JIT-compiled code.
//!
//! Every helper receives `*mut JitRuntimeCtx` as its first argument.
//! Non-scalar values are passed/returned as `*mut Value` (heap-boxed).

use std::collections::HashMap;
use std::sync::Arc;

use crate::runtime::values::Value;

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
    ]
}
