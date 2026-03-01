//! `extern "C"` runtime helpers callable from Cranelift JIT-compiled code.
//!
//! Every helper receives `*mut JitRuntimeCtx` as its first argument.
//! Non-scalar values are passed/returned as `*mut Value` (heap-boxed).

use std::collections::HashMap;
use std::sync::Arc;

use crate::runtime::values::Value;
use crate::runtime::RuntimeError;

use super::abi::{self, JitRuntimeCtx};

// ---------------------------------------------------------------------------
// ANSI color helpers for runtime error reporting
// ---------------------------------------------------------------------------

const RT_YELLOW: &str = "\x1b[1;33m";
const RT_CYAN: &str = "\x1b[1;36m";
const RT_GRAY: &str = "\x1b[90m";
const RT_RESET: &str = "\x1b[0m";
const RT_BOLD: &str = "\x1b[1m";

/// Print a formatted runtime warning to stderr.
fn rt_warn(ctx: *mut JitRuntimeCtx, category: &str, message: &str, hint: &str) {
    let (fn_ctx, loc_ctx) = unsafe {
        let runtime = (*ctx).runtime_mut();
        let fn_part = runtime
            .jit_current_fn
            .as_deref()
            .map(|s| format!(" {RT_GRAY}in `{s}`{RT_RESET}"))
            .unwrap_or_default();
        let loc_part = runtime
            .jit_current_loc
            .as_deref()
            .map(|s| format!(" {RT_GRAY}at {s}{RT_RESET}"))
            .unwrap_or_default();
        (fn_part, loc_part)
    };
    eprintln!("{RT_YELLOW}warning[RT]{RT_RESET}{fn_ctx}{loc_ctx} {RT_BOLD}{category}{RT_RESET}: {message}");
    if !hint.is_empty() {
        eprintln!("  {RT_CYAN}hint{RT_RESET}: {hint}");
    }
}

/// Store a pending error on the runtime context, preserving the first error
/// (root cause) when multiple cascading failures occur within a single JIT call.
unsafe fn set_pending_error(ctx: *mut JitRuntimeCtx, e: RuntimeError) {
    let runtime = (*ctx).runtime_mut();
    if runtime.jit_pending_error.is_none() {
        runtime.jit_pending_error = Some(e);
    }
}

fn set_pending_error_text(ctx: *mut JitRuntimeCtx, message: impl Into<String>) {
    let message = message.into();
    if ctx.is_null() {
        eprintln!(
            "{RT_YELLOW}warning[RT]{RT_RESET} {RT_BOLD}runtime boundary{RT_RESET}: {message}"
        );
        return;
    }
    unsafe {
        set_pending_error(ctx, RuntimeError::Error(Value::Text(message)));
    }
}

fn unit_value() -> *mut Value {
    abi::box_value(Value::Unit)
}

fn reuse_or_unit(token: *mut Value) -> *mut Value {
    if token.is_null() {
        unit_value()
    } else {
        token
    }
}

fn decode_utf8_owned(
    ctx: *mut JitRuntimeCtx,
    ptr: *const u8,
    len: usize,
    label: &str,
) -> Option<String> {
    if len == 0 {
        return Some(String::new());
    }
    if ptr.is_null() {
        set_pending_error_text(ctx, format!("{label}: null UTF-8 pointer"));
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    match std::str::from_utf8(bytes) {
        Ok(text) => Some(text.to_string()),
        Err(err) => {
            set_pending_error_text(ctx, format!("{label}: invalid UTF-8 ({err})"));
            None
        }
    }
}

fn clone_value_array(
    ctx: *mut JitRuntimeCtx,
    items: *const *const Value,
    len: usize,
    label: &str,
) -> Option<Vec<Value>> {
    if len == 0 {
        return Some(Vec::new());
    }
    if items.is_null() {
        set_pending_error_text(ctx, format!("{label}: null value array"));
        return None;
    }
    let mut values = Vec::with_capacity(len);
    for i in 0..len {
        let value_ptr = unsafe { *items.add(i) };
        let Some(value) = (unsafe { value_ptr.as_ref() }) else {
            set_pending_error_text(ctx, format!("{label}: null value pointer at index {i}"));
            return None;
        };
        values.push(value.clone());
    }
    Some(values)
}

fn clone_record_fields(
    ctx: *mut JitRuntimeCtx,
    names: *const *const u8,
    name_lens: *const usize,
    values: *const *const Value,
    len: usize,
    label: &str,
) -> Option<HashMap<String, Value>> {
    if len == 0 {
        return Some(HashMap::new());
    }
    if names.is_null() || name_lens.is_null() || values.is_null() {
        set_pending_error_text(ctx, format!("{label}: null record field arrays"));
        return None;
    }
    let mut map = HashMap::with_capacity(len);
    for i in 0..len {
        let name_ptr = unsafe { *names.add(i) };
        let name_len = unsafe { *name_lens.add(i) };
        let field_name = decode_utf8_owned(ctx, name_ptr, name_len, label)?;
        let value_ptr = unsafe { *values.add(i) };
        let Some(value) = (unsafe { value_ptr.as_ref() }) else {
            set_pending_error_text(ctx, format!("{label}: null value pointer at index {i}"));
            return None;
        };
        map.insert(field_name, value.clone());
    }
    Some(map)
}

// ---------------------------------------------------------------------------
// Function entry tracking — records the current function name for diagnostics
// ---------------------------------------------------------------------------

thread_local! {
    static FN_HISTORY: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Called at the start of every JIT-compiled function to record its name.
/// This makes subsequent runtime warnings show which function triggered them.
#[no_mangle]
pub extern "C" fn rt_enter_fn(ctx: *mut JitRuntimeCtx, ptr: *const u8, len: usize) {
    if ctx.is_null() {
        return;
    }
    let Some(name) = decode_utf8_owned(ctx, ptr, len, "rt_enter_fn") else {
        return;
    };
    let runtime = unsafe { (*ctx).runtime_mut() };
    runtime.jit_current_fn = Some(name.clone().into_boxed_str());
    FN_HISTORY.with(|h| {
        let mut h = h.borrow_mut();
        h.push(name);
        if h.len() > 20 {
            h.remove(0);
        }
    });
}

/// Called before potentially-failing operations to record the source location.
/// This makes subsequent runtime warnings show the source location (line:col).
#[no_mangle]
pub extern "C" fn rt_set_location(ctx: *mut JitRuntimeCtx, ptr: *const u8, len: usize) {
    if ctx.is_null() {
        return;
    }
    let Some(loc) = decode_utf8_owned(ctx, ptr, len, "rt_set_location") else {
        return;
    };
    let runtime = unsafe { (*ctx).runtime_mut() };
    runtime.jit_current_loc = Some(loc.into_boxed_str());
}

// ---------------------------------------------------------------------------
// Call-depth guard helpers — prevent stack overflow from infinite JIT recursion
// ---------------------------------------------------------------------------

/// Increment the call depth counter and return 1 if the limit has been
/// exceeded, 0 otherwise.
#[no_mangle]
pub extern "C" fn rt_check_call_depth(ctx: *mut JitRuntimeCtx) -> i64 {
    let runtime = unsafe { (*ctx).runtime_mut() };
    runtime.jit_call_depth += 1;
    if runtime.jit_call_depth > runtime.jit_max_call_depth {
        1
    } else {
        0
    }
}

/// Decrement the call depth counter (called before each function return).
#[no_mangle]
pub extern "C" fn rt_dec_call_depth(ctx: *mut JitRuntimeCtx) {
    let runtime = unsafe { (*ctx).runtime_mut() };
    runtime.jit_call_depth = runtime.jit_call_depth.saturating_sub(1);
}

/// Signal that a JIT-compiled pattern match was non-exhaustive.
///
/// Sets `runtime.jit_match_failed` so that `make_jit_builtin` can return
/// `Err("non-exhaustive match")`, allowing `apply_multi_clause` to try the
/// next clause.
#[no_mangle]
pub extern "C" fn rt_signal_match_fail(ctx: *mut JitRuntimeCtx) -> *mut Value {
    let runtime = unsafe { (*ctx).runtime_mut() };
    runtime.jit_match_failed = true;
    abi::box_value(Value::Unit)
}

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
            eprintln!(
                "{RT_YELLOW}warning[RT]{RT_RESET} {RT_BOLD}type mismatch{RT_RESET}: expected an integer value, but got `{other:?}`"
            );
            eprintln!("  {RT_CYAN}hint{RT_RESET}: a numeric expression evaluated to the wrong type — check that the value is declared as `Int`");
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
            eprintln!(
                "{RT_YELLOW}warning[RT]{RT_RESET} {RT_BOLD}type mismatch{RT_RESET}: expected a float value, but got `{other:?}`"
            );
            eprintln!("  {RT_CYAN}hint{RT_RESET}: a floating-point expression evaluated to the wrong type — check that the value is declared as `Float`");
            0f64.to_bits() as i64
        }
    }
}

