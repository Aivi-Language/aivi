// ---------------------------------------------------------------------------
// ANSI color helpers for runtime error reporting
// ---------------------------------------------------------------------------

use aivi_driver::{RuntimeFrame, RuntimeFrameKind};

use crate::{SourceKind, SourceOrigin, Span};

const RT_YELLOW: &str = "\x1b[1;33m";
const RT_CYAN: &str = "\x1b[1;36m";
const RT_GRAY: &str = "\x1b[90m";
const RT_RESET: &str = "\x1b[0m";
const RT_BOLD: &str = "\x1b[1m";

/// Print a formatted runtime warning to stderr.
fn rt_warn(ctx: *mut JitRuntimeCtx, category: &str, message: &str, hint: &str) {
    let (warning_line, hint_line, suppress, capture_ctx) = unsafe {
        let runtime = (*ctx).runtime_mut();
        runtime.jit_rt_warning_count += 1;
        let suppress = runtime.jit_binary_op_dispatching || runtime.jit_suppress_warnings;
        let fn_part = runtime
            .jit_current_fn
            .as_deref()
            .map(|s| format!(" {RT_GRAY}in `{s}`{RT_RESET}"))
            .unwrap_or_default();
        let loc_part = runtime
            .jit_current_loc
            .as_ref()
            .map(|origin| format!(" {RT_GRAY}at {}{RT_RESET}", origin.start_position_text()))
            .unwrap_or_default();
        let warning_line = format!(
            "{RT_YELLOW}warning[RT]{RT_RESET}{fn_part}{loc_part} {RT_BOLD}{category}{RT_RESET}: {message}"
        );
        let hint_line = (!hint.is_empty()).then(|| format!("  {RT_CYAN}hint{RT_RESET}: {hint}"));
        (warning_line, hint_line, suppress, runtime.ctx.clone())
    };
    if suppress {
        return;
    }
    if !capture_ctx.capture_stderr(&warning_line, true) {
        eprintln!("{warning_line}");
    }
    if let Some(hint_line) = hint_line {
        if !capture_ctx.capture_stderr(&hint_line, true) {
            eprintln!("{hint_line}");
        }
    }
}

