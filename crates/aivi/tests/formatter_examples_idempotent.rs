use std::fs;
use std::path::{Path, PathBuf};

use aivi::format_text;

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn collect_aivi_files(dir: &Path, out: &mut Vec<PathBuf>) {
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

fn first_diff(a: &str, b: &str) -> Option<(usize, String, String)> {
    let mut a_lines = a.lines();
    let mut b_lines = b.lines();
    let mut line_no = 1usize;
    loop {
        let la = a_lines.next();
        let lb = b_lines.next();
        match (la, lb) {
            (None, None) => return None,
            (Some(x), Some(y)) if x == y => {
                line_no += 1;
                continue;
            }
            (Some(x), Some(y)) => return Some((line_no, x.to_string(), y.to_string())),
            (Some(x), None) => return Some((line_no, x.to_string(), String::new())),
            (None, Some(y)) => return Some((line_no, String::new(), y.to_string())),
        }
    }
}

#[test]
fn examples_are_formatter_idempotent() {
    let root = workspace_root();
    let examples_dir = root.join("examples");

    let mut files = Vec::new();
    collect_aivi_files(&examples_dir, &mut files);
    assert!(!files.is_empty(), "no .aivi files found under examples/");

    let mut failures = Vec::new();
    for path in files {
        let input = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let output = format_text(&input);
        if input != output {
            let rel = path.strip_prefix(&root).unwrap_or(&path).display().to_string();
            let diff = first_diff(&input, &output);
            failures.push((rel, diff));
        }
    }

    if failures.is_empty() {
        return;
    }

    let mut report = String::new();
    for (path, diff) in failures {
        report.push_str(&format!("{path}\n"));
        if let Some((line, a, b)) = diff {
            report.push_str(&format!("  first diff at line {line}\n"));
            report.push_str(&format!("  in : {a:?}\n"));
            report.push_str(&format!("  out: {b:?}\n"));
        } else {
            report.push_str("  outputs differed\n");
        }
    }
    panic!("formatter is not idempotent on examples:\n{report}");
}

