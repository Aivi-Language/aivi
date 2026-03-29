use std::{fs, path::PathBuf, process::Command};

fn stdlib_path(relative: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("stdlib")
        .join(relative);
    fs::canonicalize(&path).unwrap_or(path)
}

#[test]
fn test_command_accepts_stdlib_validation_files() {
    for (relative, summary) in [
        (
            "aivi/text.aivi",
            "test result: ok. 3 passed; 0 failed; 3 total",
        ),
        (
            "aivi/math.aivi",
            "test result: ok. 2 passed; 0 failed; 2 total",
        ),
        (
            "aivi/bool.aivi",
            "test result: ok. 2 passed; 0 failed; 2 total",
        ),
        (
            "aivi/defaults.aivi",
            "test result: ok. 3 passed; 0 failed; 3 total",
        ),
        (
            "aivi/core/float.aivi",
            "test result: ok. 2 passed; 0 failed; 2 total",
        ),
        (
            "tests/runtime-stdlib-validation/main.aivi",
            "test result: ok. 3 passed; 0 failed; 3 total",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
            .arg("test")
            .arg(stdlib_path(relative))
            .output()
            .expect("test command should run");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "expected {relative} to pass `aivi test`, stdout was: {stdout}, stderr was: {stderr}"
        );
        assert!(
            stderr.is_empty(),
            "expected {relative} to keep stderr empty, stderr was: {stderr}"
        );
        assert!(
            stdout.contains(summary),
            "expected success summary for {relative}, stdout was: {stdout}"
        );
    }
}

#[test]
fn test_command_reports_when_workspace_has_no_tests() {
    let output = Command::new(env!("CARGO_BIN_EXE_aivi"))
        .arg("test")
        .arg(stdlib_path("aivi/order.aivi"))
        .output()
        .expect("test command should run");

    assert!(
        !output.status.success(),
        "expected `aivi test` to fail when no `@test` values exist"
    );
    assert!(String::from_utf8_lossy(&output.stdout).is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("no `@test` values found in the loaded workspace"),
        "expected missing-test diagnostic, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
