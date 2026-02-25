//! `extern "C"` runtime helpers callable from Cranelift JIT-compiled code.
//!
//! Every helper receives `*mut JitRuntimeCtx` as its first argument.
//! Non-scalar values are passed/returned as `*mut Value` (heap-boxed).

use std::collections::HashMap;
use std::sync::Arc;

use crate::runtime::values::Value;
use crate::runtime::format_runtime_error;

use super::abi::{self, JitRuntimeCtx};

// ---------------------------------------------------------------------------
// Boxing / unboxing helpers
// ---------------------------------------------------------------------------

/// Box an i64 integer into a heap-allocated `Value::Int`.
#[no_mangle]
pub extern "C" fn rt_box_int(_ctx: *mut JitRuntimeCtx, value: i64) -> *mut Value {
    abi::box_value(Value::Int(value))
}

/// Box an f64 float into a heap-allocated `Value::Float`.
/// The f64 is passed as raw i64 bits.
#[no_mangle]
pub extern "C" fn rt_box_float(_ctx: *mut JitRuntimeCtx, bits: i64) -> *mut Value {
    let f = f64::from_bits(bits as u64);
    abi::box_value(Value::Float(f))
}

/// Box a bool into a heap-allocated `Value::Bool`.
#[no_mangle]
pub extern "C" fn rt_box_bool(_ctx: *mut JitRuntimeCtx, value: i64) -> *mut Value {
    abi::box_value(Value::Bool(value != 0))
}

/// Unbox `Value::Int` → i64. Returns 0 and logs on type mismatch.
#[no_mangle]
pub extern "C" fn rt_unbox_int(_ctx: *mut JitRuntimeCtx, ptr: *const Value) -> i64 {
    let value = unsafe { &*ptr };
    match value {
        Value::Int(v) => *v,
        other => {
            eprintln!("aivi: rt_unbox_int: expected Int, got {other:?}");
            0
        }
    }
}

/// Unbox `Value::Float` → i64 (f64 bit pattern). Returns 0.0 bits and logs on type mismatch.
#[no_mangle]
pub extern "C" fn rt_unbox_float(_ctx: *mut JitRuntimeCtx, ptr: *const Value) -> i64 {
    let value = unsafe { &*ptr };
    match value {
        Value::Float(v) => v.to_bits() as i64,
        other => {
            eprintln!("aivi: rt_unbox_float: expected Float, got {other:?}");
            0f64.to_bits() as i64
        }
    }
}

/// Unbox `Value::Bool` → i64 (0 or 1). Returns 0 (false) and logs on type mismatch.
#[no_mangle]
pub extern "C" fn rt_unbox_bool(_ctx: *mut JitRuntimeCtx, ptr: *const Value) -> i64 {
    let value = unsafe { &*ptr };
    match value {
        Value::Bool(v) => i64::from(*v),
        other => {
            eprintln!("aivi: rt_unbox_bool: expected Bool, got {other:?}");
            0
        }
    }
}

// ---------------------------------------------------------------------------
// Value allocation helpers
// ---------------------------------------------------------------------------

/// Allocate a `Value::Unit`.
#[no_mangle]
pub extern "C" fn rt_alloc_unit(_ctx: *mut JitRuntimeCtx) -> *mut Value {
    abi::box_value(Value::Unit)
}

/// Allocate a `Value::Text` from a raw UTF-8 pointer + length.
///
/// # Safety
/// `ptr` must point to valid UTF-8 of `len` bytes.
#[no_mangle]
pub extern "C" fn rt_alloc_string(
    _ctx: *mut JitRuntimeCtx,
    ptr: *const u8,
    len: usize,
) -> *mut Value {
    let s = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len)) };
    abi::box_value(Value::Text(s.to_string()))
}

/// Allocate a `Value::DateTime` from a UTF-8 string.
#[no_mangle]
pub extern "C" fn rt_alloc_datetime(
    _ctx: *mut JitRuntimeCtx,
    ptr: *const u8,
    len: usize,
) -> *mut Value {
    let s = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len)) };
    abi::box_value(Value::DateTime(s.to_string()))
}