/// Store a pending error on the runtime context, preserving the first error
/// (root cause) when multiple cascading failures occur within a single JIT call.
unsafe fn set_pending_error(ctx: *mut JitRuntimeCtx, e: RuntimeError) {
    let runtime = (*ctx).runtime_mut();
    if runtime.jit_pending_error.is_none() {
        if runtime.jit_pending_snapshot.is_none() {
            runtime.jit_pending_snapshot = Some(runtime.capture_runtime_snapshot());
        }
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

struct EncodedSourcePath<'a> {
    ptr: *const u8,
    len: usize,
    label: &'a str,
}

struct RawSourceSpan {
    start_line: i64,
    start_col: i64,
    end_line: i64,
    end_col: i64,
    kind: i64,
}

/// Called at the start of every JIT-compiled function to record its name.
/// This makes subsequent runtime warnings show which function triggered them.
fn decode_source_origin(
    ctx: *mut JitRuntimeCtx,
    path: EncodedSourcePath<'_>,
    raw_span: RawSourceSpan,
) -> Option<SourceOrigin> {
    if path.ptr.is_null() || path.len == 0 {
        return None;
    }
    let path = decode_utf8_owned(ctx, path.ptr, path.len, path.label)?;
    let span = Span {
        start: crate::diagnostics::Position {
            line: raw_span.start_line.max(1) as usize,
            column: raw_span.start_col.max(1) as usize,
        },
        end: crate::diagnostics::Position {
            line: raw_span.end_line.max(1) as usize,
            column: raw_span.end_col.max(1) as usize,
        },
    };
    Some(SourceOrigin::with_kind(
        path,
        span,
        SourceKind::from_i64(raw_span.kind),
    ))
}

#[no_mangle]
pub extern "C" fn rt_enter_fn(
    ctx: *mut JitRuntimeCtx,
    ptr: *const u8,
    len: usize,
    fallback_ptr: *const u8,
    fallback_len: usize,
    start_line: i64,
    start_col: i64,
    end_line: i64,
    end_col: i64,
    kind: i64,
) {
    if ctx.is_null() {
        return;
    }
    let Some(name) = decode_utf8_owned(ctx, ptr, len, "rt_enter_fn") else {
        return;
    };
    let fallback_origin = decode_source_origin(
        ctx,
        EncodedSourcePath {
            ptr: fallback_ptr,
            len: fallback_len,
            label: "rt_enter_fn",
        },
        RawSourceSpan {
            start_line,
            start_col,
            end_line,
            end_col,
            kind,
        },
    );
    let runtime = unsafe { (*ctx).runtime_mut() };
    let inherited_origin = runtime.jit_pending_call_loc.take().or(fallback_origin);
    runtime.jit_current_fn = Some(name.clone().into_boxed_str());
    runtime.jit_current_loc = inherited_origin.clone();
    runtime.jit_frame_stack.push(RuntimeFrame {
        kind: RuntimeFrameKind::Function,
        name: name.clone(),
        origin: inherited_origin,
    });
    FN_HISTORY.with(|h| {
        let mut h = h.borrow_mut();
        h.push(name);
        if h.len() > 20 {
            h.remove(0);
        }
    });
}

#[no_mangle]
pub extern "C" fn rt_leave_fn(ctx: *mut JitRuntimeCtx) {
    if ctx.is_null() {
        return;
    }
    let runtime = unsafe { (*ctx).runtime_mut() };
    runtime.jit_frame_stack.pop();
    runtime.jit_current_fn = runtime
        .jit_frame_stack
        .last()
        .map(|frame| frame.name.clone().into_boxed_str());
    runtime.jit_current_loc = runtime
        .jit_frame_stack
        .last()
        .and_then(|frame| frame.origin.clone());
}

/// Called before potentially-failing operations to record the source location.
/// This makes subsequent runtime warnings show the source location (line:col).
#[no_mangle]
pub extern "C" fn rt_set_location(
    ctx: *mut JitRuntimeCtx,
    ptr: *const u8,
    len: usize,
    start_line: i64,
    start_col: i64,
    end_line: i64,
    end_col: i64,
    kind: i64,
) {
    if ctx.is_null() {
        return;
    }
    let Some(origin) = decode_source_origin(
        ctx,
        EncodedSourcePath {
            ptr,
            len,
            label: "rt_set_location",
        },
        RawSourceSpan {
            start_line,
            start_col,
            end_line,
            end_col,
            kind,
        },
    ) else {
        return;
    };
    let runtime = unsafe { (*ctx).runtime_mut() };
    runtime.jit_current_loc = Some(origin.clone());
    if let Some(frame) = runtime.jit_frame_stack.last_mut() {
        frame.origin = Some(origin);
    }
}

#[no_mangle]
pub extern "C" fn rt_prepare_call_location(
    ctx: *mut JitRuntimeCtx,
    ptr: *const u8,
    len: usize,
    start_line: i64,
    start_col: i64,
    end_line: i64,
    end_col: i64,
    kind: i64,
) {
    if ctx.is_null() {
        return;
    }
    let Some(origin) = decode_source_origin(
        ctx,
        EncodedSourcePath {
            ptr,
            len,
            label: "rt_prepare_call_location",
        },
        RawSourceSpan {
            start_line,
            start_col,
            end_line,
            end_col,
            kind,
        },
    ) else {
        return;
    };
    let runtime = unsafe { (*ctx).runtime_mut() };
    runtime.jit_pending_call_loc = Some(origin);
}

#[cfg(test)]
mod core_tests {
    use crate::cranelift_backend::abi::JitRuntimeCtx;
    use crate::runtime::Runtime;
    use crate::{Position, SourceOrigin, Span};

    use super::{rt_enter_fn, rt_leave_fn, rt_prepare_call_location, rt_set_location};

    fn test_runtime() -> Runtime {
        crate::runtime::build_runtime_base()
    }

    #[test]
    fn enter_fn_clears_stale_source_location() {
        let mut runtime = test_runtime();
        runtime.jit_current_loc = Some(SourceOrigin::new(
            "<embedded:old>",
            Span {
                start: Position { line: 99, column: 1 },
                end: Position {
                    line: 99,
                    column: 10,
                },
            },
        ));
        let mut ctx = unsafe { JitRuntimeCtx::from_runtime(&mut runtime) };

        rt_enter_fn(
            &mut ctx,
            b"test.fn".as_ptr(),
            "test.fn".len(),
            std::ptr::null(),
            0,
            0,
            0,
            0,
            0,
            0,
        );

        assert_eq!(runtime.jit_current_fn.as_deref(), Some("test.fn"));
        assert!(runtime.jit_current_loc.is_none());
        assert_eq!(runtime.jit_frame_stack.len(), 1);
    }

    #[test]
    fn pending_call_location_is_consumed_on_enter_fn() {
        let mut runtime = test_runtime();
        let mut ctx = unsafe { JitRuntimeCtx::from_runtime(&mut runtime) };

        rt_prepare_call_location(
            &mut ctx,
            b"src/main.aivi".as_ptr(),
            "src/main.aivi".len(),
            12,
            4,
            12,
            18,
            0,
        );
        rt_enter_fn(
            &mut ctx,
            b"test.fn".as_ptr(),
            "test.fn".len(),
            std::ptr::null(),
            0,
            0,
            0,
            0,
            0,
            0,
        );

        assert_eq!(runtime.jit_current_fn.as_deref(), Some("test.fn"));
        assert_eq!(
            runtime
                .jit_current_loc
                .as_ref()
                .expect("current location")
                .start_position_text(),
            "src/main.aivi:12:4"
        );
        assert_eq!(
            runtime
                .jit_frame_stack
                .last()
                .and_then(|frame| frame.origin.clone())
                .expect("frame origin")
                .start_position_text(),
            "src/main.aivi:12:4"
        );
        assert!(runtime.jit_pending_call_loc.is_none());
    }

    #[test]
    fn set_location_updates_top_frame_origin() {
        let mut runtime = test_runtime();
        let mut ctx = unsafe { JitRuntimeCtx::from_runtime(&mut runtime) };

        rt_enter_fn(
            &mut ctx,
            b"test.fn".as_ptr(),
            "test.fn".len(),
            std::ptr::null(),
            0,
            0,
            0,
            0,
            0,
            0,
        );
        rt_set_location(&mut ctx, b"src/main.aivi".as_ptr(), "src/main.aivi".len(), 4, 2, 4, 12, 0);

        let current = runtime
            .jit_current_loc
            .clone()
            .expect("current location");
        assert_eq!(current.start_position_text(), "src/main.aivi:4:2");
        assert_eq!(
            runtime
                .jit_frame_stack
                .last()
                .and_then(|frame| frame.origin.clone())
                .expect("frame origin")
                .start_position_text(),
            "src/main.aivi:4:2"
        );
    }

    #[test]
    fn leave_fn_restores_previous_frame() {
        let mut runtime = test_runtime();
        let mut ctx = unsafe { JitRuntimeCtx::from_runtime(&mut runtime) };

        rt_enter_fn(
            &mut ctx,
            b"outer.fn".as_ptr(),
            "outer.fn".len(),
            std::ptr::null(),
            0,
            0,
            0,
            0,
            0,
            0,
        );
        rt_set_location(&mut ctx, b"src/main.aivi".as_ptr(), "src/main.aivi".len(), 1, 1, 1, 6, 0);
        rt_enter_fn(
            &mut ctx,
            b"inner.fn".as_ptr(),
            "inner.fn".len(),
            std::ptr::null(),
            0,
            0,
            0,
            0,
            0,
            0,
        );
        rt_set_location(&mut ctx, b"src/main.aivi".as_ptr(), "src/main.aivi".len(), 8, 3, 8, 10, 0);

        rt_leave_fn(&mut ctx);

        assert_eq!(runtime.jit_current_fn.as_deref(), Some("outer.fn"));
        assert_eq!(
            runtime
                .jit_current_loc
                .as_ref()
                .expect("restored location")
                .start_position_text(),
            "src/main.aivi:1:1"
        );
    }

    #[test]
    fn enter_fn_uses_fallback_location_when_pending_location_is_missing() {
        let mut runtime = test_runtime();
        let mut ctx = unsafe { JitRuntimeCtx::from_runtime(&mut runtime) };

        rt_enter_fn(
            &mut ctx,
            b"test.fn".as_ptr(),
            "test.fn".len(),
            b"src/main.aivi".as_ptr(),
            "src/main.aivi".len(),
            7,
            3,
            7,
            12,
            0,
        );

        assert_eq!(
            runtime
                .jit_current_loc
                .as_ref()
                .expect("fallback location")
                .start_position_text(),
            "src/main.aivi:7:3"
        );
    }

    #[test]
    fn pending_call_location_overrides_fallback_location() {
        let mut runtime = test_runtime();
        let mut ctx = unsafe { JitRuntimeCtx::from_runtime(&mut runtime) };

        rt_prepare_call_location(
            &mut ctx,
            b"src/callsite.aivi".as_ptr(),
            "src/callsite.aivi".len(),
            4,
            9,
            4,
            20,
            0,
        );
        rt_enter_fn(
            &mut ctx,
            b"test.fn".as_ptr(),
            "test.fn".len(),
            b"src/definition.aivi".as_ptr(),
            "src/definition.aivi".len(),
            12,
            2,
            12,
            15,
            0,
        );

        assert_eq!(
            runtime
                .jit_current_loc
                .as_ref()
                .expect("call-site location")
                .start_position_text(),
            "src/callsite.aivi:4:9"
        );
    }
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
    runtime.capture_match_failure();
    abi::box_value(Value::Unit)
}
