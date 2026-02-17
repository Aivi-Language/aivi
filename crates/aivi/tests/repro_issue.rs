use std::path::Path;
use aivi::{parse_modules, check_modules, check_types, file_diagnostics_have_errors};

#[test]
fn repro_type_mismatch_order() {
    let source = r#"
module test.repro
export main

main : Int
main = 1.0
"#;
    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(!file_diagnostics_have_errors(&diagnostics), "parse errors: {:?}", diagnostics);

    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));

    for diag in &module_diags {
        println!("[DEBUG_LOG] Diagnostic: {}", diag.diagnostic.message);
    }

    let mismatch_diag = module_diags.iter().find(|d| d.diagnostic.message.contains("type mismatch")).expect("expected type mismatch diagnostic");

    // The issue is that it currently says "(expected Float, found Int)" but it should be "(expected Int, found Float)"
    // If it currently says "(expected Float, found Int)", then it's wrong.
    assert!(mismatch_diag.diagnostic.message.contains("expected Int, found Float"), "Message was: {}", mismatch_diag.diagnostic.message);
}
