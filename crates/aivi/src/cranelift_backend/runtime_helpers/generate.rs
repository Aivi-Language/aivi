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
            db_patch_meta: None,
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
