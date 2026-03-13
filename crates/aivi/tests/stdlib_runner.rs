use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use aivi::{
    check_modules, desugar_modules, elaborate_expected_coercions, load_modules_from_paths,
    run_test_suite,
};
use rayon::prelude::*;

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

fn walk_aivi_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir({}): {e}", dir.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|e| panic!("read_dir({}): {e}", dir.display()));
        let path = entry.path();
        let ty = entry
            .file_type()
            .unwrap_or_else(|e| panic!("file_type({}): {e}", path.display()));
        if ty.is_dir() {
            walk_aivi_files(&path, out);
            continue;
        }
        if path.extension().is_some_and(|ext| ext == "aivi") {
            out.push(path);
        }
    }
}

fn file_contains_test(path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    text.contains("@test")
}

fn should_skip_file(path: &Path) -> bool {
    let p = path.to_string_lossy();
    if p.contains("/integration-tests/stdlib/aivi/ui/gtk4/") {
        return std::env::var_os("AIVI_RUN_GTK_TESTS").is_none();
    }
    if p.contains("/integration-tests/stdlib/aivi/console/") {
        return std::env::var_os("AIVI_RUN_CONSOLE_TESTS").is_none();
    }
    false
}

fn run_stdlib_file_with_timeout(path: &Path, timeout_secs: u64) -> aivi::TestReport {
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
    let result = run_test_suite_with_timeout(program, &tests, &modules, &display, timeout_secs)
        .unwrap_or_else(|| panic!("timeout running stdlib tests in {}", path.display()));
    result.unwrap_or_else(|e| panic!("run_test_suite({}): {e}", path.display()))
}

#[test]
fn stdlib_modules_execute_without_failures() {
    let result = std::thread::Builder::new()
        .name("stdlib-all".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(stdlib_modules_execute_without_failures_inner)
        .expect("spawn test thread")
        .join();
    match result {
        Ok(()) => {}
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn stdlib_modules_execute_without_failures_inner() {
    let workspace_root = test_support::workspace_root();
    std::env::set_current_dir(&workspace_root).expect("set cwd");
    let root = workspace_root.join("integration-tests/stdlib/aivi");
    let mut all_files = Vec::new();
    walk_aivi_files(&root, &mut all_files);
    all_files.sort();

    let files: Vec<_> = all_files
        .into_iter()
        .filter(|p| file_contains_test(p))
        .filter(|p| !should_skip_file(p))
        .collect();

    assert!(!files.is_empty(), "no stdlib @test files found");

    let stdlib_threads = std::env::var("AIVI_STDLIB_TEST_THREADS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|count| *count > 0)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|count| count.get().min(2))
                .unwrap_or(1)
        });
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(stdlib_threads)
        .build()
        .expect("build stdlib runner rayon pool");

    let results: Vec<_> = pool.install(|| {
        files
            .par_iter()
            .map(|path| (path.clone(), run_stdlib_file_with_timeout(path, 60)))
            .collect()
    });

    let mut total_passed = 0usize;
    let mut total_failed = 0usize;
    let mut failures = Vec::new();

    for (path, report) in results {
        total_passed += report.passed;
        total_failed += report.failed;
        if report.failed > 0 {
            failures.push(format!("{}: {:#?}", path.display(), report.failures));
        }
    }

    if !failures.is_empty() {
        panic!("stdlib test failures:\n{}", failures.join("\n"));
    }
    assert_eq!(total_failed, 0, "stdlib tests reported failures");
    assert!(total_passed > 0, "expected stdlib tests to execute");
}
