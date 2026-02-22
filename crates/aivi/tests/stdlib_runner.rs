use std::path::{Path, PathBuf};

use aivi::{
    check_modules, desugar_modules, elaborate_expected_coercions, file_diagnostics_have_errors,
    load_modules_from_paths, run_test_suite, Expr, Literal, Module, ModuleItem,
};

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn collect_test_entries(modules: &[Module]) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    for module in modules {
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

    let tests = collect_test_entries(&modules);
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
    let root = workspace_root();
    let files = [
        root.join("integration-tests/stdlib/aivi/collections/collections.aivi"),
        root.join("integration-tests/stdlib/aivi/text/text.aivi"),
        root.join("integration-tests/stdlib/aivi/prelude/prelude.aivi"),
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
