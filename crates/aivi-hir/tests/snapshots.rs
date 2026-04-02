//! Snapshot tests for HIR lowering output.
use aivi_base::SourceDatabase;
use aivi_hir::lower_module;
use aivi_syntax::parse_module;

fn lower(src: &str) -> aivi_hir::Module {
    let mut db = SourceDatabase::new();
    let file_id = db.add_file("test.aivi", src);
    let parsed = parse_module(&db[file_id]);
    assert!(
        !parsed.has_errors(),
        "input should parse cleanly: {:?}",
        parsed.all_diagnostics().collect::<Vec<_>>()
    );
    let lowered = lower_module(&parsed.module);
    assert!(
        !lowered.has_errors(),
        "input should lower to HIR cleanly: {:?}",
        lowered.diagnostics()
    );
    lowered.into_parts().0
}

#[test]
fn snapshot_value_hir() {
    let module = lower("value answer = 42");
    insta::assert_debug_snapshot!(module);
}

#[test]
fn snapshot_signal_hir() {
    let module = lower("signal counter = 0");
    insta::assert_debug_snapshot!(module);
}
