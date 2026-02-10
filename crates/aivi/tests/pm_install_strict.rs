use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, contents).expect("write file");
}

#[test]
fn install_rejects_non_aivi_dependency_and_rolls_back() {
    let temp = TempDir::new().expect("tempdir");
    let root = temp.path().join("app");
    let dep = temp.path().join("dep");

    write_file(
        &root.join("aivi.toml"),
        r#"[project]
kind = "bin"
entry = "main.aivi"
"#,
    );
    let cargo_toml = r#"[package]
name = "demo"
version = "0.1.0"
edition = "2024"

[dependencies]
"#;
    write_file(&root.join("Cargo.toml"), cargo_toml);
    write_file(&root.join("src/lib.rs"), "");

    write_file(
        &dep.join("Cargo.toml"),
        r#"[package]
name = "dep"
version = "0.1.0"
edition = "2024"
"#,
    );
    write_file(&dep.join("src/lib.rs"), "");

    let Ok(exe) = std::env::var("CARGO_BIN_EXE_aivi") else {
        eprintln!("skipping: CARGO_BIN_EXE_aivi not set");
        return;
    };
    let spec = format!("path:{}", dep.display());
    let output = Command::new(exe)
        .arg("install")
        .arg(spec)
        .arg("--no-fetch")
        .current_dir(&root)
        .output()
        .expect("run aivi install");

    assert!(!output.status.success());
    let updated = fs::read_to_string(root.join("Cargo.toml")).expect("read Cargo.toml");
    assert_eq!(updated, cargo_toml);
    assert!(!root.join("Cargo.lock").exists());
}