/// Allocate a `Value::List` from an array of `*const Value` pointers.
///
/// # Safety
/// `items` must point to `len` valid `*const Value` pointers.
#[no_mangle]
pub extern "C" fn rt_alloc_list(
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
#[no_mangle]
pub extern "C" fn rt_alloc_tuple(
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
#[no_mangle]
pub extern "C" fn rt_alloc_record(
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
#[no_mangle]
pub extern "C" fn rt_alloc_constructor(
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

/// Access a record field by name. Returns `*mut Value` (Unit with diagnostic if missing).
///
/// # Safety
/// `value_ptr` must be a valid `Value::Record`.
#[no_mangle]
pub extern "C" fn rt_record_field(
    _ctx: *mut JitRuntimeCtx,
    value_ptr: *const Value,
    name_ptr: *const u8,
    name_len: usize,
) -> *mut Value {
    let value = unsafe { &*value_ptr };
    let name =
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(name_ptr, name_len)) };
    match value {
        Value::Record(rec) => match rec.get(name) {
            Some(v) => abi::box_value(v.clone()),
            None => {
                eprintln!("aivi: record field '{name}' not found");
                abi::box_value(Value::Unit)
            }
        },
        other => {
            eprintln!("aivi: rt_record_field: expected Record, got {other:?}");
            abi::box_value(Value::Unit)
        }
    }
}

/// Access a list element by index. Returns `*mut Value` (Unit with diagnostic if out of bounds).
///
/// # Safety
/// `value_ptr` must be a valid `Value::List`.
#[no_mangle]
pub extern "C" fn rt_list_index(
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
            match list.get(idx) {
                Some(v) => abi::box_value(v.clone()),
                None => {
                    eprintln!(
                        "aivi: list index {index} out of bounds (len {})",
                        list.len()
                    );
                    abi::box_value(Value::Unit)
                }
            }
        }
        other => {
            eprintln!("aivi: rt_list_index: expected List, got {other:?}");
            abi::box_value(Value::Unit)
        }
    }
}

// ---------------------------------------------------------------------------
// Value lifecycle helpers
// ---------------------------------------------------------------------------

/// Clone a heap-boxed `Value`, returning a new heap-boxed copy.
///
/// # Safety
/// `ptr` must point to a valid `Value`.
#[no_mangle]
pub extern "C" fn rt_clone_value(_ctx: *mut JitRuntimeCtx, ptr: *const Value) -> *mut Value {
    unsafe { abi::clone_boxed_value(ptr) }
}

/// Drop (deallocate) a heap-boxed `Value`.
///
/// # Safety
/// `ptr` must have been created by one of the `rt_alloc_*` / `rt_box_*` helpers
/// and must not be used afterwards.
#[no_mangle]
pub extern "C" fn rt_drop_value(_ctx: *mut JitRuntimeCtx, ptr: *mut Value) {
    unsafe {
        abi::unbox_value(ptr);
    }
}

// ---------------------------------------------------------------------------
// Runtime interaction helpers
// ---------------------------------------------------------------------------

/// Look up a global definition by name, forcing thunks.
///
/// # Safety
/// `ctx` must be a valid `JitRuntimeCtx` pointer.
#[no_mangle]
pub extern "C" fn rt_get_global(
    ctx: *mut JitRuntimeCtx,
    name_ptr: *const u8,
    name_len: usize,
) -> *mut Value {
    let name =
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(name_ptr, name_len)) };
    let runtime = unsafe { (*ctx).runtime_mut() };
    let val = match runtime.ctx.globals.get(name) {
        Some(v) => v,
        None => {
            eprintln!("aivi: global '{name}' not found");
            return abi::box_value(Value::Unit);
        }
    };
    match runtime.force_value(val) {
        Ok(forced) => abi::box_value(forced),
        Err(e) => {
            eprintln!("aivi: force global '{name}': {}", format_runtime_error(e));
            abi::box_value(Value::Unit)
        }
    }
}

