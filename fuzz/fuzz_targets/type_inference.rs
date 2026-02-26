//! Fuzz target: type inference.
//!
//! Invariants checked:
//! - `infer_value_types`, `infer_value_types_full`, and `elaborate_expected_coercions`
//!   must NEVER panic, even on syntactically valid but semantically nonsensical input.
//! - All functions return diagnostics gracefully on ill-typed programs.

use std::path::Path;

#[test]
fn type_inference() {
    bolero::check!().for_each(|data: &[u8]| {
        if data.len() > 32 * 1024 {
            return;
        }
        let src = String::from_utf8_lossy(data);
        let (modules, parse_diags) = aivi::parse_modules(Path::new("fuzz.aivi"), &src);
        if aivi::file_diagnostics_have_errors(&parse_diags) {
            return;
        }

        let diags = aivi::check_modules(&modules);
        if aivi::file_diagnostics_have_errors(&diags) {
            return;
        }

        // Exercise all type inference entry points — must not panic.
        let (_diags, _type_strings, _span_types) = aivi::infer_value_types(&modules);
        let _full = aivi::infer_value_types_full(&modules);

        // elaborate_expected_coercions takes &mut — clone first.
        let mut modules_mut = modules.to_vec();
        let _elab_diags = aivi::elaborate_expected_coercions(&mut modules_mut);
    });
}
