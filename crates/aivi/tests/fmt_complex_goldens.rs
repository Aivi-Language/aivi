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

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
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
fn complex_examples_match_formatter_output() {
    let root = workspace_root();
    let dir = root.join("integration-tests/complex");
    let files = [
        "aStar.aivi",
        "dinic.aivi",
        "edmondsKarp.aivi",
        "fenwickTree.aivi",
        "persistentSegmentTree.aivi",
        "sccTarjan.aivi",
        "topologicalSort.aivi",
    ];

    let mut failures = Vec::new();
    for name in files {
        let path = dir.join(name);
        let input = read(&path);
        let output = format_text(&input);
        if input != output {
            failures.push((name.to_string(), first_diff(&input, &output)));
        }
    }

    if failures.is_empty() {
        return;
    }

    let mut report = String::new();
    for (name, diff) in failures {
        report.push_str(&format!("{name}\n"));
        if let Some((line, a, b)) = diff {
            report.push_str(&format!("  first diff at line {line}\n"));
            report.push_str(&format!("  in : {a:?}\n"));
            report.push_str(&format!("  out: {b:?}\n"));
        } else {
            report.push_str("  outputs differed\n");
        }
    }
    panic!("complex examples are not formatter-idempotent:\n{report}");
}
