use std::path::Path;

#[test]
fn frontend() {
    bolero::check!().for_each(|data: &[u8]| {
        if data.len() > 32 * 1024 {
            return;
        }
        let src = String::from_utf8_lossy(data);
        let (modules, parse_diags) = aivi::parse_modules(Path::new("fuzz.aivi"), &src);
        if aivi::file_diagnostics_have_errors(&parse_diags) {
            return;
        }
        let _arena = aivi::lower_modules_to_arena(&modules);
        let _ = aivi::check_types_including_stdlib(&modules);

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
        let _kernel = aivi::desugar_blocks(hir);
    });
}
