use aivi::{parse_file, parse_modules, DiagnosticSeverity};
use std::path::PathBuf;

#[test]
#[ignore = "debug-only: prints parse output, no assertions"]
fn debug_file_content() {
    let path = PathBuf::from("../../integration-tests/syntax/sigils/basic.aivi");
    let content = std::fs::read_to_string(&path).expect("read file");
    println!("CONTENT:\n{}", content);
    let file = parse_file(&path).expect("parse");
    for diag in file.diagnostics {
        println!("{}: {}", diag.code, diag.message);
    }
}

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