/// Apply a closure/builtin value to one argument.
///
/// # Safety
/// `ctx` must be a valid `JitRuntimeCtx` pointer.
/// `func_ptr` and `arg_ptr` must be valid `Value` pointers.
#[no_mangle]
pub extern "C" fn rt_apply(
    ctx: *mut JitRuntimeCtx,
    func_ptr: *const Value,
    arg_ptr: *const Value,
) -> *mut Value {
    let func = unsafe { &*func_ptr };
    let arg = unsafe { &*arg_ptr };

    // Fast path: fully-saturated builtin (arity reached with this arg)
    if let Value::Builtin(ref b) = func {
        if b.args.len() + 1 == b.imp.arity {
            let mut all_args = b.args.clone();
            all_args.push(arg.clone());
            let runtime = unsafe { (*ctx).runtime_mut() };
            match (b.imp.func)(all_args, runtime) {
                Ok(val) => return abi::box_value(val),
                Err(e) => {
                    eprintln!("aivi: error in builtin '{}': {}", b.imp.name, format_runtime_error(e));
                    return abi::box_value(Value::Unit);
                }
            }
        }
        // Partial application: accumulate arg without going through trampoline
        if b.args.len() + 1 < b.imp.arity {
            let mut new_args = b.args.clone();
            new_args.push(arg.clone());
            let new_tagged = b.tagged_args.as_ref().map(|t| {
                let mut new_t = t.clone();
                if let Some(tv) = crate::runtime::values::TaggedValue::from_value(arg) {
                    new_t.push(tv);
                }
                new_t
            });
            return abi::box_value(Value::Builtin(crate::runtime::values::BuiltinValue {
                imp: b.imp.clone(),
                args: new_args,
                tagged_args: new_tagged,
            }));
        }
    }

    // Fast path: constructor application (just accumulate the arg)
    if let Value::Constructor { ref name, ref args } = func {
        let mut new_args = args.clone();
        new_args.push(arg.clone());
        return abi::box_value(Value::Constructor {
            name: name.clone(),
            args: new_args,
        });
    }

    let runtime = unsafe { (*ctx).runtime_mut() };
    match runtime.apply(func.clone(), arg.clone()) {
        Ok(val) => abi::box_value(val),
        Err(e) => {
            eprintln!("aivi: apply error: {}", format_runtime_error(e));
            abi::box_value(Value::Unit)
        }
    }
}

/// Force a thunk value (or return it unchanged if not a thunk).
///
/// # Safety
/// `ctx` must be valid.  `ptr` must be a valid `Value` pointer.
#[no_mangle]
pub extern "C" fn rt_force_thunk(ctx: *mut JitRuntimeCtx, ptr: *const Value) -> *mut Value {
    let value = unsafe { (*ptr).clone() };
    let runtime = unsafe { (*ctx).runtime_mut() };
    match runtime.force_value(value) {
        Ok(val) => abi::box_value(val),
        Err(e) => {
            eprintln!("aivi: force_thunk error: {}", format_runtime_error(e));
            abi::box_value(Value::Unit)
        }
    }
}

/// Run an effect value (for the `main` entrypoint).
///
/// # Safety
/// `ctx` must be valid.  `ptr` must be a valid `Value::Effect`.
#[no_mangle]
pub extern "C" fn rt_run_effect(ctx: *mut JitRuntimeCtx, ptr: *const Value) -> *mut Value {
    let value = unsafe { (*ptr).clone() };
    let runtime = unsafe { (*ctx).runtime_mut() };
    match runtime.run_effect_value(value) {
        Ok(val) => abi::box_value(val),
        Err(e) => {
            eprintln!("aivi: effect error: {}", format_runtime_error(e));
            abi::box_value(Value::Unit)
        }
    }
}

/// Bind an effect: given an effect and a continuation function, produce a
/// new effect that chains them.
///
/// # Safety
/// `ctx` must be valid.
#[no_mangle]
pub extern "C" fn rt_bind_effect(
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
            Err(e) => {
                eprintln!("aivi: bind continuation error: {}", format_runtime_error(e));
                abi::box_value(Value::Unit)
            }
        },
        Err(e) => {
            eprintln!("aivi: bind effect error: {}", format_runtime_error(e));
            abi::box_value(Value::Unit)
        }
    }
}

// ---------------------------------------------------------------------------
// Pattern matching helpers
// ---------------------------------------------------------------------------

/// Check if a value is a Constructor with the given name.
/// Returns 1 if match, 0 otherwise.
#[no_mangle]
pub extern "C" fn rt_constructor_name_eq(
    _ctx: *mut JitRuntimeCtx,
    ptr: *const Value,
    name_ptr: *const u8,
    name_len: usize,
) -> i64 {
    let value = unsafe { &*ptr };
    let name =
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(name_ptr, name_len)) };
    match value {
        Value::Constructor { name: n, .. } => i64::from(n == name),
        _ => 0,
    }
}

