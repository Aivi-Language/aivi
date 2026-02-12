use std::path::{Path, PathBuf};

use aivi::{desugar_target, run_native};
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

#[test]
fn run_aivi_sources() {
    let root = set_workspace_root();
    let tests_dir = root.join("crates/aivi/tests/aivi_sources");

    if !tests_dir.exists() {
        eprintln!("No AIVI sources found at {}", tests_dir.display());
        return;
    }

    let mut failed_tests = Vec::new();
    let mut passed_count = 0;

    for entry in WalkDir::new(&tests_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "aivi"))
    {
        let path = entry.path();
        let rel_path = path.strip_prefix(&root).unwrap_or(path);
        let path_str = rel_path.to_string_lossy();

        println!("Running test: {}", path_str);

        match run_test(rel_path) {
            Ok(_) => {
                println!("PASS: {}", path_str);
                passed_count += 1;
            }
            Err(e) => {
                println!("FAIL: {}", path_str);
                eprintln!("Error: {}", e);
                failed_tests.push((path_str.to_string(), e));
            }
        }
    }

    println!("\nTest Summary:");
    println!("Passed: {}", passed_count);
    println!("Failed: {}", failed_tests.len());

    if !failed_tests.is_empty() {
        panic!("{} tests failed", failed_tests.len());
    }
}

fn run_test(path: &Path) -> Result<(), String> {
    let path_str = path.to_string_lossy();
    let program = desugar_target(&path_str).map_err(|e| format!("Desugar failed: {}", e))?;

    run_native(program).map_err(|e| format!("Runtime failed: {}", e))
}
