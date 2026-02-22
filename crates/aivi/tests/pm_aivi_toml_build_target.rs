use std::fs;

use aivi::{read_aivi_toml, NativeUiTarget};

#[test]
fn aivi_toml_defaults_to_portable_native_ui_target() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("aivi.toml");
    fs::write(
        &path,
        r#"[project]
kind = "bin"
entry = "main.aivi"
language_version = "0.1"
"#,
    )
    .expect("write aivi.toml");
    let cfg = read_aivi_toml(&path).expect("parse aivi.toml");
    assert_eq!(cfg.build.native_ui_target, NativeUiTarget::Portable);
}

#[test]
fn aivi_toml_parses_gnome_native_ui_target() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("aivi.toml");
    fs::write(
        &path,
        r#"[project]
kind = "bin"
entry = "main.aivi"
language_version = "0.1"

[build]
native_ui_target = "gnome-gtk4-libadwaita"
"#,
    )
    .expect("write aivi.toml");
    let cfg = read_aivi_toml(&path).expect("parse aivi.toml");
    assert_eq!(
        cfg.build.native_ui_target,
        NativeUiTarget::GnomeGtk4Libadwaita
    );
}
