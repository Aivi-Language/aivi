//! Snapshot tests for lambda IR lowering output.
use aivi_base::SourceDatabase;
use aivi_core::{lower_module as lower_core_module, validate_module as validate_core_module};
use aivi_lambda::{lower_module, validate_module};
use aivi_syntax::parse_module;

fn lower_to_lambda(src: &str) -> aivi_lambda::Module {
    let mut db = SourceDatabase::new();
    let file_id = db.add_file("test.aivi", src);
    let parsed = parse_module(&db[file_id]);
    assert!(
        !parsed.has_errors(),
        "input should parse cleanly: {:?}",
        parsed.all_diagnostics().collect::<Vec<_>>()
    );
    let hir = aivi_hir::lower_module(&parsed.module);
    assert!(
        !hir.has_errors(),
        "input should lower to HIR: {:?}",
        hir.diagnostics()
    );
    let core = lower_core_module(hir.module()).expect("HIR should lower into typed core");
    validate_core_module(&core).expect("typed core should validate");
    let lambda = lower_module(&core).expect("lambda lowering should succeed");
    validate_module(&lambda).expect("lambda module should validate");
    lambda
}

#[test]
fn snapshot_lambda_value_module() {
    let lambda = lower_to_lambda("value answer = 42");
    insta::assert_snapshot!(lambda.pretty());
}

#[test]
fn snapshot_lambda_func_module() {
    let lambda = lower_to_lambda("type Int -> Int -> Int\nfunc add = x y =>\n    x + y");
    insta::assert_snapshot!(lambda.pretty());
}
