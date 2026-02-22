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
    let report = run_test_suite(program, &tests, &modules)
        .unwrap_or_else(|e| panic!("run_test_suite({}): {e}", path.display()));
    (report.passed, report.failed)
}

#[test]
fn stdlib_selected_modules_execute_without_failures() {
    let root = test_support::workspace_root();
    let files = [
        root.join("integration-tests/stdlib/aivi/collections/collections.aivi"),
        root.join("integration-tests/stdlib/aivi/text/text.aivi"),
        root.join("integration-tests/stdlib/aivi/prelude/prelude.aivi"),
        root.join("integration-tests/stdlib/aivi/probability/probability.aivi"),
        root.join("integration-tests/stdlib/aivi/signal/signal.aivi"),
        root.join("integration-tests/stdlib/aivi/system/system.aivi"),
        root.join("integration-tests/stdlib/aivi/testing/testing.aivi"),
    ];

    let mut total_passed = 0usize;
    let mut total_failed = 0usize;
    for path in files {
        let (passed, failed) = run_stdlib_file(&path);
        total_passed += passed;
        total_failed += failed;
    }

    assert_eq!(total_failed, 0, "stdlib tests reported failures");
    assert!(total_passed > 0, "expected stdlib tests to execute");
}
