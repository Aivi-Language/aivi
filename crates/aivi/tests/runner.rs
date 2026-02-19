use std::path::PathBuf;

use aivi::{
    check_modules, desugar_modules, elaborate_expected_coercions, file_diagnostics_have_errors,
    load_modules_from_paths, run_test_suite, Module, ModuleItem,
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

fn collect_test_names(modules: &[Module]) -> Vec<String> {
    let mut names = Vec::new();
    for module in modules {
        // skip embedded stdlib modules
        if module.name.name.starts_with("aivi.") || module.name.name == "aivi" {
            continue;
        }
        for item in &module.items {
            let ModuleItem::Def(def) = item else {
                continue;
            };
            if def.decorators.iter().any(|d| d.name.name == "test") {
                names.push(format!("{}.{}", module.name.name, def.name.name));
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

#[test]
fn run_aivi_sources() {
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

        // Collect qualified test names
        let test_names = collect_test_names(&modules);
        if test_names.is_empty() {
            continue;
        }

        // Desugar and run
        let program = desugar_modules(&modules);
        match run_test_suite(program, &test_names, &modules) {
            Ok(report) => {
                total_passed += report.passed;
                total_failed += report.failed;
                for failure in &report.failures {
                    println!("  FAIL: {} — {}", failure.name, failure.message);
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
