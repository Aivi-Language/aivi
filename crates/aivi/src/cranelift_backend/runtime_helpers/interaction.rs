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

/// Returns `true` when the value is an arity-0 JIT builtin that should be
/// evaluated once and then cached in the global environment.
fn is_jit_arity0(val: &Value) -> bool {
    matches!(val, Value::Builtin(b)
        if b.imp.arity == 0 && b.args.is_empty() && b.imp.name.starts_with("__jit|"))
}

/// Force a value and, when it was a lazy arity-0 JIT builtin, cache the
/// result back into the global environment under `name` so that subsequent
/// lookups return the same identity (critical for `Signal`, `Resource`, etc.).
fn force_and_cache(
    runtime: &mut Runtime,
    name: &str,
    val: Value,
) -> Result<Value, RuntimeError> {
    let needs_cache = is_jit_arity0(&val);
    let forced = runtime.force_value(val)?;
    if needs_cache {
        runtime.ctx.globals.set(name.to_string(), forced.clone());
    }
    Ok(forced)
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
            // For qualified names (e.g. "aivi.ui.gtk4.init"), fall back to the
            // bare name ("init") when the qualified form isn't registered.
            // This handles imported names that only exist under their bare form.
            if let Some(dot_pos) = name.rfind('.') {
                let bare = &name[dot_pos + 1..];
                if let Some(v) = runtime.ctx.globals.get(bare) {
                    return match force_and_cache(runtime, bare, v) {
                        Ok(forced) => abi::box_value(forced),
                        Err(e) => {
                            unsafe { set_pending_error(ctx, e) };
                            abi::box_value(Value::Unit)
                        }
                    };
                }
            }
            rt_warn(
                ctx,
                "undefined global",
                &format!("global definition `{name}` was not found"),
                "this may indicate a missing import or a definition that failed to compile",
            );
            return abi::box_value(Value::Unit);
        }
    };
    match force_and_cache(runtime, name, val) {
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
/// Used at the end of JIT-compiled effect-style blocks to ensure the return
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
