use std::fs;
use std::path::Path;

use aivi::{read_aivi_toml, validate_publish_preflight};

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, contents).expect("write file");
}

#[test]
fn publish_preflight_accepts_consistent_manifests() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    write_file(
        &root.join("aivi.toml"),
        r#"[project]
kind = "bin"
entry = "main.aivi"
language_version = "0.1"
"#,
    );
    write_file(
        &root.join("Cargo.toml"),
        r#"[package]
name = "demo"
version = "0.1.0"
edition = "2024"

[package.metadata.aivi]
language_version = "0.1"
kind = "bin"
entry = "src/main.aivi"

[dependencies]
"#,
    );

    let cfg = read_aivi_toml(&root.join("aivi.toml")).expect("read aivi.toml");
    validate_publish_preflight(root, &cfg).expect("preflight ok");
}

#[test]
fn publish_preflight_rejects_kind_mismatch() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    write_file(
        &root.join("aivi.toml"),
        r#"[project]
kind = "lib"
entry = "lib.aivi"
language_version = "0.1"
"#,
    );
    write_file(
        &root.join("Cargo.toml"),
        r#"[package]
name = "demo"
version = "0.1.0"
edition = "2024"

[package.metadata.aivi]
language_version = "0.1"
kind = "bin"
entry = "src/lib.aivi"
"#,
    );

    let cfg = read_aivi_toml(&root.join("aivi.toml")).expect("read aivi.toml");
    let err = validate_publish_preflight(root, &cfg).unwrap_err();
    assert!(err.to_string().contains("kind"));
}

#[test]
fn publish_preflight_rejects_language_version_mismatch() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    write_file(
        &root.join("aivi.toml"),
        r#"[project]
kind = "bin"
entry = "main.aivi"
language_version = "0.2"
"#,
    );
    write_file(
        &root.join("Cargo.toml"),
        r#"[package]
name = "demo"
version = "0.1.0"
edition = "2024"

[package.metadata.aivi]
language_version = "0.1"
kind = "bin"
entry = "src/main.aivi"
"#,
    );

    let cfg = read_aivi_toml(&root.join("aivi.toml")).expect("read aivi.toml");
    let err = validate_publish_preflight(root, &cfg).unwrap_err();
    assert!(err.to_string().contains("language_version"));
}

#[test]
fn publish_preflight_rejects_missing_package_metadata_aivi() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    write_file(
        &root.join("aivi.toml"),
        r#"[project]
kind = "bin"
entry = "main.aivi"
language_version = "0.1"
"#,
    );
    write_file(
        &root.join("Cargo.toml"),
        r#"[package]
name = "demo"
version = "0.1.0"
edition = "2024"
"#,
    );

    let cfg = read_aivi_toml(&root.join("aivi.toml")).expect("read aivi.toml");
    let err = validate_publish_preflight(root, &cfg).unwrap_err();
    assert!(err.to_string().contains("missing [package.metadata.aivi]"));
}