/// Unbox `Value::Bool` → i64 (0 or 1). Returns 0 (false) and logs on type mismatch.
#[no_mangle]
pub extern "C" fn rt_unbox_bool(ctx: *mut JitRuntimeCtx, ptr: *const Value) -> i64 {
    thread_local! {
        static CALL_COUNT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    }
    CALL_COUNT.with(|c| c.set(c.get() + 1));
    let value = unsafe { &*ptr };
    match value {
        Value::Bool(v) => i64::from(*v),
        other => {
            let count = CALL_COUNT.with(|c| c.get());
            rt_warn(
                ctx,
                "type mismatch",
                &format!("expected a boolean (`true`/`false`), but got `{other:?}` (call #{count})"),
                "a condition expression did not produce a Bool — check `if` guards and boolean-typed bindings",
            );
            // Print the call stack of recently entered JIT functions
            let runtime = unsafe { (*ctx).runtime_mut() };
            if let Some(ref fn_name) = runtime.jit_current_fn {
                eprintln!("  current fn: {fn_name}");
            }
            // Print recent function history
            FN_HISTORY.with(|h| {
                let h = h.borrow();
                if !h.is_empty() {
                    eprintln!(
                        "  recent fn entries (last {}): {:?}",
                        h.len(),
                        &h[h.len().saturating_sub(10)..]
                    );
                }
            });
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
    ctx: *mut JitRuntimeCtx,
    ptr: *const u8,
    len: usize,
) -> *mut Value {
    let Some(s) = decode_utf8_owned(ctx, ptr, len, "rt_alloc_string") else {
        return unit_value();
    };
    abi::box_value(Value::Text(s))
}

/// Allocate a `Value::DateTime` from a UTF-8 string.
#[no_mangle]
pub extern "C" fn rt_alloc_datetime(
    ctx: *mut JitRuntimeCtx,
    ptr: *const u8,
    len: usize,
) -> *mut Value {
    let Some(s) = decode_utf8_owned(ctx, ptr, len, "rt_alloc_datetime") else {
        return unit_value();
    };
    abi::box_value(Value::DateTime(s))
}

/// Allocate a `Value::List` from an array of `*const Value` pointers.
///
/// # Safety
/// `items` must point to `len` valid `*const Value` pointers.
#[no_mangle]
pub extern "C" fn rt_alloc_list(
    ctx: *mut JitRuntimeCtx,
    items: *const *const Value,
    len: usize,
) -> *mut Value {
    let Some(values) = clone_value_array(ctx, items, len, "rt_alloc_list") else {
        return unit_value();
    };
    abi::box_value(Value::List(Arc::new(values)))
}

/// Allocate a `Value::Tuple` from an array of `*const Value` pointers.
///
/// # Safety
/// `items` must point to `len` valid `*const Value` pointers.
#[no_mangle]
pub extern "C" fn rt_alloc_tuple(
    ctx: *mut JitRuntimeCtx,
    items: *const *const Value,
    len: usize,
) -> *mut Value {
    let Some(values) = clone_value_array(ctx, items, len, "rt_alloc_tuple") else {
        return unit_value();
    };
    abi::box_value(Value::Tuple(values))
}

/// Allocate a `Value::Record` from parallel arrays of field-name pointers and value pointers.
///
/// # Safety
/// `names` and `values` must each point to `len` valid entries.
/// Each name entry is a `(*const u8, usize)` pair packed as two consecutive pointer-sized values.
#[no_mangle]
pub extern "C" fn rt_alloc_record(
    ctx: *mut JitRuntimeCtx,
    names: *const *const u8,
    name_lens: *const usize,
    values: *const *const Value,
    len: usize,
) -> *mut Value {
    let Some(map) = clone_record_fields(ctx, names, name_lens, values, len, "rt_alloc_record")
    else {
        return unit_value();
    };
    abi::box_value(Value::Record(Arc::new(map)))
}

/// Allocate a `Value::Constructor { name, args }`.
///
/// # Safety
/// `name_ptr`/`name_len` must describe valid UTF-8.
/// `args` must point to `args_len` valid `*const Value` pointers.
#[no_mangle]
pub extern "C" fn rt_alloc_constructor(
    ctx: *mut JitRuntimeCtx,
    name_ptr: *const u8,
    name_len: usize,
    args: *const *const Value,
    args_len: usize,
) -> *mut Value {
    let Some(name) = decode_utf8_owned(ctx, name_ptr, name_len, "rt_alloc_constructor name") else {
        return unit_value();
    };
    let Some(arg_values) = clone_value_array(ctx, args, args_len, "rt_alloc_constructor args")
    else {
        return unit_value();
    };
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
    ctx: *mut JitRuntimeCtx,
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
                rt_warn(
                    ctx,
                    "missing record field",
                    &format!("field `{name}` does not exist on this record"),
                    &format!("check that the record type includes a `{name}` field and that it was correctly constructed"),
                );
                abi::box_value(Value::Unit)
            }
        },
        other => {
            rt_warn(
                ctx,
                "type mismatch",
                &format!("tried to access field `{name}` on a non-record value: `{other:?}`"),
                "this value should be a record — check that the expression producing it returns the correct type",
            );
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
    ctx: *mut JitRuntimeCtx,
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
                    rt_warn(
                        ctx,
                        "index out of bounds",
                        &format!(
                            "list index {index} is out of range (list has {} element{})",
                            list.len(),
                            if list.len() == 1 { "" } else { "s" }
                        ),
                        "ensure the index is within [0, len-1]; use `List.get` for a safe `Option`-returning lookup",
                    );
                    abi::box_value(Value::Unit)
                }
            }
        }
        other => {
            rt_warn(
                ctx,
                "type mismatch",
                &format!("tried to index into a non-list value: `{other:?}`"),
                "this value should be a list — check that the expression producing it returns `List _`",
            );
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

/// Perceus reuse: consume a boxed `Value` and return the raw allocation for
/// reuse. The inner data is dropped (Arcs decremented, Strings freed, etc.)
/// but the `Box<Value>`-sized heap allocation is preserved.
///
/// Returns the pointer (usable as a reuse token) on success, or null if `ptr`
/// is null. The caller must either write a new `Value` into the returned
/// pointer via `rt_reuse_as_*` or free it with `rt_drop_value`.
///
/// # Safety
/// `ptr` must be a valid `*mut Value` from `rt_alloc_*` / `rt_box_*`, and
/// must not be used after this call (its contents are destroyed).
#[no_mangle]
pub extern "C" fn rt_try_reuse(_ctx: *mut JitRuntimeCtx, ptr: *mut Value) -> *mut Value {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        // Drop the inner Value data without deallocating the box.
        // We overwrite the contents with Unit (cheapest variant) to drop
        // whatever was there (Arcs get decremented, Strings freed, etc.).
        std::ptr::drop_in_place(ptr);
        // Write a placeholder so the allocation is in a valid state.
        std::ptr::write(ptr, Value::Unit);
    }
    ptr
}

