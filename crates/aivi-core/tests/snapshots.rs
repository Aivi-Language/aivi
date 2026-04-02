//! Snapshot tests for typed-core IR lowering output.
use aivi_base::SourceDatabase;
use aivi_core::{lower_module, validate_module};
use aivi_syntax::parse_module;

fn lower_to_core(src: &str) -> aivi_core::Module {
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
    let core = lower_module(hir.module()).expect("HIR should lower into typed core");
    validate_module(&core).expect("typed core should validate");
    core
}

#[test]
fn snapshot_core_value_module() {
    let core = lower_to_core("value answer = 42");
    insta::assert_snapshot!(core.pretty());
}

#[test]
fn snapshot_core_func_module() {
    let core = lower_to_core("type Int -> Int -> Int\nfunc add = x y =>\n    x + y");
    insta::assert_snapshot!(core.pretty());
}
