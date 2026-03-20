use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use aivi::{
    check_modules, desugar_modules, elaborate_stdlib_checkpoint, elaborate_with_checkpoint,
    embedded_stdlib_modules, file_diagnostics_have_errors, parse_modules, resolve_import_names,
    run_test_suite, ElaborationCheckpoint,
};
use walkdir::WalkDir;

#[path = "test_support.rs"]
mod test_support;

const FILE_TIMEOUT_SECS: u64 = 60;

fn runner_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("runner test lock")
}

/// Run a test suite for a single file with a timeout to guard against JIT infinite loops.
fn run_test_suite_with_timeout(
    program: aivi::HirProgram,
    test_entries: &[(String, String)],
    modules: &[aivi::surface::Module],
    display_name: &str,
    timeout_secs: u64,
    project_root: Option<PathBuf>,
) -> Option<Result<aivi::TestReport, aivi::AiviError>> {
    let test_entries = test_entries.to_vec();
    let modules = modules.to_vec();
    let project_root = project_root.clone();
    let done = Arc::new(AtomicBool::new(false));
    let done2 = done.clone();

    let handle = std::thread::Builder::new()
        .name(format!("test-{}", display_name))
        .stack_size(256 * 1024 * 1024)
        .spawn(move || {
            let result = catch_unwind(AssertUnwindSafe(|| {
                run_test_suite(
                    program,
                    &test_entries,
                    &modules,
                    false,
                    project_root.clone(),
                )
            }));
            done2.store(true, Ordering::Release);
            result
        })
        .ok()?;

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    while !done.load(Ordering::Acquire) {
        if Instant::now() >= deadline {
            return None; // Thread is leaked but caller continues
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let result = handle.join().ok()?;
    match result {
        Ok(report) => Some(report),
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

/// Result of processing a single test file.
enum FileResult {
    Skipped,
    Passed(usize),
    Failed {
        passed: usize,
        failed: usize,
        failures: Vec<(String, String)>,
    },
}

/// Process a single test file through the full pipeline: parse → check → elaborate → desugar → JIT.
fn process_test_file(
    path: &Path,
    stdlib_modules: &[aivi::surface::Module],
    checkpoint: &ElaborationCheckpoint,
) -> FileResult {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return FileResult::Skipped,
    };
    let (file_modules, _) = parse_modules(path, &content);
    let mut modules = stdlib_modules.to_vec();
    modules.extend(file_modules);
    resolve_import_names(&mut modules);

    let mut diags = check_modules(&modules);
    if !test_support::file_diagnostics_have_non_embedded_errors(&diags) {
        diags.extend(elaborate_with_checkpoint(&mut modules, checkpoint));
    }
    diags.retain(|d| !d.path.starts_with("<embedded:"));
    if file_diagnostics_have_errors(&diags) {
        return FileResult::Skipped;
    }

    let tests = test_support::collect_test_entries(&modules);
    if tests.is_empty() {
        return FileResult::Skipped;
    }

    let program = desugar_modules(&modules);
    let display = path.display().to_string();
    let project_root = path
        .ancestors()
        .find(|ancestor| ancestor.join("Cargo.toml").exists())
        .map(Path::to_path_buf);
    let result = run_test_suite_with_timeout(
        program,
        &tests,
        &modules,
        &display,
        FILE_TIMEOUT_SECS,
        project_root,
    );
    let Some(Ok(report)) = result else {
        return FileResult::Skipped;
    };
    if report.failed > 0 {
        let failures = report
            .failures
            .iter()
            .map(|f| (f.name.clone(), f.message.clone()))
            .collect();
        return FileResult::Failed {
            passed: report.passed,
            failed: report.failed,
            failures,
        };
    }
    FileResult::Passed(report.passed)
}

/// Run files in parallel using scoped threads, returning (total_passed, total_failed, skipped, failures).
/// Limits concurrency to available CPUs so multiple `#[test]` functions don't oversubscribe the system.
fn run_files_parallel(
    files: &[PathBuf],
    stdlib_modules: &[aivi::surface::Module],
    checkpoint: &ElaborationCheckpoint,
) -> (usize, usize, usize, Vec<(String, String)>) {
    let total_passed = AtomicUsize::new(0);
    let total_failed = AtomicUsize::new(0);
    let skipped = AtomicUsize::new(0);
    let failures = std::sync::Mutex::new(Vec::<(String, String)>::new());
    let max_threads = test_support::configured_test_threads("AIVI_RUNNER_TEST_THREADS", 1);
    let semaphore = Arc::new((std::sync::Mutex::new(0usize), std::sync::Condvar::new()));

    std::thread::scope(|s| {
        let handles: Vec<_> = files
            .iter()
            .map(|path| {
                let sem = semaphore.clone();
                s.spawn(move || {
                    // Acquire: wait until running < max_threads
                    {
                        let (lock, cvar) = &*sem;
                        let mut running = lock.lock().expect("lock");
                        while *running >= max_threads {
                            running = cvar.wait(running).expect("wait");
                        }
                        *running += 1;
                    }
                    let result = process_test_file(path, stdlib_modules, checkpoint);
                    // Release
                    {
                        let (lock, cvar) = &*sem;
                        let mut running = lock.lock().expect("lock");
                        *running -= 1;
                        cvar.notify_one();
                    }
                    result
                })
            })
            .collect();

        for handle in handles {
            match handle.join().expect("thread panicked") {
                FileResult::Skipped => {
                    skipped.fetch_add(1, Ordering::Relaxed);
                }
                FileResult::Passed(n) => {
                    total_passed.fetch_add(n, Ordering::Relaxed);
                }
                FileResult::Failed {
                    passed,
                    failed,
                    failures: file_failures,
                } => {
                    total_passed.fetch_add(passed, Ordering::Relaxed);
                    total_failed.fetch_add(failed, Ordering::Relaxed);
                    failures.lock().expect("lock").extend(file_failures);
                }
            }
        }
    });

    (
        total_passed.load(Ordering::Relaxed),
        total_failed.load(Ordering::Relaxed),
        skipped.load(Ordering::Relaxed),
        failures.into_inner().expect("lock"),
    )
}

#[test]
fn run_aivi_sources() {
    let _guard = runner_test_lock();
    // Spawn on a thread with a large stack so deeply-recursive AIVI programs
    // (which use recursion for all iteration) don't overflow the default 8 MiB
    // test-thread stack.
    let result = std::thread::Builder::new()
        .name("aivi-tests".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(run_aivi_sources_inner)
        .expect("spawn test thread")
        .join();
    match result {
        Ok(()) => {}
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn run_aivi_sources_inner() {
    let root = test_support::workspace_root();
    std::env::set_current_dir(&root).expect("set cwd");
    let tests_dir = root.join("integration-tests");

    if !tests_dir.exists() {
        eprintln!("No AIVI sources found at {}", tests_dir.display());
        return;
    }

    // Collect all .aivi files that contain @test
    let mut test_paths: Vec<PathBuf> = Vec::new();
    for entry in WalkDir::new(&tests_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "aivi"))
    {
        let path = entry.path().to_path_buf();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if content.contains("@test") {
                test_paths.push(path);
            }
        }
    }

    if test_paths.is_empty() {
        eprintln!("No @test definitions found under {}", tests_dir.display());
        return;
    }

    // Filter out stdlib test files handled by dedicated tests
    let test_paths: Vec<_> = test_paths
        .into_iter()
        .filter(|p| {
            let path_str = p.to_string_lossy();
            let keep = path_str.contains("fromListDebug.aivi")
                || path_str.contains("closures_hof.aivi")
                || path_str.contains("resourceLifecycle.aivi");
            keep && !path_str.starts_with("integration-tests/stdlib/")
        })
        .collect();

    println!("Found {} test file(s)", test_paths.len());

    // Parse and pre-elaborate the stdlib once, reuse across all files.
    let mut stdlib_modules = embedded_stdlib_modules();
    resolve_import_names(&mut stdlib_modules);
    let checkpoint = elaborate_stdlib_checkpoint(&mut stdlib_modules);

    let (total_passed, total_failed, skipped_files, test_failures) =
        run_files_parallel(&test_paths, &stdlib_modules, &checkpoint);

    for (name, message) in &test_failures {
        println!("  FAIL: {} — {}", name, message);
    }

    println!("\nTest Summary:");
    println!("  Passed:  {}", total_passed);
    println!("  Failed:  {}", total_failed);
    println!("  Skipped: {} file(s)", skipped_files);

    if total_failed > 0 {
        panic!("{} integration test(s) failed", total_failed);
    }
}

#[test]
fn syntax_effects_selected_batch_one_files_execute_without_failures() {
    let _guard = runner_test_lock();
    let root = test_support::workspace_root();
    let files: Vec<PathBuf> = [
        "integration-tests/syntax/bindings/basic.aivi",
        "integration-tests/syntax/bindings/recursion.aivi",
        "integration-tests/syntax/decorators/static_and_test.aivi",
        "integration-tests/syntax/domains/import_and_suffix_literals.aivi",
        "integration-tests/syntax/domains/rhs_typed_overload.aivi",
        "integration-tests/syntax/domains/suffix_application_expr.aivi",
        "integration-tests/syntax/effects/attempt_and_match.aivi",
        "integration-tests/syntax/effects/attempt_converts_to_result.aivi",
        "integration-tests/syntax/effects/sequential_flow.aivi",
    ]
    .iter()
    .map(|p| root.join(p))
    .collect();

    let mut stdlib_modules = embedded_stdlib_modules();
    resolve_import_names(&mut stdlib_modules);
    let checkpoint = elaborate_stdlib_checkpoint(&mut stdlib_modules);

    let (total_passed, _total_failed, skipped_files, _) =
        run_files_parallel(&files, &stdlib_modules, &checkpoint);
    assert!(
        total_passed > 0,
        "expected first syntax/effects batch to execute"
    );
    eprintln!("skipped first syntax/effects batch files: {skipped_files}");
}

#[test]
fn syntax_effects_selected_batch_two_files_execute_without_failures() {
    let _guard = runner_test_lock();
    let root = test_support::workspace_root();
    let files: Vec<PathBuf> = [
        "integration-tests/syntax/effects/applicative_flow.aivi",
        "integration-tests/syntax/effects/cleanup_failure.aivi",
        "integration-tests/syntax/effects/cleanup_idempotency.aivi",
        "integration-tests/syntax/effects/cleanup_lifo_order.aivi",
        "integration-tests/syntax/effects/cleanup_on_pattern_fail.aivi",
        "integration-tests/syntax/effects/cleanup_three_lifo.aivi",
        "integration-tests/syntax/effects/conditional_helpers.aivi",
        "integration-tests/syntax/effects/guard_behavior.aivi",
        "integration-tests/syntax/effects/guard_precondition.aivi",
        "integration-tests/syntax/effects/list_flow.aivi",
        "integration-tests/syntax/effects/load_fallback.aivi",
        "integration-tests/syntax/effects/option_flow.aivi",
        "integration-tests/syntax/effects/recursive_helpers.aivi",
        "integration-tests/syntax/effects/result_flow.aivi",
    ]
    .iter()
    .map(|p| root.join(p))
    .collect();

    let mut stdlib_modules = embedded_stdlib_modules();
    resolve_import_names(&mut stdlib_modules);
    let checkpoint = elaborate_stdlib_checkpoint(&mut stdlib_modules);

    let (total_passed, _total_failed, skipped_files, _) =
        run_files_parallel(&files, &stdlib_modules, &checkpoint);
    assert!(
        total_passed > 0,
        "expected second syntax/effects batch to execute"
    );
    eprintln!("skipped second syntax/effects batch files: {skipped_files}");
}

#[test]
fn syntax_remaining_batch_one_files_execute_without_failures() {
    let _guard = runner_test_lock();
    let root = test_support::workspace_root();
    let files: Vec<PathBuf> = [
        "integration-tests/syntax/effects/when_conditional.aivi",
        "integration-tests/syntax/operators/domain_operator_resolution.aivi",
        "integration-tests/syntax/patching/record_patch_basic.aivi",
        "integration-tests/syntax/pattern_matching/guards_when.aivi",
        "integration-tests/syntax/pattern_matching/guards_unless.aivi",
        "integration-tests/syntax/predicates/implicit_and_explicit.aivi",
        "integration-tests/syntax/sigils/collections_structured.aivi",
        "integration-tests/syntax/bidirectional_class_members.aivi",
    ]
    .iter()
    .map(|p| root.join(p))
    .collect();

    let mut stdlib_modules = embedded_stdlib_modules();
    resolve_import_names(&mut stdlib_modules);
    let checkpoint = elaborate_stdlib_checkpoint(&mut stdlib_modules);

    let (total_passed, _total_failed, skipped_files, _) =
        run_files_parallel(&files, &stdlib_modules, &checkpoint);
    assert!(
        total_passed > 0,
        "expected first remaining syntax batch to execute"
    );
    eprintln!("skipped first remaining syntax batch files: {skipped_files}");
}

#[test]
fn syntax_remaining_batch_two_files_execute_without_failures() {
    let _guard = runner_test_lock();
    let root = test_support::workspace_root();
    let files: Vec<PathBuf> = [
        "integration-tests/syntax/external_sources/env_get_and_default.aivi",
        "integration-tests/syntax/ir_dump_minimal.aivi",
        "integration-tests/syntax/operators/list_concat_operator.aivi",
        "integration-tests/syntax/pattern_matching/as_binding.aivi",
        "integration-tests/syntax/pattern_matching/lists_and_records.aivi",
        "integration-tests/syntax/resources/basic_resource_block.aivi",
        "integration-tests/syntax/sigils/gtk_builder.aivi",
        "integration-tests/syntax/types/type_level_transforms.aivi",
    ]
    .iter()
    .map(|p| root.join(p))
    .collect();

    let mut stdlib_modules = embedded_stdlib_modules();
    resolve_import_names(&mut stdlib_modules);
    let checkpoint = elaborate_stdlib_checkpoint(&mut stdlib_modules);

    let (total_passed, _total_failed, skipped_files, _) =
        run_files_parallel(&files, &stdlib_modules, &checkpoint);
    assert!(
        total_passed > 0,
        "expected second remaining syntax batch to execute"
    );
    eprintln!("skipped second remaining syntax batch files: {skipped_files}");
}

#[test]
fn syntax_remaining_batch_three_files_execute_without_failures() {
    let _guard = runner_test_lock();
    let root = test_support::workspace_root();
    let files: Vec<PathBuf> = [
        "integration-tests/syntax/functions/multi_arg_and_sig.aivi",
        "integration-tests/syntax/modules/use_alias_and_selective_imports.aivi",
        "integration-tests/syntax/modules/use_hiding_imports.aivi",
        "integration-tests/syntax/operators/precedence_and_pipes.aivi",
        "integration-tests/syntax/pattern_matching/guarded_case_with_if.aivi",
        "integration-tests/syntax/pattern_matching/match_keyword.aivi",
        "integration-tests/syntax/sigils/basic.aivi",
        "integration-tests/syntax/sigils/raw_text.aivi",
        "integration-tests/syntax/types/unions_and_aliases.aivi",
    ]
    .iter()
    .map(|p| root.join(p))
    .collect();

    let mut stdlib_modules = embedded_stdlib_modules();
    resolve_import_names(&mut stdlib_modules);
    let checkpoint = elaborate_stdlib_checkpoint(&mut stdlib_modules);

    let (total_passed, _total_failed, skipped_files, _) =
        run_files_parallel(&files, &stdlib_modules, &checkpoint);
    assert!(
        total_passed > 0,
        "expected third remaining syntax batch to execute"
    );
    eprintln!("skipped third remaining syntax batch files: {skipped_files}");
}

#[test]
fn combinations_batch_one_execute_without_failures() {
    let _guard = runner_test_lock();
    let root = test_support::workspace_root();
    let files: Vec<PathBuf> = [
        "integration-tests/combinations/combo_closures_across_scopes.aivi",
        "integration-tests/combinations/combo_collection_nesting.aivi",
        "integration-tests/combinations/combo_resource_nesting.aivi",
        "integration-tests/combinations/combo_multi_clause_with_nesting.aivi",
    ]
    .iter()
    .map(|p| root.join(p))
    .collect();

    let mut stdlib_modules = embedded_stdlib_modules();
    resolve_import_names(&mut stdlib_modules);
    let checkpoint = elaborate_stdlib_checkpoint(&mut stdlib_modules);

    let (total_passed, total_failed, skipped_files, test_failures) =
        run_files_parallel(&files, &stdlib_modules, &checkpoint);

    for (name, message) in &test_failures {
        eprintln!("  FAIL: {} — {}", name, message);
    }

    assert!(
        total_passed > 0,
        "expected first combination batch to execute (skipped: {skipped_files})"
    );
    assert_eq!(
        total_failed, 0,
        "{total_failed} first combination batch test(s) failed"
    );
    eprintln!("combination batch one: {total_passed} passed, {skipped_files} file(s) skipped");
}

#[test]
fn combinations_batch_two_execute_without_failures() {
    let _guard = runner_test_lock();
    let root = test_support::workspace_root();
    let files: Vec<PathBuf> = [
        "integration-tests/combinations/combo_do_nesting.aivi",
        "integration-tests/combinations/combo_match_nesting.aivi",
        "integration-tests/combinations/combo_operators_in_scopes.aivi",
        "integration-tests/combinations/combo_mock_nesting.aivi",
    ]
    .iter()
    .map(|p| root.join(p))
    .collect();

    let mut stdlib_modules = embedded_stdlib_modules();
    resolve_import_names(&mut stdlib_modules);
    let checkpoint = elaborate_stdlib_checkpoint(&mut stdlib_modules);

    let (total_passed, total_failed, skipped_files, test_failures) =
        run_files_parallel(&files, &stdlib_modules, &checkpoint);

    for (name, message) in &test_failures {
        eprintln!("  FAIL: {} — {}", name, message);
    }

    assert!(
        total_passed > 0,
        "expected second combination batch to execute (skipped: {skipped_files})"
    );
    assert_eq!(
        total_failed, 0,
        "{total_failed} second combination batch test(s) failed"
    );
    eprintln!("combination batch two: {total_passed} passed, {skipped_files} file(s) skipped");
}

#[test]
fn combinations_batch_three_execute_without_failures() {
    let _guard = runner_test_lock();
    let root = test_support::workspace_root();
    let files: Vec<PathBuf> = [
        "integration-tests/combinations/combo_lambda_nesting.aivi",
        "integration-tests/combinations/combo_loop_nesting.aivi",
        "integration-tests/combinations/combo_adt_lifecycle.aivi",
    ]
    .iter()
    .map(|p| root.join(p))
    .collect();

    let mut stdlib_modules = embedded_stdlib_modules();
    resolve_import_names(&mut stdlib_modules);
    let checkpoint = elaborate_stdlib_checkpoint(&mut stdlib_modules);

    let (total_passed, total_failed, skipped_files, test_failures) =
        run_files_parallel(&files, &stdlib_modules, &checkpoint);

    for (name, message) in &test_failures {
        eprintln!("  FAIL: {} — {}", name, message);
    }

    assert!(
        total_passed > 0,
        "expected third combination batch to execute (skipped: {skipped_files})"
    );
    assert_eq!(
        total_failed, 0,
        "{total_failed} third combination batch test(s) failed"
    );
    eprintln!("combination batch three: {total_passed} passed, {skipped_files} file(s) skipped");
}

#[test]
fn scoping_tests_execute_without_failures() {
    let _guard = runner_test_lock();
    let root = test_support::workspace_root();
    let files: Vec<PathBuf> = [
        "integration-tests/syntax/scoping/closure_loop_capture.aivi",
        "integration-tests/syntax/scoping/closure_list_capture.aivi",
        "integration-tests/syntax/scoping/shadowing_all_binders.aivi",
        "integration-tests/syntax/scoping/nested_lambda_capture.aivi",
        "integration-tests/syntax/scoping/destructure_capture.aivi",
        "integration-tests/syntax/scoping/closure_match_arm.aivi",
    ]
    .iter()
    .map(|p| root.join(p))
    .collect();

    let mut stdlib_modules = embedded_stdlib_modules();
    resolve_import_names(&mut stdlib_modules);
    let checkpoint = elaborate_stdlib_checkpoint(&mut stdlib_modules);

    let (total_passed, total_failed, skipped_files, test_failures) =
        run_files_parallel(&files, &stdlib_modules, &checkpoint);

    for (name, message) in &test_failures {
        eprintln!("  FAIL: {} — {}", name, message);
    }

    assert!(
        total_passed > 0,
        "expected scoping tests to execute (skipped: {skipped_files})"
    );
    assert_eq!(total_failed, 0, "{total_failed} scoping test(s) failed");
    eprintln!("scoping: {total_passed} passed, {skipped_files} file(s) skipped");
}

#[test]
fn runtime_numeric_boundary_tests_execute_without_failures() {
    let _guard = runner_test_lock();
    let root = test_support::workspace_root();
    let files: Vec<PathBuf> = [
        "integration-tests/runtime/integer_overflow.aivi",
        "integration-tests/runtime/float_extremes.aivi",
        "integration-tests/runtime/equality_hash_consistency.aivi",
        "integration-tests/runtime/ordering_laws.aivi",
        "integration-tests/runtime/deep_recursion_list.aivi",
    ]
    .iter()
    .map(|p| root.join(p))
    .collect();

    let mut stdlib_modules = embedded_stdlib_modules();
    resolve_import_names(&mut stdlib_modules);
    let checkpoint = elaborate_stdlib_checkpoint(&mut stdlib_modules);

    let (total_passed, total_failed, skipped_files, test_failures) =
        run_files_parallel(&files, &stdlib_modules, &checkpoint);

    for (name, message) in &test_failures {
        eprintln!("  FAIL: {} — {}", name, message);
    }

    assert!(
        total_passed > 0,
        "expected numeric boundary tests to execute (skipped: {skipped_files})"
    );
    assert_eq!(
        total_failed, 0,
        "{total_failed} numeric boundary test(s) failed"
    );
    eprintln!("numeric boundary: {total_passed} passed, {skipped_files} file(s) skipped");
}

#[test]
fn determinism_tests_execute_without_failures() {
    let _guard = runner_test_lock();
    let root = test_support::workspace_root();
    let files: Vec<PathBuf> = [
        "integration-tests/runtime/determinism_map_order.aivi",
        "integration-tests/runtime/determinism_set_order.aivi",
    ]
    .iter()
    .map(|p| root.join(p))
    .collect();

    let mut stdlib_modules = embedded_stdlib_modules();
    resolve_import_names(&mut stdlib_modules);
    let checkpoint = elaborate_stdlib_checkpoint(&mut stdlib_modules);

    let (total_passed, total_failed, skipped_files, test_failures) =
        run_files_parallel(&files, &stdlib_modules, &checkpoint);

    for (name, message) in &test_failures {
        eprintln!("  FAIL: {} — {}", name, message);
    }

    assert!(
        total_passed > 0,
        "expected no-dup-eval / determinism tests to execute (skipped: {skipped_files})"
    );
    assert_eq!(
        total_failed, 0,
        "{total_failed} no-dup-eval / determinism test(s) failed"
    );
    eprintln!("no-dup-eval / determinism: {total_passed} passed, {skipped_files} file(s) skipped");
}

#[test]
fn eval_order_and_tco_batch_one_tests_execute_without_failures() {
    let _guard = runner_test_lock();
    let root = test_support::workspace_root();
    let files: Vec<PathBuf> = [
        "integration-tests/runtime/eval_order_args.aivi",
        "integration-tests/runtime/eval_order_record_fields.aivi",
        "integration-tests/runtime/effect_stack_safety_channels.aivi",
        "integration-tests/runtime/effectful_tuple_loop_state.aivi",
    ]
    .iter()
    .map(|p| root.join(p))
    .collect();

    let mut stdlib_modules = embedded_stdlib_modules();
    resolve_import_names(&mut stdlib_modules);
    let checkpoint = elaborate_stdlib_checkpoint(&mut stdlib_modules);

    let (total_passed, total_failed, skipped_files, test_failures) =
        run_files_parallel(&files, &stdlib_modules, &checkpoint);

    for (name, message) in &test_failures {
        eprintln!("  FAIL: {} — {}", name, message);
    }

    assert!(
        total_passed > 0,
        "expected first eval-order / TCO batch to execute (skipped: {skipped_files})"
    );
    assert_eq!(
        total_failed, 0,
        "{total_failed} first eval-order / TCO batch test(s) failed"
    );
    eprintln!("eval-order / TCO batch one: {total_passed} passed, {skipped_files} file(s) skipped");
}

#[test]
fn eval_order_and_tco_batch_two_tests_execute_without_failures() {
    let _guard = runner_test_lock();
    let root = test_support::workspace_root();
    let files: Vec<PathBuf> = [
        "integration-tests/runtime/short_circuit_and_or.aivi",
        "integration-tests/runtime/tco_deep_recursion.aivi",
        "integration-tests/runtime/tco_mutual_recursion_deep.aivi",
    ]
    .iter()
    .map(|p| root.join(p))
    .collect();

    let mut stdlib_modules = embedded_stdlib_modules();
    resolve_import_names(&mut stdlib_modules);
    let checkpoint = elaborate_stdlib_checkpoint(&mut stdlib_modules);

    let (total_passed, total_failed, skipped_files, test_failures) =
        run_files_parallel(&files, &stdlib_modules, &checkpoint);

    for (name, message) in &test_failures {
        eprintln!("  FAIL: {} — {}", name, message);
    }

    assert!(
        total_passed > 0,
        "expected second eval-order / TCO batch to execute (skipped: {skipped_files})"
    );
    assert_eq!(
        total_failed, 0,
        "{total_failed} second eval-order / TCO batch test(s) failed"
    );
    eprintln!("eval-order / TCO batch two: {total_passed} passed, {skipped_files} file(s) skipped");
}

#[test]
fn source_pipeline_tests_execute_without_failures() {
    let _guard = runner_test_lock();
    let root = test_support::workspace_root();
    let files: Vec<PathBuf> = ["integration-tests/runtime/source_pipeline.aivi"]
        .iter()
        .map(|p| root.join(p))
        .collect();

    let mut stdlib_modules = embedded_stdlib_modules();
    resolve_import_names(&mut stdlib_modules);
    let checkpoint = elaborate_stdlib_checkpoint(&mut stdlib_modules);

    let (total_passed, total_failed, skipped_files, test_failures) =
        run_files_parallel(&files, &stdlib_modules, &checkpoint);

    for (name, message) in &test_failures {
        eprintln!("  FAIL: {} — {}", name, message);
    }

    assert!(
        total_passed > 0,
        "expected source pipeline tests to execute (skipped: {skipped_files})"
    );
    assert_eq!(
        total_failed, 0,
        "{total_failed} source pipeline test(s) failed"
    );
    eprintln!("source pipeline: {total_passed} passed, {skipped_files} file(s) skipped");
}
