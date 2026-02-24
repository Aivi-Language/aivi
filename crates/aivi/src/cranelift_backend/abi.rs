//! Cranelift JIT calling-convention types.
//!
//! Every JIT-compiled function receives `*mut JitRuntimeCtx` as its first
//! argument.  Non-scalar values are passed and returned as `*mut Value`
//! (heap-boxed); scalar types (Int, Float, Bool) use native unboxed
//! representations when the `CgType` is known at compile time.

use crate::runtime::values::Value;
use crate::runtime::Runtime;

/// Opaque context pointer threaded through every JIT-compiled function.
///
/// This is `#[repr(C)]` so its address can be passed to / from Cranelift code
/// as a raw `i64` (pointer-width integer).
#[repr(C)]
pub(crate) struct JitRuntimeCtx {
    /// The full interpreter runtime — used by runtime helpers that need to
    /// force thunks, apply closures, run effects, etc.
    pub(crate) runtime: *mut Runtime,
}

impl JitRuntimeCtx {
    /// # Safety
    /// Caller must ensure `runtime` outlives the JIT-compiled code that
    /// receives a pointer to this context.
    pub(crate) unsafe fn from_runtime(runtime: &mut Runtime) -> Self {
        Self {
            runtime: runtime as *mut Runtime,
        }
    }

    /// Create a context that owns the runtime (heap-allocated).
    /// Used by AOT binaries where the runtime lives for the process lifetime.
    ///
    /// # Safety
    /// The caller must ensure this context is dropped before process exit
    /// to clean up the runtime.
    pub(crate) unsafe fn from_runtime_owned(runtime: Runtime) -> Self {
        Self {
            runtime: Box::into_raw(Box::new(runtime)),
        }
    }

    /// # Safety
    /// The stored pointer must still be valid.
    pub(crate) unsafe fn runtime_mut(&mut self) -> &mut Runtime {
        &mut *self.runtime
    }
}

// ---------------------------------------------------------------------------
// Value tag constants — used by Cranelift-emitted code to inspect / construct
// values at the bit level.  The actual discriminant layout is determined by
// the Rust compiler, so we use helper functions instead of hard-coding offsets.
// ---------------------------------------------------------------------------

/// Box a Rust `Value` onto the heap and return a stable pointer.
/// The caller (JIT code) is responsible for eventually calling `rt_drop_value`.
pub(crate) fn box_value(value: Value) -> *mut Value {
    Box::into_raw(Box::new(value))
}

/// Reclaim a heap-boxed `Value`.
///
/// # Safety
/// `ptr` must have been created by `box_value` and must not be used afterwards.
pub(crate) unsafe fn unbox_value(ptr: *mut Value) -> Value {
    *Box::from_raw(ptr)
}

/// Clone a heap-boxed `Value`, returning a new heap-boxed copy.
///
/// # Safety
/// `ptr` must point to a valid `Value`.
pub(crate) unsafe fn clone_boxed_value(ptr: *const Value) -> *mut Value {
    let value = &*ptr;
    box_value(value.clone())
}
