use std::path::Path;

fn without_embedded_errors(
    diags: Vec<crate::diagnostics::FileDiagnostic>,
) -> Vec<crate::diagnostics::FileDiagnostic> {
    diags
        .into_iter()
        .filter(|d| !d.path.starts_with("<embedded:"))
        .collect()
}

#[test]
fn bigint_arithmetic_typechecks_with_stdlib_import() {
    let source = r#"
@no_prelude
module test.bigint.domain

use aivi
use aivi.number.bigint

value = fromInt 3 * fromInt 2 - fromInt 1
"#;

    let (mut modules, diags) = crate::surface::parse_modules(Path::new("test.aivi"), source);
    assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");

    let mut all_modules = crate::stdlib::embedded_stdlib_modules();
    all_modules.append(&mut modules);

    let diags = without_embedded_errors(crate::resolver::check_modules(&all_modules));
    let errors: Vec<_> = diags
        .into_iter()
        .filter(|d| d.diagnostic.severity == crate::diagnostics::DiagnosticSeverity::Error)
        .collect();
    assert!(errors.is_empty(), "unexpected diagnostics: {errors:?}");
}
