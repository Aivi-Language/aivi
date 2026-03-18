use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use aivi::{
    check_modules, desugar_modules, elaborate_expected_coercions, load_modules_from_paths,
    run_test_suite,
};

#[path = "test_support.rs"]
mod test_support;

const FILE_TIMEOUT_SECS: u64 = 60;

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
        .name(format!("test-{display_name}"))
        .stack_size(256 * 1024 * 1024)
        .spawn(move || {
            let result = catch_unwind(AssertUnwindSafe(|| {
                run_test_suite(program, &test_entries, &modules, false, None)
            }));
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
    let result = handle.join().ok()?;
    match result {
        Ok(report) => Some(report),
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn run_number_stdlib_file(path: &Path) -> aivi::TestReport {
    let mut modules = load_modules_from_paths(&[path.to_path_buf()])
        .unwrap_or_else(|e| panic!("load_modules_from_paths({}): {e}", path.display()));

    let mut diags = check_modules(&modules);
    if !test_support::file_diagnostics_have_non_embedded_errors(&diags) {
        diags.extend(elaborate_expected_coercions(&mut modules));
    }
    diags.retain(|d| !d.path.starts_with("<embedded:"));
    assert!(
        !test_support::file_diagnostics_have_non_embedded_errors(&diags),
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
    let display = path.display().to_string();
    let result =
        run_test_suite_with_timeout(program, &tests, &modules, &display, FILE_TIMEOUT_SECS)
            .unwrap_or_else(|| panic!("timeout running number stdlib tests in {}", path.display()));
    result.unwrap_or_else(|e| panic!("run_test_suite({}): {e}", path.display()))
}

fn number_test_files() -> Vec<PathBuf> {
    let root = test_support::workspace_root().join("integration-tests/stdlib/aivi/number");
    let entries =
        std::fs::read_dir(&root).unwrap_or_else(|e| panic!("read_dir({}): {e}", root.display()));
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.unwrap_or_else(|e| panic!("read_dir({}): {e}", root.display()));
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "aivi") {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read_to_string({}): {e}", path.display()));
            if text.contains("@test") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

#[test]
fn number_stdlib_modules_execute_without_failures() {
    let result = std::thread::Builder::new()
        .name("number-stdlib".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(number_stdlib_modules_execute_without_failures_inner)
        .expect("spawn test thread")
        .join();
    match result {
        Ok(()) => {}
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn number_stdlib_modules_execute_without_failures_inner() {
    let workspace_root = test_support::workspace_root();
    std::env::set_current_dir(&workspace_root).expect("set cwd");

    let files = number_test_files();
    assert!(!files.is_empty(), "no number stdlib @test files found");

    let mut total_passed = 0usize;
    let mut total_failed = 0usize;
    let mut failures = Vec::new();

    for path in files {
        let report = run_number_stdlib_file(&path);
        total_passed += report.passed;
        total_failed += report.failed;
        if report.failed > 0 {
            failures.push(format!("{}: {:#?}", path.display(), report.failures));
        }
    }

    if !failures.is_empty() {
        panic!("number stdlib test failures:\n{}", failures.join("\n"));
    }
    assert_eq!(total_failed, 0, "number stdlib tests reported failures");
    assert!(total_passed > 0, "expected number stdlib tests to execute");
}
