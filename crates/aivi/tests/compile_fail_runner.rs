//! Compile-fail fixture runner.
//!
//! Loads `.aivi` files from `integration-tests/compile_fail/` and asserts that
//! each produces the expected diagnostic (error or warning) declared in
//! `// EXPECT-ERROR: <substring>` or `// EXPECT-WARN: <substring>` comments.

use std::path::PathBuf;

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

    let mut entries: Vec<PathBuf> = std::fs::read_dir(&compile_fail_dir)
        .expect("read compile_fail dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "aivi"))
        .collect();
    entries.sort();

    assert!(!entries.is_empty(), "No .aivi files found in compile_fail/");

    let mut passed = 0usize;
    let mut failed_cases: Vec<String> = Vec::new();

    for path in &entries {
        let source = std::fs::read_to_string(path).expect("read source");
        let expectations = parse_expectations(&source);
        let rel = path.strip_prefix(&root).unwrap_or(path);

        if expectations.is_empty() {
            failed_cases.push(format!(
                "{}: no EXPECT-ERROR or EXPECT-WARN comments found",
                rel.display()
            ));
            continue;
        }

        // Load modules (may produce parse errors)
        let mut modules = match load_modules_from_paths(std::slice::from_ref(path)) {
            Ok(m) => m,
            Err(e) => {
                // Load failure itself may satisfy an error expectation
                let err_msg = format!("{e}");
                let all_matched = expectations.iter().all(|exp| {
                    exp.kind == DiagnosticSeverity::Error && err_msg.contains(&exp.substring)
                });
                if all_matched {
                    passed += 1;
                } else {
                    failed_cases.push(format!(
                        "{}: load error '{}' did not match expectations",
                        rel.display(),
                        err_msg
                    ));
                }
                continue;
            }
        };

        // Run type-checking to produce diagnostics (including exhaustiveness/type checks)
        let mut diags = check_modules(&modules);
        if !file_diagnostics_have_errors(&diags) {
            diags.extend(elaborate_expected_coercions(&mut modules));
        }
        diags.extend(check_types(&modules));
        // Keep only user-file diagnostics
        diags.retain(|d| !d.path.starts_with("<embedded:"));

        // Check each expectation
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
                    rel.display(),
                    kind_str,
                    exp.substring,
                    actual
                ));
                all_matched = false;
            }
        }

        // For EXPECT-ERROR, the file must actually have errors
        let has_error_expectations = expectations
            .iter()
            .any(|e| matches!(e.kind, DiagnosticSeverity::Error));
        if has_error_expectations && !file_diagnostics_have_errors(&diags) {
            failed_cases.push(format!(
                "{}: expected compile errors but file compiled successfully",
                rel.display()
            ));
            all_matched = false;
        }

        if all_matched {
            println!("PASS: {}", rel.display());
            passed += 1;
        }
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