/// Get the number of arguments of a Constructor value.
#[no_mangle]
pub extern "C" fn rt_constructor_arity(_ctx: *mut JitRuntimeCtx, ptr: *const Value) -> i64 {
    let value = unsafe { &*ptr };
    match value {
        Value::Constructor { args, .. } => args.len() as i64,
        _ => 0,
    }
}

/// Get a Constructor argument by index.
#[no_mangle]
pub extern "C" fn rt_constructor_arg(
    _ctx: *mut JitRuntimeCtx,
    ptr: *const Value,
    index: i64,
) -> *mut Value {
    let value = unsafe { &*ptr };
    match value {
        Value::Constructor { args, name } => match args.get(index as usize) {
            Some(v) => abi::box_value(v.clone()),
            None => {
                eprintln!(
                    "aivi: constructor '{name}' arg index {index} out of bounds (has {})",
                    args.len()
                );
                abi::box_value(Value::Unit)
            }
        },
        other => {
            eprintln!("aivi: rt_constructor_arg: expected Constructor, got {other:?}");
            abi::box_value(Value::Unit)
        }
    }
}

/// Get the length of a Tuple value.
#[no_mangle]
pub extern "C" fn rt_tuple_len(_ctx: *mut JitRuntimeCtx, ptr: *const Value) -> i64 {
    let value = unsafe { &*ptr };
    match value {
        Value::Tuple(items) => items.len() as i64,
        _ => 0,
    }
}

/// Get a Tuple element by index.
#[no_mangle]
pub extern "C" fn rt_tuple_item(
    _ctx: *mut JitRuntimeCtx,
    ptr: *const Value,
    index: i64,
) -> *mut Value {
    let value = unsafe { &*ptr };
    match value {
        Value::Tuple(items) => match items.get(index as usize) {
            Some(v) => abi::box_value(v.clone()),
            None => {
                eprintln!(
                    "aivi: tuple index {index} out of bounds (len {})",
                    items.len()
                );
                abi::box_value(Value::Unit)
            }
        },
        other => {
            eprintln!("aivi: rt_tuple_item: expected Tuple, got {other:?}");
            abi::box_value(Value::Unit)
        }
    }
}

/// Get the length of a List value.
#[no_mangle]
pub extern "C" fn rt_list_len(_ctx: *mut JitRuntimeCtx, ptr: *const Value) -> i64 {
    let value = unsafe { &*ptr };
    match value {
        Value::List(items) => items.len() as i64,
        _ => 0,
    }
}

