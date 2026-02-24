//! JIT module construction with runtime helper symbols pre-registered.

use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::default_libcall_names;

use super::runtime_helpers::runtime_helper_symbols;

/// Create a `JITBuilder` with all AIVI runtime helper symbols registered,
/// so that Cranelift-compiled functions can call them.
pub(crate) fn create_jit_builder() -> Result<JITBuilder, String> {
    let mut builder = JITBuilder::new(default_libcall_names())
        .map_err(|err| format!("jit builder init failed: {err}"))?;

    for (name, ptr) in runtime_helper_symbols() {
        builder.symbol(name, ptr);
    }

    Ok(builder)
}

/// Create a `JITModule` ready for defining Cranelift functions that can call
/// into the AIVI runtime.
pub(crate) fn create_jit_module() -> Result<JITModule, String> {
    let builder = create_jit_builder()?;
    Ok(JITModule::new(builder))
}
