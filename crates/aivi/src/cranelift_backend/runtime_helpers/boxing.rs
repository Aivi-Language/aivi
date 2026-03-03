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
