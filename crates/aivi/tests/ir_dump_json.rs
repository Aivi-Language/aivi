use std::path::{Path, PathBuf};
use std::process::Command;

fn aivi_exe() -> Option<String> {
    std::env::var("CARGO_BIN_EXE_aivi").ok()
}

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn truncate_for_error(s: &str) -> String {
    const LIMIT: usize = 800;
    let mut out = s.trim().to_string();
    if out.len() > LIMIT {
        out.truncate(LIMIT);
        out.push_str("...");
    }
    out
}

fn run_aivi_json(exe: &str, cwd: &Path, args: &[&str]) -> serde_json::Value {
    let output = Command::new(exe)
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|err| panic!("run aivi {args:?}: {err}"));

    if !output.status.success() {
        let stdout = truncate_for_error(&String::from_utf8_lossy(&output.stdout));
        let stderr = truncate_for_error(&String::from_utf8_lossy(&output.stderr));
        panic!("aivi {args:?} failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    }

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf-8");
    assert!(
        !stdout.trim().is_empty(),
        "expected JSON on stdout, got empty output"
    );

    serde_json::from_str(&stdout).unwrap_or_else(|err| {
        let snippet = truncate_for_error(&stdout);
        panic!("stdout is not valid JSON: {err}\nstdout snippet:\n{snippet}")
    })
}

fn json_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[test]
fn kernel_dump_ir_dump_minimal_is_valid_json() {
    let Some(exe) = aivi_exe() else {
        eprintln!("skipping: CARGO_BIN_EXE_aivi not set");
        return;
    };
    let root = workspace_root();

    let value = run_aivi_json(
        &exe,
        &root,
        &["kernel", "integration-tests/syntax/ir_dump_minimal.aivi"],
    );
    assert!(
        value.is_object() || value.is_array(),
        "expected JSON object/array, got {}",
        json_kind(&value)
    );
}

#[test]
fn rust_ir_dump_ir_dump_minimal_is_valid_json() {
    let Some(exe) = aivi_exe() else {
        eprintln!("skipping: CARGO_BIN_EXE_aivi not set");
        return;
    };
    let root = workspace_root();

    let value = run_aivi_json(
        &exe,
        &root,
        &["rust-ir", "integration-tests/syntax/ir_dump_minimal.aivi"],
    );
    assert!(
        value.is_object() || value.is_array(),
        "expected JSON object/array, got {}",
        json_kind(&value)
    );
}
