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
        // Bool values are stored as Value::Bool but matched as constructors True/False
        Value::Bool(b) => i64::from((*b && name == "True") || (!*b && name == "False")),
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
        // For MultiClause operators (multiple domains defining the same op),
        // try each clause individually and use the first one that succeeds without
        // a hard error or non-exhaustive match.  Wrong-domain clauses now fail
        // hard (via rt_record_field strict mode) instead of silently returning Unit.
        if let Value::MultiClause(clauses) = op_value {
            if !runtime.jit_binary_op_dispatching {
                runtime.jit_binary_op_dispatching = true;
                let mut fallback_result: Option<Value> = None;
                for clause in clauses.into_iter() {
                    let wc = runtime.jit_rt_warning_count;
                    // Save global state so each clause trial starts clean.
                    let saved_pending = runtime.jit_pending_error.take();
                    let saved_match_failed = runtime.jit_match_failed;
                    runtime.jit_match_failed = false;
                    let clean = if let Ok(applied) = runtime.apply(clause, lhs.clone()) {
                        match runtime.apply(applied, rhs.clone()) {
                            Ok(result) => {
                                let warns = runtime.jit_rt_warning_count - wc;
                                let is_dt = (matches!(lhs, Value::DateTime(_)) || matches!(rhs, Value::DateTime(_))) && op == "-";
                                if is_dt {
                                    eprintln!("[DISP] Ok warns={warns} match_failed={} pending={}", runtime.jit_match_failed, runtime.jit_pending_error.is_some());
                                }
                                if warns == 0 && !runtime.jit_match_failed {
                                    // Clean match — no warnings produced, restore and return.
                                    if is_dt {
                                        eprintln!("[DISP] CLEAN WIN: {:?} {op} {:?} => {:?}", lhs, rhs, result);
                                    }
                                    runtime.jit_binary_op_dispatching = false;
                                    runtime.jit_pending_error = saved_pending;
                                    runtime.jit_match_failed = saved_match_failed;
                                    return abi::box_value(result);
                                }
                                // Produced warnings — keep as fallback.
                                if fallback_result.is_none() && !runtime.jit_match_failed {
                                    fallback_result = Some(result);
                                }
                                false
                            }
                            Err(e) => {
                                let is_dt = (matches!(lhs, Value::DateTime(_)) || matches!(rhs, Value::DateTime(_))) && op == "-";
                                if is_dt {
                                    let msg = match &e { RuntimeError::Message(m) => m.clone(), _ => "other".to_string() };
                                    eprintln!("[DISP] Err: {msg}");
                                }
                                false
                            }
                        }
                    } else {
                        false
                    };
                    let _ = clean;
                    // Restore global state for next clause trial.
                    runtime.jit_pending_error = saved_pending;
                    runtime.jit_match_failed = saved_match_failed;
                    runtime.jit_rt_warning_count = wc;
                }
                runtime.jit_binary_op_dispatching = false;
                if let Some(result) = fallback_result {
                    return abi::box_value(result);
                }
            }
            // When already dispatching (nested call), skip MultiClause to avoid
            // cascading trial-and-error. Fall through to the error below.
        } else if let Ok(applied) = runtime.apply(op_value.clone(), lhs.clone()) {
            if let Ok(result) = runtime.apply(applied, rhs.clone()) {
                return abi::box_value(result);
            }
        }
    }
    if runtime.jit_binary_op_dispatching {
        // Nested inside MultiClause dispatch: allow primitive integer negation
        // since unary `-x` is desugared to `0 - x` in HIR.
        if op == "-" {
            if let (Value::Int(0), Value::Int(n)) = (&lhs, &rhs) {
                return abi::box_value(Value::Int(0i64.wrapping_sub(*n)));
            }
        }
        // For any other nested op we cannot resolve: signal failure via warning
        // counter so the outer dispatch loop discards this clause trial.
        runtime.jit_rt_warning_count += 1;
        return abi::box_value(Value::Unit);
    }
    eprintln!(
        "{RT_YELLOW}warning[RT]{RT_RESET} {RT_BOLD}operator error{RT_RESET}: binary operator `{op}` could not be applied to the given operand types"
    );
    eprintln!(
        "  {RT_CYAN}hint{RT_RESET}: check that both operands have compatible types for `{op}`"
    );
    abi::box_value(Value::Unit)
}
