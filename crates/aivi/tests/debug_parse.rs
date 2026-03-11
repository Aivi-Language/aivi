use aivi::{parse_modules, DiagnosticSeverity};

#[test]
fn parse_modules_handles_sigil_map_inline() {
    let src = r#"
module t
v = ~map{
  "a" => 1
}
"#;
    let (_modules, diags) = parse_modules(std::path::Path::new("<test>"), src);
    assert!(
        diags
            .iter()
            .all(|d| d.diagnostic.severity != DiagnosticSeverity::Error),
        "unexpected parse diagnostics: {diags:#?}"
    );
}
