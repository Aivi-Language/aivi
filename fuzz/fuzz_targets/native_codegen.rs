//! Fuzz target: native (Rust) codegen.
//!
//! Invariants checked:
//! - `compile_rust_native`, `compile_rust_native_lib`, and their `_typed` variants
//!   must NEVER panic on well-typed HIR input.
//! - `lower_rust_ir` must not panic on well-typed kernel input.
//! - Codegen may return errors, but must never crash.

use std::path::Path;

#[test]
fn native_codegen() {
    bolero::check!().for_each(|data: &[u8]| {
        if data.len() > 16 * 1024 {
            return;
        }
        let src = String::from_utf8_lossy(data);
        let (modules, parse_diags) = aivi::parse_modules(Path::new("fuzz.aivi"), &src);
        if aivi::file_diagnostics_have_errors(&parse_diags) {
            return;
        }

        let mut diags = aivi::check_modules(&modules);
        if aivi::file_diagnostics_have_errors(&diags) {
            return;
        }
        diags.extend(aivi::check_types(&modules));
        if aivi::file_diagnostics_have_errors(&diags) {
            return;
        }

        let infer_result = aivi::infer_value_types_full(&modules);
        if aivi::file_diagnostics_have_errors(&infer_result.diagnostics) {
            return;
        }

        let hir = aivi::desugar_modules(&modules);

        // Exercise Rust codegen (untyped) — must not panic.
        let _ = aivi::compile_rust_native(hir.clone());
        let _ = aivi::compile_rust_native_lib(hir.clone());

        // Exercise typed Rust codegen — must not panic.
        let _ = aivi::compile_rust_native_typed(hir.clone(), infer_result.cg_types.clone());
        let _ = aivi::compile_rust_native_lib_typed(hir, infer_result.cg_types);

        // Exercise kernel lowering → Rust IR — must not panic.
        let hir2 = aivi::desugar_modules(&modules);
        let kernel = aivi::lower_kernel(hir2);
        let _ = aivi::lower_rust_ir(kernel);
    });
}
