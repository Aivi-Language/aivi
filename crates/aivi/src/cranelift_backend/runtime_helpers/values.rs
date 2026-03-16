// ---------------------------------------------------------------------------
// Value access helpers
// ---------------------------------------------------------------------------

/// Access a record field by name.
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
    unsafe { (*ctx).runtime_mut() }.reactive_note_record_field_access(value, name);
    match value {
        Value::Record(rec) => match rec.get(name) {
            Some(v) => abi::box_value(v.clone()),
            None => {
                unsafe {
                    set_pending_error(
                        ctx,
                        RuntimeError::Message(format!("field `{name}` does not exist on this record")),
                    );
                }
                abi::box_value(Value::Unit)
            }
        },
        other => {
            unsafe {
                set_pending_error(
                    ctx,
                    RuntimeError::Message(format!(
                        "tried to access field `{name}` on a non-record value: `{other:?}`"
                    )),
                );
            }
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

#[cfg(test)]
mod values_tests {
    use std::sync::Arc;

    use crate::cranelift_backend::abi::JitRuntimeCtx;
    use crate::runtime::values::Value;
    use crate::runtime::{Runtime, RuntimeError};

    use super::rt_record_field;

    fn test_runtime() -> Runtime {
        crate::runtime::build_runtime_base()
    }

    fn call_record_field(runtime: &mut Runtime, value: Value, field: &str) -> Value {
        let mut ctx = unsafe { JitRuntimeCtx::from_runtime(runtime) };
        let result = rt_record_field(
            &mut ctx,
            &value as *const Value,
            field.as_ptr(),
            field.len(),
        );
        unsafe { crate::cranelift_backend::abi::unbox_value(result) }
    }

    #[test]
    fn record_field_sets_pending_error_for_non_record_values() {
        let mut runtime = test_runtime();

        let result = call_record_field(&mut runtime, Value::Text("boom".to_string()), "path");

        assert!(matches!(result, Value::Unit));
        match runtime.jit_pending_error.take() {
            Some(RuntimeError::Message(message)) => {
                assert!(
                    message.contains("tried to access field `path` on a non-record value"),
                    "unexpected message: {message}"
                );
            }
            Some(err) => panic!("expected Message error, got {}", crate::runtime::format_runtime_error(err)),
            None => panic!("expected pending field-access error"),
        }
    }

    #[test]
    fn record_field_sets_pending_error_for_missing_fields() {
        let mut runtime = test_runtime();
        let value = Value::Record(Arc::new(std::collections::HashMap::new()));

        let result = call_record_field(&mut runtime, value, "path");

        assert!(matches!(result, Value::Unit));
        match runtime.jit_pending_error.take() {
            Some(RuntimeError::Message(message)) => {
                assert_eq!(message, "field `path` does not exist on this record");
            }
            Some(err) => panic!("expected Message error, got {}", crate::runtime::format_runtime_error(err)),
            None => panic!("expected pending missing-field error"),
        }
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
