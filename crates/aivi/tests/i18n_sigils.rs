use std::path::Path;

use aivi::{generate_i18n_module_from_properties, parse_file, parse_modules};

#[test]
fn i18n_sigil_key_is_validated_at_parse_time() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("bad_key.aivi");
    std::fs::write(
        &path,
        r#"
module test.badKey
use aivi
x = ~k"..bad"
"#,
    )
    .expect("write file");

    let file = parse_file(&path).expect("parse file");
    assert!(
        file.diagnostics.iter().any(|d| d.code == "E1514"),
        "expected E1514 diagnostic, got: {:?}",
        file.diagnostics
            .iter()
            .map(|d| (&d.code, &d.message))
            .collect::<Vec<_>>()
    );
}

#[test]
fn i18n_sigil_message_is_validated_at_parse_time() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("bad_msg.aivi");
    std::fs::write(
        &path,
        r#"
module test.badMsg
use aivi
x = ~m"Hello {name"
"#,
    )
    .expect("write file");

    let file = parse_file(&path).expect("parse file");
    assert!(
        file.diagnostics.iter().any(|d| d.code == "E1515"),
        "expected E1515 diagnostic, got: {:?}",
        file.diagnostics
            .iter()
            .map(|d| (&d.code, &d.message))
            .collect::<Vec<_>>()
    );
}

#[test]
fn i18n_codegen_module_parses() {
    let source = generate_i18n_module_from_properties(
        "app.i18n.en_US",
        "en-US",
        "app.welcome = Hello, {name:Text}!\n",
    )
    .expect("generate module");

    let dummy = Path::new("generated_i18n.aivi");
    let (_modules, diags) = parse_modules(dummy, &source);
    assert!(
        diags.is_empty(),
        "expected generated module to parse without diagnostics, got: {:?}",
        diags
            .iter()
            .map(|d| (&d.diagnostic.code, &d.diagnostic.message))
            .collect::<Vec<_>>()
    );
}
