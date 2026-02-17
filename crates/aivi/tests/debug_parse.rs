use aivi::parse_file;
use std::path::PathBuf;

#[test]
fn debug_file_content() {
    let path = PathBuf::from("../../integration-tests/syntax/sigils/basic.aivi");
    let content = std::fs::read_to_string(&path).expect("read file");
    println!("CONTENT:\n{}", content);
    let file = parse_file(&path).expect("parse");
    for diag in file.diagnostics {
        println!("{}: {}", diag.code, diag.message);
    }
}
