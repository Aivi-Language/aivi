//! Compile-fail fixture runner.
//!
//! Loads `.aivi` files from `integration-tests/compile_fail/` and asserts that
//! each produces the expected diagnostic (error or warning) declared in
//! `// EXPECT-ERROR: <substring>` or `// EXPECT-WARN: <substring>` comments.

use std::path::{Path, PathBuf};

use aivi::{
    check_modules, check_types, elaborate_expected_coercions, file_diagnostics_have_errors,
    load_modules_from_paths, DiagnosticSeverity, FileDiagnostic,
};

#[path = "test_support.rs"]
#[allow(dead_code)]
mod test_support;

struct Expectation {
    kind: DiagnosticSeverity,
    substring: String,
}

fn parse_expectations(source: &str) -> Vec<Expectation> {
    let mut expectations = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("// EXPECT-ERROR:") {
            expectations.push(Expectation {
                kind: DiagnosticSeverity::Error,
                substring: rest.trim().to_string(),
            });
        } else if let Some(rest) = trimmed.strip_prefix("// EXPECT-WARN:") {
            expectations.push(Expectation {
                kind: DiagnosticSeverity::Warning,
                substring: rest.trim().to_string(),
            });
        }
    }
    expectations
}

fn diagnostics_match(diags: &[FileDiagnostic], exp: &Expectation) -> bool {
    diags.iter().any(|d| {
        d.diagnostic.severity == exp.kind
            && (d.diagnostic.code.contains(&exp.substring)
                || d.diagnostic.message.contains(&exp.substring))
    })
}

#[test]
fn compile_fail_fixtures_produce_expected_diagnostics() {
    let root = test_support::workspace_root();
    let compile_fail_dir = root.join("integration-tests/compile_fail");

    if !compile_fail_dir.exists() {
        eprintln!("No compile_fail directory found");
        return;
    }

    // Single-file entries (direct .aivi files in compile_fail/).
    let mut single_files: Vec<PathBuf> = std::fs::read_dir(&compile_fail_dir)
        .expect("read compile_fail dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "aivi"))
        .collect();
    single_files.sort();

    // Multi-file groups: each subdirectory is a test group whose .aivi files are
    // loaded together (simulates cross-module interactions).
    let mut subdir_groups: Vec<Vec<PathBuf>> = std::fs::read_dir(&compile_fail_dir)
        .expect("read compile_fail dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .map(|dir| {
            let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
                .expect("read subdir")
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|ext| ext == "aivi"))
                .collect();
            files.sort();
            files
        })
        .filter(|g| !g.is_empty())
        .collect();
    subdir_groups.sort();

    assert!(
        !single_files.is_empty() || !subdir_groups.is_empty(),
        "No .aivi files found in compile_fail/"
    );

    let mut passed = 0usize;
    let mut failed_cases: Vec<String> = Vec::new();

    for path in &single_files {
        run_test_group(
            std::slice::from_ref(path),
            &root,
            &mut passed,
            &mut failed_cases,
        );
    }
    for group in &subdir_groups {
        run_test_group(group, &root, &mut passed, &mut failed_cases);
    }

    println!(
        "\nCompile-fail summary: {} passed, {} failed",
        passed,
        failed_cases.len()
    );

    if !failed_cases.is_empty() {
        for msg in &failed_cases {
            eprintln!("  FAIL: {msg}");
        }
        panic!("{} compile-fail fixture(s) failed", failed_cases.len());
    }
}

fn run_test_group(
    paths: &[PathBuf],
    root: &Path,
    passed: &mut usize,
    failed_cases: &mut Vec<String>,
) {
    // Collect EXPECT annotations from all files in the group.
    let mut expectations: Vec<Expectation> = Vec::new();
    for path in paths {
        let source = std::fs::read_to_string(path).expect("read source");
        expectations.extend(parse_expectations(&source));
    }

    let display = if paths.len() == 1 {
        paths[0]
            .strip_prefix(root)
            .unwrap_or(&paths[0])
            .display()
            .to_string()
    } else {
        paths[0]
            .parent()
            .and_then(|p| p.strip_prefix(root).ok())
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    };

    if expectations.is_empty() {
        failed_cases.push(format!(
            "{}: no EXPECT-ERROR or EXPECT-WARN comments found",
            display
        ));
        return;
    }

    // Load all modules in the group together.
    let mut modules = match load_modules_from_paths(paths) {
        Ok(m) => m,
        Err(e) => {
            let err_msg = format!("{e}");
            let all_matched = expectations.iter().all(|exp| {
                exp.kind == DiagnosticSeverity::Error && err_msg.contains(&exp.substring)
            });
            if all_matched {
                println!("PASS: {display}");
                *passed += 1;
            } else {
                failed_cases.push(format!(
                    "{}: load error '{}' did not match expectations",
                    display, err_msg
                ));
            }
            return;
        }
    };

    let mut diags = check_modules(&modules);
    if !file_diagnostics_have_errors(&diags) {
        diags.extend(elaborate_expected_coercions(&mut modules));
    }
    diags.extend(check_types(&modules));
    diags.retain(|d| !d.path.starts_with("<embedded:"));

    let mut all_matched = true;
    for exp in &expectations {
        if !diagnostics_match(&diags, exp) {
            let kind_str = match exp.kind {
                DiagnosticSeverity::Error => "ERROR",
                DiagnosticSeverity::Warning => "WARN",
            };
            let actual: Vec<String> = diags
                .iter()
                .map(|d| {
                    format!(
                        "[{:?}] {} {}",
                        d.diagnostic.severity, d.diagnostic.code, d.diagnostic.message
                    )
                })
                .collect();
            failed_cases.push(format!(
                "{}: expected {} containing '{}', got: {:?}",
                display, kind_str, exp.substring, actual
            ));
            all_matched = false;
        }
    }

    let has_error_expectations = expectations
        .iter()
        .any(|e| matches!(e.kind, DiagnosticSeverity::Error));
    if has_error_expectations && !file_diagnostics_have_errors(&diags) {
        failed_cases.push(format!(
            "{}: expected compile errors but file compiled successfully",
            display
        ));
        all_matched = false;
    }

    if all_matched {
        println!("PASS: {display}");
        *passed += 1;
    }
}
