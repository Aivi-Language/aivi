use std::path::Path;

use aivi::{
    check_modules, desugar_modules, elaborate_expected_coercions, file_diagnostics_have_errors,
    load_modules_from_paths, run_test_suite,
};

#[path = "test_support.rs"]
mod test_support;

fn run_stdlib_file(path: &Path) -> (usize, usize) {
    let mut modules = load_modules_from_paths(&[path.to_path_buf()])
        .unwrap_or_else(|e| panic!("load_modules_from_paths({}): {e}", path.display()));

    let mut diags = check_modules(&modules);
    if !file_diagnostics_have_errors(&diags) {
        diags.extend(elaborate_expected_coercions(&mut modules));
    }
    diags.retain(|d| !d.path.starts_with("<embedded:"));
    assert!(
        !file_diagnostics_have_errors(&diags),
        "type errors in {}: {diags:?}",
        path.display()
    );

    let tests = test_support::collect_test_entries(&modules);
    assert!(
        !tests.is_empty(),
        "no @test definitions found in {}",
        path.display()
    );

    let program = desugar_modules(&modules);
    let report = run_test_suite(program, &tests, &modules, false, None)
        .unwrap_or_else(|e| panic!("run_test_suite({}): {e}", path.display()));
    
    if report.failed > 0 {
        for failure in &report.failures {
            eprintln!("Test failure: {:?}", failure);
        }
    }
    
    (report.passed, report.failed)
}

#[test]
fn fixes_verification_file_executes_without_failures() {
    let root = test_support::workspace_root();
    let path = root.join("integration-tests/stdlib/fixes_verification.aivi");

    let (passed, failed) = run_stdlib_file(&path);

    assert_eq!(failed, 0, "fixes_verification.aivi reported failures");
    assert!(passed > 0, "expected tests to execute");
}
