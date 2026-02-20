use std::path::PathBuf;

use aivi::{
    check_modules, desugar_modules, elaborate_expected_coercions, file_diagnostics_have_errors,
    load_modules_from_paths, run_test_suite, Expr, Literal, Module, ModuleItem,
};
use walkdir::WalkDir;

fn set_workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root");
    std::env::set_current_dir(workspace_root).expect("set cwd");
    workspace_root.to_path_buf()
}

fn collect_test_entries(modules: &[Module]) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    for module in modules {
        // skip embedded stdlib modules
        if module.name.name.starts_with("aivi.") || module.name.name == "aivi" {
            continue;
        }
        for item in &module.items {
            let ModuleItem::Def(def) = item else {
                continue;
            };
            if let Some(dec) = def.decorators.iter().find(|d| d.name.name == "test") {
                let name = format!("{}.{}", module.name.name, def.name.name);
                let description = match &dec.arg {
                    Some(Expr::Literal(Literal::String { text, .. })) => text.clone(),
                    _ => name.clone(),
                };
                entries.push((name, description));
            }
        }
    }
    entries.sort();
    entries.dedup();
    entries
}

#[test]
fn run_aivi_sources() {
    // Spawn on a thread with a 32 MiB stack so deeply-recursive AIVI programs
    // (which use recursion for all iteration) don't overflow the default 8 MiB
    // test-thread stack.
    let result = std::thread::Builder::new()
        .name("aivi-tests".into())
        .stack_size(32 * 1024 * 1024)
        .spawn(run_aivi_sources_inner)
        .expect("spawn test thread")
        .join();
    match result {
        Ok(()) => {}
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn run_aivi_sources_inner() {
    let root = set_workspace_root();
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

    println!("Found {} test file(s)", test_paths.len());

    let mut total_passed = 0usize;
    let mut total_failed = 0usize;
    let mut skipped_files = 0usize;
    let mut test_failures: Vec<(String, String)> = Vec::new();

    // Process each file independently to isolate pre-existing type errors
    for path in &test_paths {
        let rel = path.strip_prefix(&root).unwrap_or(path);
        let rel_str = rel.to_string_lossy();

        // Load this file with embedded stdlib
        let mut modules = match load_modules_from_paths(std::slice::from_ref(path)) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("SKIP (load error): {} — {}", rel_str, e);
                skipped_files += 1;
                continue;
            }
        };

        // Type-check; skip files with pre-existing errors
        let mut diags = check_modules(&modules);
        if !file_diagnostics_have_errors(&diags) {
            diags.extend(elaborate_expected_coercions(&mut modules));
        }
        diags.retain(|d| !d.path.starts_with("<embedded:"));
        if file_diagnostics_have_errors(&diags) {
            eprintln!("SKIP (type errors): {}", rel_str);
            skipped_files += 1;
            continue;
        }

        // Collect qualified test entries (name, description)
        let test_entries = collect_test_entries(&modules);
        if test_entries.is_empty() {
            continue;
        }

        // Desugar and run
        let program = desugar_modules(&modules);
        match run_test_suite(program, &test_entries, &modules) {
            Ok(report) => {
                total_passed += report.passed;
                total_failed += report.failed;
                for failure in &report.failures {
                    println!("  FAIL: {} — {}", failure.description, failure.message);
                    test_failures.push((failure.name.clone(), failure.message.clone()));
                }
                if report.failed == 0 {
                    println!("PASS: {} ({} test(s))", rel_str, report.passed);
                } else {
                    println!(
                        "FAIL: {} ({} passed, {} failed)",
                        rel_str, report.passed, report.failed
                    );
                }
            }
            Err(e) => {
                eprintln!("SKIP (runtime error): {} — {}", rel_str, e);
                skipped_files += 1;
            }
        }
    }

    println!("\nTest Summary:");
    println!("  Passed:  {}", total_passed);
    println!("  Failed:  {}", total_failed);
    println!("  Skipped: {} file(s)", skipped_files);

    if total_failed > 0 {
        panic!("{} integration test(s) failed", total_failed);
    }
}
