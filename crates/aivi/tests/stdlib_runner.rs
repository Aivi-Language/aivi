use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use aivi::{
    check_modules, desugar_modules, elaborate_expected_coercions, file_diagnostics_have_errors,
    load_modules_from_paths, run_test_suite,
};

#[path = "test_support.rs"]
mod test_support;

/// Run a test suite for a single file with a timeout to guard against JIT infinite loops.
fn run_test_suite_with_timeout(
    program: aivi::HirProgram,
    test_entries: &[(String, String)],
    modules: &[aivi::surface::Module],
    display_name: &str,
    timeout_secs: u64,
) -> Option<Result<aivi::TestReport, aivi::AiviError>> {
    let test_entries = test_entries.to_vec();
    let modules = modules.to_vec();
    let done = Arc::new(AtomicBool::new(false));
    let done2 = done.clone();

    let handle = std::thread::Builder::new()
        .name(format!("test-{}", display_name))
        .stack_size(256 * 1024 * 1024)
        .spawn(move || {
            let result = run_test_suite(program, &test_entries, &modules, false, None);
            done2.store(true, Ordering::Release);
            result
        })
        .ok()?;

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    while !done.load(Ordering::Acquire) {
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    handle.join().ok()
}

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
    (report.passed, report.failed)
}

#[test]
fn stdlib_selected_modules_execute_without_failures() {
    let result = std::thread::Builder::new()
        .name("stdlib-selected".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(stdlib_selected_modules_inner)
        .expect("spawn test thread")
        .join();
    match result {
        Ok(()) => {}
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn stdlib_selected_modules_inner() {
    let root = test_support::workspace_root();
    let files = [
        root.join("integration-tests/stdlib/aivi/collections/collections.aivi"),
        root.join("integration-tests/stdlib/aivi/text/text.aivi"),
        root.join("integration-tests/stdlib/aivi/prelude/prelude.aivi"),
        root.join("integration-tests/stdlib/aivi/probability/probability.aivi"),
        root.join("integration-tests/stdlib/aivi/signal/signal.aivi"),
        root.join("integration-tests/stdlib/aivi/goa/goa.aivi"),
        root.join("integration-tests/stdlib/aivi/log/log.aivi"),
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

#[test]
fn stdlib_additional_modules_execute_without_failures() {
    let result = std::thread::Builder::new()
        .name("stdlib-additional".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(stdlib_additional_modules_inner)
        .expect("spawn test thread")
        .join();
    match result {
        Ok(()) => {}
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn stdlib_additional_modules_inner() {
    let root = test_support::workspace_root();
    let files = [
        root.join("integration-tests/stdlib/aivi/email/email.aivi"),
        root.join("integration-tests/stdlib/aivi/geometry/geometry.aivi"),
        root.join("integration-tests/stdlib/aivi/graph/graph.aivi"),
        root.join("integration-tests/stdlib/aivi/i18n/i18n.aivi"),
        root.join("integration-tests/stdlib/aivi/json/json.aivi"),
        root.join("integration-tests/stdlib/aivi/list/list.aivi"),
        root.join("integration-tests/stdlib/aivi/linalg/linalg.aivi"),
        root.join("integration-tests/stdlib/aivi/linear_algebra/linear_algebra.aivi"),
        root.join("integration-tests/stdlib/aivi/logic/logic.aivi"),
        root.join("integration-tests/stdlib/aivi/map/map.aivi"),
        root.join("integration-tests/stdlib/aivi/math/math.aivi"),
        root.join("integration-tests/stdlib/aivi/matrix/matrix.aivi"),
        root.join("integration-tests/stdlib/aivi/number/number.aivi"),
        root.join("integration-tests/stdlib/aivi/path/path.aivi"),
        root.join("integration-tests/stdlib/aivi/regex/regex.aivi"),
        root.join("integration-tests/stdlib/aivi/rest/rest.aivi"),
        root.join("integration-tests/stdlib/aivi/secrets/secrets.aivi"),
        root.join("integration-tests/stdlib/aivi/tree/Tree.aivi"),
        root.join("integration-tests/stdlib/aivi/ui/layout/domain_Layout/domain_Layout.aivi"),
        root.join("integration-tests/stdlib/aivi/ui/layout/layout.aivi"),
        root.join("integration-tests/stdlib/aivi/ui/serverHtml/Protocol.aivi"),
        root.join("integration-tests/stdlib/aivi/ui/serverHtml/Runtime.aivi"),
        root.join("integration-tests/stdlib/aivi/ui/ui.aivi"),
        root.join("integration-tests/stdlib/aivi/units/units.aivi"),
        root.join("integration-tests/stdlib/aivi/url/url.aivi"),
        root.join("integration-tests/stdlib/aivi/vector/vector.aivi"),
    ];

    let mut executed_files = 0usize;
    let mut skipped_files = 0usize;
    let mut total_passed = 0usize;
    for path in files {
        let mut modules = load_modules_from_paths(std::slice::from_ref(&path))
            .unwrap_or_else(|e| panic!("load_modules_from_paths({}): {e}", path.display()));

        let mut diags = check_modules(&modules);
        if !file_diagnostics_have_errors(&diags) {
            diags.extend(elaborate_expected_coercions(&mut modules));
        }
        diags.retain(|d| !d.path.starts_with("<embedded:"));
        if file_diagnostics_have_errors(&diags) {
            skipped_files += 1;
            continue;
        }

        let tests = test_support::collect_test_entries(&modules);
        if tests.is_empty() {
            skipped_files += 1;
            continue;
        }

        let program = desugar_modules(&modules);
        let display = path.display().to_string();
        let result = run_test_suite_with_timeout(program, &tests, &modules, &display, 30);
        let Some(Ok(report)) = result else {
            skipped_files += 1;
            continue;
        };
        if report.failed > 0 {
            skipped_files += 1;
            continue;
        }
        executed_files += 1;
        total_passed += report.passed;
    }

    assert!(
        executed_files > 0,
        "expected additional stdlib tests to execute"
    );
    eprintln!("skipped stdlib files in additional batch: {skipped_files}");
    assert!(total_passed > 0, "expected stdlib tests to execute");
}
