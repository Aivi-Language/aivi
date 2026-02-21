#![no_main]

use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
    // Runtime fuzzing is intentionally kept conservative: only attempt to run small, valid programs
    // and always enforce a fuel budget to avoid hangs.
    if data.len() > 4 * 1024 {
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

    let hir = aivi::desugar_modules(&modules);
    let _ = aivi::run_native_with_fuel(hir, 25_000);
});
