use std::fs;
use std::path::Path;
use std::process::Command;

fn aivi_exe() -> Option<String> {
    std::env::var("CARGO_BIN_EXE_aivi").ok()
}

fn run_aivi(exe: &str, cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(exe)
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|err| panic!("run aivi {args:?}: {err}"))
}

fn panic_with_output(args: &[&str], output: &std::process::Output) -> ! {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    panic!("aivi {args:?} failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

#[test]
fn aivi_test_resolves_decorated_import_dependencies() {
    let Some(exe) = aivi_exe() else {
        eprintln!("skipping: CARGO_BIN_EXE_aivi not set");
        return;
    };

    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(
        dir.path().join("helper.aivi"),
        r#"@no_prelude
module demo.helper

use aivi

value = 1
"#,
    )
    .expect("write helper");
    fs::write(
        dir.path().join("main.aivi"),
        r#"@no_prelude
module demo.main

use aivi
use aivi.testing
use demo.helper (value)

@test "imports decorated helper"
importsDecoratedHelper = assertEq value 1
"#,
    )
    .expect("write main");

    let target = format!("{}/**", dir.path().display());
    let args = ["test", target.as_str()];
    let output = run_aivi(&exe, dir.path(), &args);
    if !output.status.success() {
        panic_with_output(&args, &output);
    }
}

#[test]
fn aivi_test_project_mode_sets_snapshot_root() {
    let Some(exe) = aivi_exe() else {
        eprintln!("skipping: CARGO_BIN_EXE_aivi not set");
        return;
    };

    let dir = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(dir.path().join("src")).expect("create src");
    fs::write(
        dir.path().join("aivi.toml"),
        r#"[project]
kind = "bin"
entry = "main.aivi"
language_version = "0.1"
"#,
    )
    .expect("write aivi.toml");
    fs::write(
        dir.path().join("src/main.aivi"),
        r#"@no_prelude
module demo.main

use aivi
use aivi.testing

@test "snapshot project mode"
snapshotProjectMode = assertSnapshot "record_test" { name: "Ada", age: 42 }
"#,
    )
    .expect("write main.aivi");

    let args = ["test", "--update-snapshots"];
    let output = run_aivi(&exe, dir.path(), &args);
    if !output.status.success() {
        panic_with_output(&args, &output);
    }

    let snapshot = dir
        .path()
        .join("__snapshots__/demo.main/snapshotProjectMode/record_test.snap");
    assert!(
        snapshot.exists(),
        "expected snapshot file at {}",
        snapshot.display()
    );
    let contents = fs::read_to_string(&snapshot).expect("read snapshot");
    assert!(
        contents.contains("Ada"),
        "snapshot should contain serialized data"
    );
}
