use aivi::{edit_cargo_toml_dependencies, CargoDepSpec};

#[test]
fn install_adds_or_updates_dependency() {
    let original = r#"
[package]
name = "demo"
version = "0.1.0"
edition = "2024"

[dependencies]
serde = "1"
"#;

    let dep = CargoDepSpec::parse("foo@^1.2").expect("parse spec");
    let edits = edit_cargo_toml_dependencies(original, &dep).expect("edit manifest");
    assert!(edits.changed);
    assert!(edits.updated_manifest.contains("foo = \"^1.2\""));

    let dep = CargoDepSpec::parse("serde@1.0").expect("parse spec");
    let edits = edit_cargo_toml_dependencies(&edits.updated_manifest, &dep).expect("edit manifest");
    assert!(edits.updated_manifest.contains("serde = \"1.0\""));
}