/// Write a `Constructor` into a reuse token. If `token` is null, allocates fresh.
///
/// # Safety
/// Same as `rt_alloc_constructor`, plus `token` must be either null or a valid
/// reuse token from `rt_try_reuse`.
#[no_mangle]
pub extern "C" fn rt_reuse_constructor(
    ctx: *mut JitRuntimeCtx,
    token: *mut Value,
    name_ptr: *const u8,
    name_len: usize,
    args: *const *const Value,
    args_len: usize,
) -> *mut Value {
    let Some(name) = decode_utf8_owned(ctx, name_ptr, name_len, "rt_reuse_constructor name") else {
        return reuse_or_unit(token);
    };
    let Some(arg_values) = clone_value_array(ctx, args, args_len, "rt_reuse_constructor args")
    else {
        return reuse_or_unit(token);
    };
    let new_value = Value::Constructor {
        name,
        args: arg_values,
    };
    if token.is_null() {
        abi::box_value(new_value)
    } else {
        unsafe {
            std::ptr::write(token, new_value);
        }
        token
    }
}

/// Write a `Record` into a reuse token. If `token` is null, allocates fresh.
#[no_mangle]
pub extern "C" fn rt_reuse_record(
    ctx: *mut JitRuntimeCtx,
    token: *mut Value,
    names: *const *const u8,
    name_lens: *const usize,
    values: *const *const Value,
    len: usize,
) -> *mut Value {
    let Some(map) = clone_record_fields(ctx, names, name_lens, values, len, "rt_reuse_record")
    else {
        return reuse_or_unit(token);
    };
    let new_value = Value::Record(Arc::new(map));
    if token.is_null() {
        abi::box_value(new_value)
    } else {
        unsafe {
            std::ptr::write(token, new_value);
        }
        token
    }
}

/// Write a `List` into a reuse token. If `token` is null, allocates fresh.
#[no_mangle]
pub extern "C" fn rt_reuse_list(
    ctx: *mut JitRuntimeCtx,
    token: *mut Value,
    items: *const *const Value,
    len: usize,
) -> *mut Value {
    let Some(values) = clone_value_array(ctx, items, len, "rt_reuse_list") else {
        return reuse_or_unit(token);
    };
    let new_value = Value::List(Arc::new(values));
    if token.is_null() {
        abi::box_value(new_value)
    } else {
        unsafe {
            std::ptr::write(token, new_value);
        }
        token
    }
}

/// Write a `Tuple` into a reuse token. If `token` is null, allocates fresh.
#[no_mangle]
pub extern "C" fn rt_reuse_tuple(
    ctx: *mut JitRuntimeCtx,
    token: *mut Value,
    items: *const *const Value,
    len: usize,
) -> *mut Value {
    let Some(values) = clone_value_array(ctx, items, len, "rt_reuse_tuple") else {
        return reuse_or_unit(token);
    };
    let new_value = Value::Tuple(values);
    if token.is_null() {
        abi::box_value(new_value)
    } else {
        unsafe {
            std::ptr::write(token, new_value);
        }
        token
    }
}

// ---------------------------------------------------------------------------
// Runtime interaction helpers
// ---------------------------------------------------------------------------

