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
    let examples_dir = workspace_root.join("examples");

    let mut failures: Vec<(PathBuf, Vec<String>)> = Vec::new();

    for entry in fs::read_dir(&examples_dir).expect("read examples") {
        let entry = entry.expect("example entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("aivi") {
            continue;
        }
        println!("DEBUG: testing file: {}", path.display());
        let content = fs::read_to_string(&path).expect("read content");
        println!(
            "DEBUG: content snippet: {:?}",
            &content[0..50.min(content.len())]
        );
        let file = parse_file(&path).expect("parse example");
        if file.diagnostics.is_empty() {
            continue;
        }
        let messages = file
            .diagnostics
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
    panic!("examples contain diagnostics:\n{report}");
}
