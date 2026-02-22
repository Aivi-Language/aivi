use aivi::{write_scaffold, ProjectKind};

#[test]
fn init_bin_writes_expected_files() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("my-app");
    write_scaffold(&dir, "my-app", ProjectKind::Bin, "2024", "0.1", false).expect("scaffold");

    assert!(dir.join("aivi.toml").exists());
    assert!(dir.join("Cargo.toml").exists());
    assert!(dir.join(".gitignore").exists());
    assert!(dir.join("src").join("main.aivi").exists());
    assert!(
        !dir.join("target").exists(),
        "scaffold must not create build output"
    );

    let cargo = std::fs::read_to_string(dir.join("Cargo.toml")).expect("read Cargo.toml");
    let aivi_toml = std::fs::read_to_string(dir.join("aivi.toml")).expect("read aivi.toml");
    assert!(cargo.contains("path = \"target/aivi-gen/src/main.rs\""));
    assert!(cargo.contains("aivi_native_runtime = { path ="));
    assert!(cargo.contains("runtime-gnome = [\"aivi_native_runtime/gtk4-libadwaita\"]"));
    assert!(aivi_toml.contains("native_ui_target = \"portable\""));
}
