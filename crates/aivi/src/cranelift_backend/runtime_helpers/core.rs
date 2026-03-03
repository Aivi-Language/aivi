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
