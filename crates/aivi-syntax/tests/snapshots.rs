//! Snapshot tests for the syntax CST parsed output.
use aivi_base::SourceDatabase;
use aivi_syntax::parse_module;

fn parse(src: &str) -> aivi_syntax::Module {
    let mut db = SourceDatabase::new();
    let file_id = db.add_file("test.aivi", src);
    let parsed = parse_module(&db[file_id]);
    assert!(
        !parsed.has_errors(),
        "input should parse cleanly: {:?}",
        parsed.all_diagnostics().collect::<Vec<_>>()
    );
    parsed.module
}

#[test]
fn snapshot_value_declaration() {
    let module = parse("value answer = 42");
    insta::assert_debug_snapshot!(module);
}

#[test]
fn snapshot_func_declaration() {
    let module = parse("type Int -> Int -> Int\nfunc add = x y =>\n    x + y");
    insta::assert_debug_snapshot!(module);
}

#[test]
fn snapshot_signal_declaration() {
    let module = parse("signal counter = 0");
    insta::assert_debug_snapshot!(module);
}

#[test]
fn snapshot_type_sum() {
    let module = parse("type Color =\n    | Red\n    | Green\n    | Blue");
    insta::assert_debug_snapshot!(module);
}
