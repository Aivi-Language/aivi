use std::path::PathBuf;
use std::process::Command;

#[test]
fn examples_build() {
    let Ok(exe) = std::env::var("CARGO_BIN_EXE_aivi") else {
        eprintln!("skipping: CARGO_BIN_EXE_aivi not set");
        return;
    };
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root");

    let output = Command::new(exe)
        .arg("build")
        .arg("integration-tests")
        .current_dir(workspace_root)
        .output()
        .expect("run aivi build integration-tests");

    if output.status.success() {
        return;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    panic!("aivi build integration-tests failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
}
