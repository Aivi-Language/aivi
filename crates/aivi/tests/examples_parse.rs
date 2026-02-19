use std::fs;
use std::path::PathBuf;

use aivi::parse_file;

#[test]
fn examples_parse_without_diagnostics() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root");
    let examples_dir = workspace_root.join("integration-tests");

    let mut failures: Vec<(PathBuf, Vec<String>)> = Vec::new();

    fn collect_aivi_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
            .map(|e| e.expect("dir entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                collect_aivi_files(&path, out);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) == Some("aivi") {
                out.push(path);
            }
        }
    }

    let mut files = Vec::new();
    collect_aivi_files(&examples_dir, &mut files);
    assert!(
        !files.is_empty(),
        "no .aivi files found under integration-tests/"
    );

    for path in files {
        let file = parse_file(&path).expect("parse integration test");
        let errors: Vec<_> = file
            .diagnostics
            .iter()
            .filter(|d| d.severity == aivi::DiagnosticSeverity::Error)
            .collect();
        if errors.is_empty() {
            continue;
        }
        let messages = errors
            .into_iter()
            .map(|diag| format!("{}: {}", diag.code, diag.message))
            .collect();
        failures.push((path, messages));
    }

    if failures.is_empty() {
        return;
    }

    let mut report = String::new();
    for (path, messages) in failures {
        let rel = path.strip_prefix(workspace_root).unwrap_or(&path);
        report.push_str(&format!("{}\n", rel.display()));
        for message in messages {
            report.push_str(&format!("  {message}\n"));
        }
    }
    panic!("integration-tests contain diagnostics:\n{report}");
}
