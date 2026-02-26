mod abi;
mod compile;
mod jit_module;
pub(crate) mod lower;
mod object_module;
mod runtime_helpers;

pub use compile::compile_to_object;
pub use compile::run_cranelift_jit;
pub use compile::run_cranelift_jit_cancellable;
pub use compile::run_test_suite_jit;

pub fn init_aot_runtime(program: crate::hir::HirProgram) -> usize {
    let ptr = Box::into_raw(Box::new(program));
    runtime_helpers::aivi_rt_init(ptr) as usize
}

/// Create a minimal AOT runtime with only builtins (no user program needed).
/// AOT-compiled functions are registered by `__aivi_main` at entry.
pub fn init_aot_runtime_base() -> usize {
    runtime_helpers::aivi_rt_init_base() as usize
}

pub fn destroy_aot_runtime(ctx: usize) {
    if ctx != 0 {
        runtime_helpers::aivi_rt_destroy(ctx as *mut abi::JitRuntimeCtx);
    }
}