/// Register a value as a named global.  Used by loop desugaring so that
/// recursive `rt_get_global` calls inside the loop body can find the
/// loop closure by its generated name (e.g. `__loop1`).
///
/// # Safety
/// `ctx` must be a valid `JitRuntimeCtx` pointer.  `value_ptr` must be a
/// valid `Value` pointer.
#[no_mangle]
pub extern "C" fn rt_set_global(
    ctx: *mut JitRuntimeCtx,
    name_ptr: *const u8,
    name_len: usize,
    value_ptr: *const Value,
) {
    let name =
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(name_ptr, name_len)) };
    let value = unsafe { (*value_ptr).clone() };
    let runtime = unsafe { (*ctx).runtime_mut() };
    runtime.ctx.globals.set(name.to_string(), value);
}

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
            rt_warn(
                ctx,
                "undefined global",
                &format!("global definition `{name}` was not found"),
                "this may indicate a missing import or a definition that failed to compile",
            );
            return abi::box_value(Value::Unit);
        }
    };
    match runtime.force_value(val) {
        Ok(forced) => abi::box_value(forced),
        Err(e) => {
            unsafe { set_pending_error(ctx, e) };
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
    // Check fuel/cancel to break infinite JIT loops
    {
        let runtime = unsafe { (*ctx).runtime_mut() };
        if runtime.check_cancelled().is_err() {
            return abi::box_value(Value::Unit);
        }
    }

    let func = unsafe { &*func_ptr };
    let arg = unsafe { &*arg_ptr };

    // Fast path: fully-saturated builtin (arity reached with this arg)
    if let Value::Builtin(ref b) = func {
        if b.args.len() + 1 == b.imp.arity {
            let mut all_args = b.args.clone();
            all_args.push(arg.clone());
            let runtime = unsafe { (*ctx).runtime_mut() };
            match (b.imp.func)(all_args, runtime) {
                Ok(val) => {
                    return abi::box_value(val);
                }
                Err(e) => {
                    unsafe { set_pending_error(ctx, e) };
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
            unsafe { set_pending_error(ctx, e) };
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
            unsafe { set_pending_error(ctx, e) };
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
    match runtime.run_effect_value(value.clone()) {
        Ok(val) => abi::box_value(val),
        Err(e) => {
            unsafe { set_pending_error(ctx, e) };
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
                unsafe { set_pending_error(ctx, e) };
                abi::box_value(Value::Unit)
            }
        },
        Err(e) => {
            unsafe { set_pending_error(ctx, e) };
            abi::box_value(Value::Unit)
        }
    }
}

// ---------------------------------------------------------------------------
// Effect wrapping
// ---------------------------------------------------------------------------

/// Wrap a value in an `Effect` thunk so that `rt_run_effect` can consume it.
/// Used at the end of JIT-compiled `do Effect` blocks to ensure the return
/// type is always `Value::Effect`, matching what the caller expects.
///
/// # Safety
/// `ptr` must be a valid `*const Value`.
#[no_mangle]
pub extern "C" fn rt_wrap_effect(_ctx: *mut JitRuntimeCtx, ptr: *const Value) -> *mut Value {
    let value = unsafe { (*ptr).clone() };
    // If it's already an Effect or Source, return as-is to avoid double-wrapping.
    if matches!(value, Value::Effect(_) | Value::Source(_)) {
        return abi::box_value(value);
    }
    let effect = crate::runtime::values::EffectValue::Thunk {
        func: Arc::new(move |_| Ok(value.clone())),
    };
    abi::box_value(Value::Effect(Arc::new(effect)))
}

/// Push a resource scope marker. Called at the start of a do-block.
#[no_mangle]
pub extern "C" fn rt_push_resource_scope(ctx: *mut JitRuntimeCtx) {
    let runtime = unsafe { (*ctx).runtime_mut() };
    runtime.push_resource_scope();
}

/// Pop a resource scope and run all cleanups registered since the last marker.
/// Called at the end of a do-block (before wrapping the result).
#[no_mangle]
pub extern "C" fn rt_pop_resource_scope(ctx: *mut JitRuntimeCtx) {
    let runtime = unsafe { (*ctx).runtime_mut() };
    runtime.pop_resource_scope();
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
    ctx: *mut JitRuntimeCtx,
    ptr: *const Value,
    index: i64,
) -> *mut Value {
    let value = unsafe { &*ptr };
    match value {
        Value::Constructor { args, name } => match args.get(index as usize) {
            Some(v) => abi::box_value(v.clone()),
            None => {
                rt_warn(
                    ctx,
                    "constructor argument out of bounds",
                    &format!(
                        "tried to access argument {index} of constructor `{name}`, but it only has {} argument{}",
                        args.len(),
                        if args.len() == 1 { "" } else { "s" }
                    ),
                    &format!("pattern matching on `{name}` accessed more fields than the constructor carries — check the variant definition"),
                );
                abi::box_value(Value::Unit)
            }
        },
        other => {
            rt_warn(
                ctx,
                "type mismatch",
                &format!("tried to extract a constructor argument, but the value is `{other:?}`"),
                "a pattern match or destructuring expected a constructor (variant) value here — check that the matched expression returns the right type",
            );
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
    ctx: *mut JitRuntimeCtx,
    ptr: *const Value,
    index: i64,
) -> *mut Value {
    let value = unsafe { &*ptr };
    match value {
        Value::Tuple(items) => match items.get(index as usize) {
            Some(v) => abi::box_value(v.clone()),
            None => {
                rt_warn(
                    ctx,
                    "tuple index out of bounds",
                    &format!(
                        "tuple index {index} is out of range (tuple has {} element{})",
                        items.len(),
                        if items.len() == 1 { "" } else { "s" }
                    ),
                    "ensure the tuple destructuring matches the actual tuple size",
                );
                abi::box_value(Value::Unit)
            }
        },
        other => {
            rt_warn(
                ctx,
                "type mismatch",
                &format!("tried to index a tuple, but the value is `{other:?}`"),
                "a tuple destructuring pattern expected a tuple value here",
            );
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

/// Concatenate two lists into a new list.
#[no_mangle]
pub extern "C" fn rt_list_concat(
    _ctx: *mut JitRuntimeCtx,
    a: *const Value,
    b: *const Value,
) -> *mut Value {
    let va = unsafe { &*a };
    let vb = unsafe { &*b };
    let mut result = match va {
        Value::List(items) => items.as_ref().clone(),
        _ => Vec::new(),
    };
    if let Value::List(items) = vb {
        result.extend(items.iter().cloned())
    }
    abi::box_value(Value::List(Arc::new(result)))
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
    let result = crate::runtime::values_equal(va, vb);
    i64::from(result)
}

// ---------------------------------------------------------------------------
// Record patching helper
// ---------------------------------------------------------------------------

/// Patch a record: clone the base record and overlay new fields.
///
/// Apply patch fields to a record map. When a patch value is callable
/// (a function/closure), it is applied to the field's current value as a
/// transform; otherwise the value replaces the field directly.
fn apply_patch_fields(
    ctx: *mut JitRuntimeCtx,
    map: &mut HashMap<String, Value>,
    patch_fields: HashMap<String, Value>,
) {
    for (name, value) in patch_fields {
        if is_patch_transform(&value) {
            if let Some(current) = map.get(&name) {
                let func_box = abi::box_value(value);
                let arg_box = abi::box_value(current.clone());
                let result_ptr = rt_apply(ctx, func_box, arg_box);
                let result = unsafe { (*result_ptr).clone() };
                unsafe { drop(Box::from_raw(func_box)) };
                unsafe { drop(Box::from_raw(arg_box)) };
                unsafe { drop(Box::from_raw(result_ptr)) };
                map.insert(name, result);
            } else {
                map.insert(name, value);
            }
        } else {
            map.insert(name, value);
        }
    }
}

/// Returns true when the value is callable and should be treated as a
/// transform (applied to the current field value) rather than a replacement.
fn is_patch_transform(value: &Value) -> bool {
    matches!(value, Value::Builtin(_) | Value::MultiClause(_))
}

/// # Safety
/// `base_ptr` must be a valid `Value::Record` (or any Value — non-records
/// produce a fresh record).  `names`, `name_lens`, `values` arrays must
/// each have `len` entries.
#[no_mangle]
pub extern "C" fn rt_patch_record(
    ctx: *mut JitRuntimeCtx,
    base_ptr: *const Value,
    names: *const *const u8,
    name_lens: *const usize,
    values: *const *const Value,
    len: usize,
) -> *mut Value {
    if base_ptr.is_null() {
        set_pending_error_text(ctx, "rt_patch_record: null base value pointer");
        return unit_value();
    }
    let base = unsafe { &*base_ptr };
    let mut map = match base {
        Value::Record(rec) => (**rec).clone(),
        _ => HashMap::new(),
    };
    let Some(patch_fields) =
        clone_record_fields(ctx, names, name_lens, values, len, "rt_patch_record")
    else {
        return unit_value();
    };
    apply_patch_fields(ctx, &mut map, patch_fields);
    abi::box_value(Value::Record(Arc::new(map)))
}

/// Perceus in-place record patching. If the base record's `Arc<HashMap>` has a
/// strong count of 1, we mutate it in-place and reuse the box allocation.
/// Otherwise falls back to clone-and-patch like `rt_patch_record`.
///
/// `base_ptr` is consumed (caller must not use it again).
#[no_mangle]
pub extern "C" fn rt_patch_record_inplace(
    ctx: *mut JitRuntimeCtx,
    base_ptr: *mut Value,
    names: *const *const u8,
    name_lens: *const usize,
    values: *const *const Value,
    len: usize,
) -> *mut Value {
    if base_ptr.is_null() {
        set_pending_error_text(ctx, "rt_patch_record_inplace: null base value pointer");
        return unit_value();
    }
    let Some(patch_fields) = clone_record_fields(
        ctx,
        names,
        name_lens,
        values,
        len,
        "rt_patch_record_inplace",
    ) else {
        return base_ptr;
    };
    let base = unsafe { &mut *base_ptr };
    let can_reuse = matches!(base, Value::Record(ref arc) if Arc::strong_count(arc) == 1);

    if can_reuse {
        // Safe: we have the only reference, so Arc::get_mut will succeed.
        if let Value::Record(ref mut arc) = base {
            if let Some(map) = Arc::get_mut(arc) {
                apply_patch_fields(ctx, map, patch_fields);
                return base_ptr;
            }
        }
    }

    // Fall back: clone the HashMap, patch, write back into the same box.
    let mut map = match base {
        Value::Record(rec) => (**rec).clone(),
        _ => HashMap::new(),
    };
    apply_patch_fields(ctx, &mut map, patch_fields);
    // Overwrite in place so the caller can still free the same pointer.
    *base = Value::Record(Arc::new(map));
    base_ptr
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
    ctx: *mut JitRuntimeCtx,
    func_ptr: i64,
    captured: *const *const Value,
    captured_count: i64,
) -> *mut Value {
    use crate::runtime::values::{BuiltinImpl, BuiltinValue};
    use crate::runtime::Runtime;
    use std::sync::Arc;

    let count = captured_count as usize;
    let Some(captured_values) = clone_value_array(ctx, captured, count, "rt_make_closure captured")
    else {
        return unit_value();
    };

    let builtin = Value::Builtin(BuiltinValue {
        imp: Arc::new(BuiltinImpl {
            name: format!("__jit_closure_{:#x}", func_ptr),
            arity: 1,
            func: Arc::new(move |args: Vec<Value>, runtime: &mut Runtime| {
                let arg = args.into_iter().next().unwrap_or(Value::Unit);
                // Clear any stale pending error before entering JIT code
                runtime.jit_pending_error = None;
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
                    eprintln!(
                        "{RT_YELLOW}warning[RT]{RT_RESET} {RT_BOLD}null return{RT_RESET}: a JIT closure returned a null pointer (treated as unit)"
                    );
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

                // Propagate any error that occurred inside JIT code
                if let Some(err) = runtime.jit_pending_error.take() {
                    return Err(err);
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

    if op == "==" {
        // Equality logic...
        let eq = crate::runtime::values_equal(&lhs, &rhs);
        return abi::box_value(Value::Bool(eq));
    }

    // Fast path: try the pure built-in evaluation
    if let Some(result) = crate::runtime::eval_binary_builtin(op, &lhs, &rhs) {
        return abi::box_value(result);
    }

    // Slow path: look up operator in globals and apply curried
    let runtime = unsafe { (*ctx).runtime_mut() };
    let op_name = format!("({})", op);
    if let Some(op_value) = runtime.ctx.globals.get(&op_name) {
        if let Ok(applied) = runtime.apply(op_value, lhs.clone()) {
            if let Ok(result) = runtime.apply(applied, rhs.clone()) {
                return abi::box_value(result);
            }
        }
    }
    eprintln!(
        "{RT_YELLOW}warning[RT]{RT_RESET} {RT_BOLD}operator error{RT_RESET}: binary operator `{op}` could not be applied to the given operand types"
    );
    eprintln!(
        "  {RT_CYAN}hint{RT_RESET}: check that both operands have compatible types for `{op}`"
    );
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

/// Extend the generator accumulator with elements from a generator value.
///
/// If `value_ptr` points to a generator (fold function), it is folded and
/// all yielded elements are pushed into `vec_ptr`. Non-generator values
/// (e.g. `Unit` from side-effectful expressions) are silently ignored.
///
/// # Safety
/// `vec_ptr` must point to a live `Vec<Value>` (from `rt_gen_vec_new`).
/// `value_ptr` must point to a live `Value`.
#[no_mangle]
pub extern "C" fn rt_gen_vec_extend_generator(
    ctx: *mut JitRuntimeCtx,
    vec_ptr: *mut Vec<Value>,
    value_ptr: *mut Value,
) {
    let runtime = unsafe { &mut *(*ctx).runtime };
    let value = unsafe { (*value_ptr).clone() };
    let vec = unsafe { &mut *vec_ptr };
    if let Ok(items) = runtime.generator_to_list(value) {
        vec.extend(items);
    }
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
    ctx: *mut JitRuntimeCtx,
    vec_ptr: *mut Vec<Value>,
) -> *mut Value {
    use crate::runtime::values::{BuiltinImpl, BuiltinValue};

    if vec_ptr.is_null() {
        set_pending_error_text(
            ctx,
            "rt_gen_vec_into_generator: null generator buffer pointer",
        );
        return unit_value();
    }
    let values = Arc::new(*unsafe { Box::from_raw(vec_ptr) });
    let builtin = Value::Builtin(BuiltinValue {
        imp: Arc::new(BuiltinImpl {
            name: "<native_generator>".to_string(),
            arity: 2,
            func: Arc::new(move |mut args, runtime| {
                let Some(z) = args.pop() else {
                    return Err(RuntimeError::Message(
                        "native generator expected 2 arguments (missing seed)".to_string(),
                    ));
                };
                let Some(k) = args.pop() else {
                    return Err(RuntimeError::Message(
                        "native generator expected 2 arguments (missing step function)".to_string(),
                    ));
                };
                if !args.is_empty() {
                    return Err(RuntimeError::Message(
                        "native generator expected exactly 2 arguments".to_string(),
                    ));
                }
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
pub extern "C" fn rt_generator_to_list(ctx: *mut JitRuntimeCtx, gen_ptr: *mut Value) -> *mut Value {
    let gen = unsafe { (*gen_ptr).clone() };
    let runtime = unsafe { &mut *(*ctx).runtime };
    match runtime.generator_to_list(gen) {
        Ok(items) => abi::box_value(Value::List(Arc::new(items))),
        Err(e) => {
            unsafe { set_pending_error(ctx, e) };
            abi::box_value(Value::Unit)
        }
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
    if program_ptr.is_null() {
        eprintln!(
            "{RT_YELLOW}warning[RT]{RT_RESET} {RT_BOLD}runtime init{RT_RESET}: null program pointer"
        );
        return std::ptr::null_mut();
    }
    let program = unsafe { Box::from_raw(program_ptr) };
    let runtime = match build_runtime_from_program(&program) {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!(
                "{RT_YELLOW}warning[RT]{RT_RESET} {RT_BOLD}runtime init{RT_RESET}: failed to build runtime: {err}"
            );
            return std::ptr::null_mut();
        }
    };
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
                    rt.jit_pending_error = None;
                    let ctx = unsafe { JitRuntimeCtx::from_runtime(rt) };
                    let ctx_ptr = &ctx as *const JitRuntimeCtx as usize;
                    let call_args = [ctx_ptr as i64];
                    let result_ptr = unsafe { super::compile::call_jit_function(fp, &call_args) };
                    let result = if result_ptr == 0 {
                        eprintln!(
                            "{RT_YELLOW}warning[RT]{RT_RESET} {RT_BOLD}null return{RT_RESET}: AOT effect `{}` returned a null pointer (treated as unit)",
                            def_name
                        );
                        Value::Unit
                    } else {
                        unsafe { abi::unbox_value(result_ptr as *mut Value) }
                    };
                    if let Some(err) = rt.jit_pending_error.take() {
                        return Err(err);
                    }
                    Ok(result)
                }),
            },
        ));
        runtime.ctx.globals.set(name.to_string(), effect);
    } else {
        let builtin = super::compile::make_jit_builtin(name, arity as usize, func_ptr as usize);
        runtime.ctx.globals.set(name.to_string(), builtin);
    }
}

/// Evaluate a sigil literal into the correct runtime value.
/// Dispatches to the shared `eval_sigil_literal` function from the runtime.
#[no_mangle]
pub extern "C" fn rt_eval_sigil(
    ctx: *mut JitRuntimeCtx,
    tag_ptr: *const u8,
    tag_len: i64,
    body_ptr: *const u8,
    body_len: i64,
    flags_ptr: *const u8,
    flags_len: i64,
) -> *mut Value {
    let tag = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(tag_ptr, tag_len as usize))
    };
    let body = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(body_ptr, body_len as usize))
    };
    let flags = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(flags_ptr, flags_len as usize))
    };
    match crate::runtime::eval_sigil_literal(tag, body, flags) {
        Ok(val) => abi::box_value(val),
        Err(e) => {
            unsafe { set_pending_error(ctx, e) };
            abi::box_value(Value::Unit)
        }
    }
}

// ---------------------------------------------------------------------------
// AOT machine registration
// ---------------------------------------------------------------------------

/// Register machine declarations from a binary blob into the runtime globals.
///
/// Called by the AOT entry point `__aivi_main` to register machine transition
/// builtins, `can` predicates, `currentState`, and the machine record itself —
/// exactly as `register_machines_for_jit` does at JIT startup.
///
/// Binary format (all lengths as little-endian u32):
/// ```text
/// n_machines: u32
/// for each machine:
///   qual_name_len: u32  qual_name: [u8]
///   initial_state_len: u32  initial_state: [u8]
///   n_states: u32
///   for each state:  state_len: u32  state: [u8]
///   n_transitions: u32
///   for each transition:
///     event_len: u32  event: [u8]
///     source_len: u32  source: [u8]  (empty for init transitions)
///     target_len: u32  target: [u8]
/// ```
#[no_mangle]
pub extern "C" fn rt_register_machines_from_data(
    ctx: *mut JitRuntimeCtx,
    data_ptr: *const u8,
    data_len: usize,
) {
    use crate::runtime::environment::MachineEdge;
    use crate::runtime::{
        make_machine_can_builtin, make_machine_current_state_builtin,
        make_machine_transition_builtin,
    };
    use std::collections::HashMap;
    use std::sync::Arc;

    let data = unsafe { std::slice::from_raw_parts(data_ptr, data_len) };
    let mut pos = 0usize;

    macro_rules! read_u32 {
        () => {{
            if pos + 4 > data.len() {
                return;
            }
            let v = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            pos += 4;
            v as usize
        }};
    }

    macro_rules! read_str {
        () => {{
            let len = read_u32!();
            if pos + len > data.len() {
                return;
            }
            let s = match std::str::from_utf8(&data[pos..pos + len]) {
                Ok(s) => s.to_string(),
                Err(_) => return,
            };
            pos += len;
            s
        }};
    }

    let n_machines = read_u32!();
    let runtime = unsafe { (*ctx).runtime_mut() };
    let globals = &runtime.ctx.globals;

    for _ in 0..n_machines {
        let qual_name = read_str!();
        let initial_state = read_str!();

        let n_states = read_u32!();
        let mut state_names: Vec<String> = Vec::with_capacity(n_states);
        for _ in 0..n_states {
            state_names.push(read_str!());
        }

        let n_transitions = read_u32!();
        let mut transitions: HashMap<String, Vec<MachineEdge>> = HashMap::new();
        for _ in 0..n_transitions {
            let event = read_str!();
            let source_raw = read_str!();
            let target = read_str!();
            let source = if source_raw.is_empty() {
                None
            } else {
                Some(source_raw)
            };
            transitions
                .entry(event)
                .or_default()
                .push(MachineEdge { source, target });
        }

        // Derive short machine name and module name from the qualified name.
        // e.g. "mailfox.main.ComposeView" -> short = "ComposeView", module = "mailfox.main"
        let short_machine_name = qual_name
            .rsplit('.')
            .next()
            .unwrap_or(&qual_name)
            .to_string();
        let module_name = qual_name
            .rsplit_once('.')
            .map(|x| x.0)
            .unwrap_or(&qual_name)
            .to_string();

        // Register state constructors
        for state_name in &state_names {
            let state_ctor = Value::Constructor {
                name: state_name.clone(),
                args: Vec::new(),
            };
            globals.set(state_name.clone(), state_ctor.clone());
            let qualified = format!("{module_name}.{state_name}");
            if globals.get(&qualified).is_none() {
                globals.set(qualified, state_ctor);
            }
        }

        // Build machine record fields
        let mut machine_fields: HashMap<String, Value> = HashMap::new();
        let mut can_fields: HashMap<String, Value> = HashMap::new();
        let mut event_names: Vec<String> = transitions.keys().cloned().collect();
        event_names.sort();

        for event_name in &event_names {
            let transition_value =
                make_machine_transition_builtin(qual_name.clone(), event_name.clone());
            machine_fields.insert(event_name.clone(), transition_value.clone());
            globals.set(event_name.clone(), transition_value.clone());
            let qualified_transition = format!("{module_name}.{event_name}");
            if globals.get(&qualified_transition).is_none() {
                globals.set(qualified_transition, transition_value);
            }
            can_fields.insert(
                event_name.clone(),
                make_machine_can_builtin(qual_name.clone(), event_name.clone()),
            );
        }

        machine_fields.insert(
            "currentState".to_string(),
            make_machine_current_state_builtin(qual_name.clone()),
        );
        machine_fields.insert("can".to_string(), Value::Record(Arc::new(can_fields)));
        let machine_value = Value::Record(Arc::new(machine_fields));
        globals.set(short_machine_name.clone(), machine_value.clone());
        let qualified_machine = qual_name.clone();
        if globals.get(&qualified_machine).is_none() {
            globals.set(qualified_machine, machine_value);
        }

        // Register machine spec in RuntimeContext
        runtime
            .ctx
            .register_machine(qual_name, initial_state, transitions);
    }
}

// ---------------------------------------------------------------------------
// Snapshot mock helpers
// ---------------------------------------------------------------------------

/// Install a snapshot mock for a global binding.
///
/// - **Recording mode** (`--update-snapshots`): wraps the real function in a
///   proxy that records every Effect result to `runtime.snapshot_recordings`.
/// - **Replay mode** (default): loads recorded values from the snapshot file
///   on disk and installs a function that returns them in order.
///
/// Returns a pointer to the **old** global value (for later restoration).
///
/// # Safety
/// `ctx` must be a valid `JitRuntimeCtx` pointer.
#[no_mangle]
pub extern "C" fn rt_snapshot_mock_install(
    ctx: *mut JitRuntimeCtx,
    path_ptr: *const u8,
    path_len: usize,
) -> *mut Value {
    use crate::runtime::snapshot;
    use crate::runtime::values::{BuiltinImpl, BuiltinValue, EffectValue};

    let path =
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(path_ptr, path_len)) };
    let runtime = unsafe { (*ctx).runtime_mut() };

    // Save old value
    let old_val = match runtime.ctx.globals.get(path) {
        Some(v) => match runtime.force_value(v) {
            Ok(forced) => forced,
            Err(e) => {
                unsafe { set_pending_error(ctx, e) };
                return abi::box_value(Value::Unit);
            }
        },
        None => Value::Unit,
    };

    let path_owned = path.to_string();

    if runtime.update_snapshots {
        // --- Recording mode ---
        runtime
            .snapshot_recordings
            .entry(path_owned.clone())
            .or_default();

        let original = old_val.clone();
        let rec_path = path_owned.clone();
        let wrapper = Value::Builtin(BuiltinValue {
            imp: Arc::new(BuiltinImpl {
                name: format!("__snapshot_record_{rec_path}"),
                arity: 1,
                func: Arc::new(move |args, rt| {
                    let result = rt.apply(original.clone(), args[0].clone())?;
                    match result {
                        Value::Effect(eff) => {
                            let rp = rec_path.clone();
                            Ok(Value::Effect(Arc::new(EffectValue::Thunk {
                                func: Arc::new(move |rt2| {
                                    let val = match eff.as_ref() {
                                        EffectValue::Thunk { func } => func(rt2)?,
                                    };
                                    let json = snapshot::value_to_snapshot_json(&val)?;
                                    rt2.snapshot_recordings
                                        .entry(rp.clone())
                                        .or_default()
                                        .push(json.to_string());
                                    Ok(val)
                                }),
                            })))
                        }
                        other => {
                            let json = snapshot::value_to_snapshot_json(&other)?;
                            rt.snapshot_recordings
                                .entry(rec_path.clone())
                                .or_default()
                                .push(json.to_string());
                            Ok(other)
                        }
                    }
                }),
            }),
            args: vec![],
            tagged_args: None,
        });

        runtime.ctx.globals.set(path_owned, wrapper);
    } else {
        // --- Replay mode ---
        let test_name = runtime.current_test_name.clone().unwrap_or_default();
        let project_root = runtime.project_root.clone();

        let Some(root) = project_root else {
            unsafe {
                set_pending_error(
                    ctx,
                    RuntimeError::Message("snapshot: project root not set".to_string()),
                )
            };
            return abi::box_value(old_val);
        };

        let snap_dir = snapshot::snapshot_dir(&root, &test_name);
        let snap_path = snap_dir.join(format!("{}.snap", path_owned.replace('.', "_")));

        if !snap_path.exists() {
            unsafe {
                set_pending_error(
                    ctx,
                    RuntimeError::Message(format!(
                        "snapshot file not found: {} — run with --update-snapshots to create it",
                        snap_path.display()
                    )),
                )
            };
            return abi::box_value(old_val);
        }

        let contents = match std::fs::read_to_string(&snap_path) {
            Ok(c) => c,
            Err(e) => {
                unsafe {
                    set_pending_error(
                        ctx,
                        RuntimeError::Message(format!(
                            "snapshot: failed to read {}: {e}",
                            snap_path.display()
                        )),
                    )
                };
                return abi::box_value(old_val);
            }
        };

        let entries: Vec<serde_json::Value> = match serde_json::from_str(&contents) {
            Ok(v) => v,
            Err(e) => {
                unsafe {
                    set_pending_error(
                        ctx,
                        RuntimeError::Message(format!(
                            "snapshot: failed to parse {}: {e}",
                            snap_path.display()
                        )),
                    )
                };
                return abi::box_value(old_val);
            }
        };

        let values: Result<Vec<Value>, _> = entries
            .iter()
            .map(snapshot::snapshot_json_to_value)
            .collect();
        let values = match values {
            Ok(v) => v,
            Err(e) => {
                unsafe { set_pending_error(ctx, e) };
                return abi::box_value(old_val);
            }
        };

        let replay_values = Arc::new(std::sync::Mutex::new(values));
        let rp = path_owned.clone();
        let wrapper = Value::Builtin(BuiltinValue {
            imp: Arc::new(BuiltinImpl {
                name: format!("__snapshot_replay_{rp}"),
                arity: 1,
                func: Arc::new(move |_args, _rt| {
                    let mut vals = replay_values.lock().unwrap();
                    if vals.is_empty() {
                        return Err(RuntimeError::Message(format!(
                            "snapshot replay exhausted for `{rp}` — run with --update-snapshots to re-record"
                        )));
                    }
                    let val = vals.remove(0);
                    Ok(Value::Effect(Arc::new(EffectValue::Thunk {
                        func: Arc::new(move |_| Ok(val.clone())),
                    })))
                }),
            }),
            args: vec![],
            tagged_args: None,
        });

        runtime.ctx.globals.set(path_owned, wrapper);
    }

    abi::box_value(old_val)
}

/// Flush snapshot recordings to disk for a mock binding.
/// Only writes in recording mode; no-op in replay mode.
///
/// # Safety
/// `ctx` must be a valid `JitRuntimeCtx` pointer.
#[no_mangle]
pub extern "C" fn rt_snapshot_mock_flush(
    ctx: *mut JitRuntimeCtx,
    path_ptr: *const u8,
    path_len: usize,
) {
    use crate::runtime::snapshot;

    let path =
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(path_ptr, path_len)) };
    let runtime = unsafe { (*ctx).runtime_mut() };

    if !runtime.update_snapshots {
        return;
    }

    let test_name = runtime.current_test_name.clone().unwrap_or_default();
    let project_root = runtime.project_root.clone();

    let Some(root) = project_root else {
        return;
    };

    if let Some(recordings) = runtime.snapshot_recordings.remove(path) {
        let snap_dir = snapshot::snapshot_dir(&root, &test_name);
        let snap_path = snap_dir.join(format!("{}.snap", path.replace('.', "_")));

        if let Err(e) = std::fs::create_dir_all(&snap_dir) {
            unsafe {
                set_pending_error(
                    ctx,
                    RuntimeError::Message(format!(
                        "snapshot: failed to create directory {}: {e}",
                        snap_dir.display()
                    )),
                )
            };
            return;
        }

        let json_entries: Vec<serde_json::Value> = recordings
            .iter()
            .filter_map(|s| serde_json::from_str(s).ok())
            .collect();
        let json_str = serde_json::to_string_pretty(&json_entries).unwrap_or_default();

        if let Err(e) = std::fs::write(&snap_path, json_str) {
            unsafe {
                set_pending_error(
                    ctx,
                    RuntimeError::Message(format!(
                        "snapshot: failed to write {}: {e}",
                        snap_path.display()
                    )),
                )
            };
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
        // Call-depth guard
        ("rt_check_call_depth", rt_check_call_depth as *const u8),
        ("rt_dec_call_depth", rt_dec_call_depth as *const u8),
        // Match failure signaling
        ("rt_signal_match_fail", rt_signal_match_fail as *const u8),
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
        // Perceus reuse helpers
        ("rt_try_reuse", rt_try_reuse as *const u8),
        ("rt_reuse_constructor", rt_reuse_constructor as *const u8),
        ("rt_reuse_record", rt_reuse_record as *const u8),
        ("rt_reuse_list", rt_reuse_list as *const u8),
        ("rt_reuse_tuple", rt_reuse_tuple as *const u8),
        ("rt_set_global", rt_set_global as *const u8),
        ("rt_get_global", rt_get_global as *const u8),
        ("rt_apply", rt_apply as *const u8),
        ("rt_force_thunk", rt_force_thunk as *const u8),
        ("rt_run_effect", rt_run_effect as *const u8),
        ("rt_bind_effect", rt_bind_effect as *const u8),
        ("rt_wrap_effect", rt_wrap_effect as *const u8),
        (
            "rt_push_resource_scope",
            rt_push_resource_scope as *const u8,
        ),
        ("rt_pop_resource_scope", rt_pop_resource_scope as *const u8),
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
        ("rt_list_concat", rt_list_concat as *const u8),
        ("rt_value_equals", rt_value_equals as *const u8),
        // Record patching
        ("rt_patch_record", rt_patch_record as *const u8),
        (
            "rt_patch_record_inplace",
            rt_patch_record_inplace as *const u8,
        ),
        // Closure creation
        ("rt_make_closure", rt_make_closure as *const u8),
        // Native generate helpers
        ("rt_generator_to_list", rt_generator_to_list as *const u8),
        ("rt_gen_vec_new", rt_gen_vec_new as *const u8),
        ("rt_gen_vec_push", rt_gen_vec_push as *const u8),
        (
            "rt_gen_vec_extend_generator",
            rt_gen_vec_extend_generator as *const u8,
        ),
        (
            "rt_gen_vec_into_generator",
            rt_gen_vec_into_generator as *const u8,
        ),
        // AOT function registration
        ("rt_register_jit_fn", rt_register_jit_fn as *const u8),
        // AOT machine registration
        (
            "rt_register_machines_from_data",
            rt_register_machines_from_data as *const u8,
        ),
        // Sigil evaluation
        ("rt_eval_sigil", rt_eval_sigil as *const u8),
        // Function entry tracking for diagnostics
        ("rt_enter_fn", rt_enter_fn as *const u8),
        // Snapshot mock helpers
        (
            "rt_snapshot_mock_install",
            rt_snapshot_mock_install as *const u8,
        ),
        (
            "rt_snapshot_mock_flush",
            rt_snapshot_mock_flush as *const u8,
        ),
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

    /// Build surface modules from a minimal AIVI source containing one machine declaration.
    fn make_test_surface_modules() -> Vec<crate::surface::Module> {
        use std::path::Path;
        let source = r#"
module test.mod

machine Counter = {
  -> Idle : init {}
  Idle -> Running : start {}
  Running -> Idle : stop {}
}

main = do Effect { unit }
"#;
        let (modules, diags) = crate::surface::parse_modules(Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "parse errors: {diags:?}");
        modules
    }

    /// Collect the names of machine-related globals registered by the given runtime
    /// (excludes builtins that exist before machine registration).
    fn machine_global_names(runtime: &crate::runtime::Runtime) -> Vec<String> {
        let mut names: Vec<String> = runtime
            .ctx
            .globals
            .keys()
            .into_iter()
            .filter(|k: &String| {
                !matches!(
                    k.as_str(),
                    "True" | "False" | "Some" | "None" | "Ok" | "Err" | "Closed" | "__machine_on"
                ) && !k.starts_with("aivi.")
                    && !k.starts_with("__")
            })
            .collect();
        names.sort();
        names
    }

    /// Verify that `rt_register_machines_from_data` (AOT path) registers exactly
    /// the same globals as `register_machines_for_jit` (JIT path) for a given
    /// surface module.
    #[test]
    fn aot_machine_globals_parity_with_jit() {
        use crate::cranelift_backend::compile::serialize_machine_data;
        use crate::runtime::{build_runtime_base, register_machines_for_jit};

        let surface_modules = make_test_surface_modules();

        // --- JIT path ---
        let jit_runtime = build_runtime_base();
        register_machines_for_jit(&jit_runtime, &surface_modules);
        let jit_globals = machine_global_names(&jit_runtime);

        // --- AOT path: serialize → rt_register_machines_from_data ---
        let aot_runtime = build_runtime_base();
        let data = serialize_machine_data(&surface_modules);
        let mut aot_ctx = unsafe { JitRuntimeCtx::from_runtime_owned(aot_runtime) };
        rt_register_machines_from_data(&mut aot_ctx as *mut _, data.as_ptr(), data.len());
        let aot_globals = unsafe {
            let runtime = (*(&mut aot_ctx as *mut JitRuntimeCtx)).runtime_mut();
            machine_global_names(runtime)
        };

        let jit_only: Vec<&String> = jit_globals
            .iter()
            .filter(|g| !aot_globals.contains(g))
            .collect();
        let aot_only: Vec<&String> = aot_globals
            .iter()
            .filter(|g| !jit_globals.contains(g))
            .collect();
        assert!(
            jit_only.is_empty() && aot_only.is_empty(),
            "JIT and AOT machine registration produced different globals.\n\
             JIT only: {jit_only:?}\n\
             AOT only: {aot_only:?}",
        );
    }

    /// Verify specific globals are registered by `rt_register_machines_from_data`.
    #[test]
    fn aot_machine_registration_expected_globals() {
        use crate::cranelift_backend::compile::serialize_machine_data;
        use crate::runtime::build_runtime_base;

        let surface_modules = make_test_surface_modules();
        let aot_runtime = build_runtime_base();
        let data = serialize_machine_data(&surface_modules);
        let mut aot_ctx = unsafe { JitRuntimeCtx::from_runtime_owned(aot_runtime) };
        rt_register_machines_from_data(&mut aot_ctx as *mut _, data.as_ptr(), data.len());

        let runtime = unsafe { (*(&mut aot_ctx as *mut JitRuntimeCtx)).runtime_mut() };

        // State constructors (short + qualified with module_name, NOT with machine qual_name)
        let expected = [
            "Idle",
            "Running",
            "test.mod.Idle",    // module_name.state — NOT "test.mod.Counter.Idle"
            "test.mod.Running", // module_name.state — NOT "test.mod.Counter.Running"
            // Transition builtins (short + qualified with module_name)
            "init",
            "start",
            "stop",
            "test.mod.init", // module_name.event
            "test.mod.start",
            "test.mod.stop",
            // Machine record (short + qualified)
            "Counter",
            "test.mod.Counter",
        ];
        for name in &expected {
            assert!(
                runtime.ctx.globals.get(name).is_some(),
                "expected global `{name}` to be registered after rt_register_machines_from_data",
            );
        }

        // Must NOT register with full qual_name prefix (the bug we caught)
        let forbidden = [
            "test.mod.Counter.Idle",
            "test.mod.Counter.Running",
            "test.mod.Counter.init",
            "test.mod.Counter.start",
            "test.mod.Counter.stop",
        ];
        for name in &forbidden {
            assert!(
                runtime.ctx.globals.get(name).is_none(),
                "global `{name}` should NOT be registered (wrong qualified prefix)",
            );
        }
    }

    /// Verify `rt_register_machines_from_data` is in the JIT symbol table
    /// so it can be called from AOT-compiled entry points.
    #[test]
    fn rt_register_machines_from_data_in_symbol_table() {
        let symbols = runtime_helper_symbols();
        let names: Vec<&str> = symbols.iter().map(|(n, _)| *n).collect();
        assert!(
            names.contains(&"rt_register_machines_from_data"),
            "rt_register_machines_from_data must be in the runtime helper symbol table"
        );
    }

    /// Guard that `build_runtime_base()` (used by AOT) registers every global that
    /// `build_runtime_from_program()` (used by JIT) registers for a minimal program.
    ///
    /// If someone adds something to `build_runtime_from_program` that is missing from
    /// `build_runtime_base`, this test will name the missing globals so they can be
    /// backfilled into `build_runtime_base` (or its AOT equivalent).
    #[test]
    fn aot_base_runtime_globals_subset_of_jit_program_runtime() {
        use crate::hir::{HirDef, HirExpr, HirModule, HirProgram};
        use crate::runtime::{build_runtime_base, build_runtime_from_program};

        // Minimal program: one module, one dummy def — just enough to pass the
        // "no modules" guard inside build_runtime_from_program.
        let program = HirProgram {
            modules: vec![HirModule {
                name: "test".to_string(),
                defs: vec![HirDef {
                    name: "main".to_string(),
                    expr: HirExpr::LitBool { id: 0, value: true },
                }],
            }],
        };

        let jit_runtime =
            build_runtime_from_program(&program).expect("build_runtime_from_program failed");
        let aot_runtime = build_runtime_base();

        let jit_keys: std::collections::HashSet<String> =
            jit_runtime.ctx.globals.keys().into_iter().collect();
        let aot_keys: std::collections::HashSet<String> =
            aot_runtime.ctx.globals.keys().into_iter().collect();

        let missing_from_aot: Vec<&String> = jit_keys
            .iter()
            .filter(|k| !aot_keys.contains(*k))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        assert!(
            missing_from_aot.is_empty(),
            "build_runtime_base() is missing globals that build_runtime_from_program() registers.\n\
             These will be absent in AOT binaries: {missing_from_aot:?}\n\
             Add them to build_runtime_base() or to rt_register_machines_from_data.",
        );
    }
}