/// Get a sub-list (tail) starting from `start` index.
#[no_mangle]
pub extern "C" fn rt_list_tail(
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
#[no_mangle]
pub extern "C" fn rt_value_equals(
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
#[no_mangle]
pub extern "C" fn rt_patch_record(
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
#[no_mangle]
pub extern "C" fn rt_make_closure(
    _ctx: *mut JitRuntimeCtx,
    func_ptr: i64,
    captured: *const *const Value,
    captured_count: i64,
) -> *mut Value {
    use crate::runtime::values::{BuiltinImpl, BuiltinValue};
    use crate::runtime::Runtime;
    use std::sync::Arc;

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
                    eprintln!("aivi: closure returned null pointer");
                    Value::Unit
                } else {
                    let rp = result_ptr as *const Value;
                    unsafe { (*rp).clone() }
                };

                // Drop boxed captures
                for cap_ptr in boxed_caps {
                    if cap_ptr as i64 != result_ptr {
                        unsafe {
                            drop(Box::from_raw(cap_ptr));
                        }
                    }
                }
                // Drop arg
                if arg_ptr as i64 != result_ptr {
                    unsafe {
                        drop(Box::from_raw(arg_ptr));
                    }
                }
                // Drop result
                if result_ptr != 0 {
                    unsafe {
                        drop(Box::from_raw(result_ptr as *mut Value));
                    }
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
#[no_mangle]
pub extern "C" fn rt_binary_op(
    ctx: *mut JitRuntimeCtx,
    op_ptr: *const u8,
    op_len: usize,
    lhs_ptr: *const Value,
    rhs_ptr: *const Value,
) -> *mut Value {
    let op = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(op_ptr, op_len)) };
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
    eprintln!("aivi: binary op '{op}' failed for operand types");
    abi::box_value(Value::Unit)
}

// ---------------------------------------------------------------------------
// Native generate block helpers
// ---------------------------------------------------------------------------

/// Allocate a new empty `Vec<Value>` on the heap. Returns a raw pointer.
#[no_mangle]
pub extern "C" fn rt_gen_vec_new(_ctx: *mut JitRuntimeCtx) -> *mut Vec<Value> {
    Box::into_raw(Box::new(Vec::new()))
}

/// Push a boxed `Value` into the generator accumulator vector.
///
/// # Safety
/// `vec_ptr` must point to a live `Vec<Value>` (from `rt_gen_vec_new`).
/// `value_ptr` must point to a live `Value`.
#[no_mangle]
pub extern "C" fn rt_gen_vec_push(
    _ctx: *mut JitRuntimeCtx,
    vec_ptr: *mut Vec<Value>,
    value_ptr: *mut Value,
) {
    let vec = unsafe { &mut *vec_ptr };
    let value = unsafe { (*value_ptr).clone() };
    vec.push(value);
}

/// Convert a `Vec<Value>` accumulator into a generator fold function.
///
/// Returns `\k -> \z -> foldl k z values` as a `Value::Builtin`.
///
/// # Safety
/// `vec_ptr` must point to a live `Vec<Value>` (from `rt_gen_vec_new`).
/// Ownership of the Vec is taken.
#[no_mangle]
pub extern "C" fn rt_gen_vec_into_generator(
    _ctx: *mut JitRuntimeCtx,
    vec_ptr: *mut Vec<Value>,
) -> *mut Value {
    use crate::runtime::values::{BuiltinImpl, BuiltinValue};

    let values = Arc::new(*unsafe { Box::from_raw(vec_ptr) });
    let builtin = Value::Builtin(BuiltinValue {
        imp: Arc::new(BuiltinImpl {
            name: "<native_generator>".to_string(),
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

/// Convert a generator fold function into a `Value::List` containing all elements.
///
/// The generator is a function `\k -> \z -> foldl k z values`. This helper
/// applies it with a list-append step and empty list to collect all elements.
#[no_mangle]
pub extern "C" fn rt_generator_to_list(
    ctx: *mut JitRuntimeCtx,
    gen_ptr: *mut Value,
) -> *mut Value {
    let gen = unsafe { (*gen_ptr).clone() };
    let runtime = unsafe { &mut *(*ctx).runtime };
    match runtime.generator_to_list(gen) {
        Ok(items) => abi::box_value(Value::List(Arc::new(items))),
        Err(_) => abi::box_value(Value::List(Arc::new(Vec::new()))),
    }
}

// ---------------------------------------------------------------------------
// AOT runtime lifecycle
// ---------------------------------------------------------------------------

use crate::hir::HirProgram;
use crate::runtime::build_runtime_from_program;

/// Initialize an AIVI runtime context from a pre-built HirProgram.
///
/// Returns a heap-allocated `JitRuntimeCtx` pointer that must be passed to
/// all `rt_*` functions. Call `aivi_rt_destroy` when done.
///
/// # Safety
/// The caller must ensure `program_ptr` points to a valid `HirProgram`.
#[no_mangle]
pub extern "C" fn aivi_rt_init(program_ptr: *mut HirProgram) -> *mut JitRuntimeCtx {
    let program = unsafe { Box::from_raw(program_ptr) };
    let runtime =
        build_runtime_from_program(&*program).expect("aivi_rt_init: failed to build runtime");
    let ctx = unsafe { JitRuntimeCtx::from_runtime_owned(runtime) };
    Box::into_raw(Box::new(ctx))
}

/// Destroy a runtime context previously created by `aivi_rt_init`.
///
/// # Safety
/// `ctx` must be a pointer returned by `aivi_rt_init`.
#[no_mangle]
pub extern "C" fn aivi_rt_destroy(ctx: *mut JitRuntimeCtx) {
    if !ctx.is_null() {
        unsafe {
            drop(Box::from_raw(ctx));
        }
    }
}

/// Initialize a minimal AIVI runtime with only builtins (no user program).
/// Used by the AOT path where compiled functions are registered via
/// `rt_register_jit_fn`.
#[no_mangle]
pub extern "C" fn aivi_rt_init_base() -> *mut JitRuntimeCtx {
    use crate::runtime::build_runtime_base;
    let runtime = build_runtime_base();
    let ctx = unsafe { JitRuntimeCtx::from_runtime_owned(runtime) };
    Box::into_raw(Box::new(ctx))
}

/// Register an AOT/JIT-compiled function as a global in the runtime.
/// Does not overwrite existing globals (e.g. builtins).
/// `is_effect`: if non-zero, wraps the function as an `EffectValue::Thunk`.
#[no_mangle]
pub extern "C" fn rt_register_jit_fn(
    ctx: *mut JitRuntimeCtx,
    name_ptr: *const u8,
    name_len: i64,
    func_ptr: i64,
    arity: i64,
    is_effect: i64,
) {
    let name = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(name_ptr, name_len as usize))
    };
    let runtime = unsafe { &mut *(*ctx).runtime };
    // Don't overwrite builtins
    if runtime.ctx.globals.get(name).is_some() {
        return;
    }
    if is_effect != 0 {
        // Wrap zero-arity effect blocks as EffectValue::Thunk
        let def_name = name.to_string();
        let fp = func_ptr as usize;
        let effect = Value::Effect(std::sync::Arc::new(
            crate::runtime::values::EffectValue::Thunk {
                func: std::sync::Arc::new(move |rt: &mut crate::runtime::Runtime| {
                    let ctx = unsafe { JitRuntimeCtx::from_runtime(rt) };
                    let ctx_ptr = &ctx as *const JitRuntimeCtx as usize;
                    let call_args = [ctx_ptr as i64];
                    let result_ptr = unsafe { super::compile::call_jit_function(fp, &call_args) };
                    if result_ptr == 0 {
                        eprintln!("aivi: AOT effect '{}' returned null pointer", def_name);
                        Ok(Value::Unit)
                    } else {
                        Ok(unsafe { abi::unbox_value(result_ptr as *mut Value) })
                    }
                }),
            },
        ));
        runtime.ctx.globals.set(name.to_string(), effect);
    } else {
        let builtin =
            super::compile::make_jit_builtin(name, arity as usize, func_ptr as usize);
        runtime.ctx.globals.set(name.to_string(), builtin);
    }
}

/// Evaluate a sigil literal into the correct runtime value.
/// Dispatches to the shared `eval_sigil_literal` function from the runtime.
#[no_mangle]
pub extern "C" fn rt_eval_sigil(
    _ctx: *mut JitRuntimeCtx,
    tag_ptr: *const u8,
    tag_len: i64,
    body_ptr: *const u8,
    body_len: i64,
    flags_ptr: *const u8,
    flags_len: i64,
) -> *mut Value {
    let tag =
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(tag_ptr, tag_len as usize)) };
    let body =
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(body_ptr, body_len as usize)) };
    let flags =
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(flags_ptr, flags_len as usize)) };
    match crate::runtime::eval_sigil_literal(tag, body, flags) {
        Ok(val) => abi::box_value(val),
        Err(e) => {
            eprintln!("aivi: sigil error: {}", format_runtime_error(e));
            abi::box_value(Value::Unit)
        }
    }
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
        ("rt_alloc_datetime", rt_alloc_datetime as *const u8),
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
        (
            "rt_constructor_name_eq",
            rt_constructor_name_eq as *const u8,
        ),
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
        // Native generate helpers
        (
            "rt_generator_to_list",
            rt_generator_to_list as *const u8,
        ),
        ("rt_gen_vec_new", rt_gen_vec_new as *const u8),
        ("rt_gen_vec_push", rt_gen_vec_push as *const u8),
        (
            "rt_gen_vec_into_generator",
            rt_gen_vec_into_generator as *const u8,
        ),
        // AOT function registration
        (
            "rt_register_jit_fn",
            rt_register_jit_fn as *const u8,
        ),
        // Sigil evaluation
        ("rt_eval_sigil", rt_eval_sigil as *const u8),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: no interpreter-delegation helpers should be registered.
    /// These were removed during the Cranelift migration.
    #[test]
    fn no_interpreter_delegation_symbols() {
        let forbidden = [
            "rt_env_new",
            "rt_env_set",
            "rt_eval_generate",
            "rt_make_resource",
        ];
        let symbols = runtime_helper_symbols();
        for (name, _) in &symbols {
            assert!(
                !forbidden.contains(name),
                "interpreter delegation helper '{name}' should not be in Cranelift symbol table"
            );
        }
    }
}
