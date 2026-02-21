#![no_main]

use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
    if data.len() > 32 * 1024 {
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

    // Exercise lowering stages on well-typed inputs.
    let hir = aivi::desugar_modules(&modules);
    let _kernel = aivi::lower_kernel(hir);
});
