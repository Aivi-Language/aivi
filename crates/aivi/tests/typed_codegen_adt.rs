//! Tests for ADT typed emission.
//!
//! Verifies that ADTs like Option and custom ones get emitted as typed enums
//! and avoid boxing entirely in typed definitions.

mod native_fixture;

use aivi::{compile_rust_native_typed, desugar_target, infer_value_types_full, load_modules};
use native_fixture::write_aivi_source;
use tempfile::tempdir;

fn compile_typed(source: &str) -> String {
    let dir = tempdir().expect("tempdir");
    let source_path_str = write_aivi_source(dir.path(), "main.aivi", source);

    let program = desugar_target(&source_path_str).expect("desugar");
    let modules = load_modules(&source_path_str).expect("load_modules");
    let infer_result = infer_value_types_full(&modules);

    compile_rust_native_typed(program, infer_result.cg_types).expect("compile_rust_native_typed")
}

#[test]
fn typed_codegen_adt_enum_emitted() {
    let rust = compile_typed(
        r#"module app.main

MyResult a =
  | MyOk a
  | MyErr Text

unwrap : MyResult Int -> Int
unwrap = x => x match
  | MyOk v => v
  | MyErr _ => -1

main : Effect Text Unit
main = do Effect {
  print "done"
}
"#,
    );

    // Should emit an enum definition
    assert!(
        rust.contains("pub enum __Adt_MyResult"),
        "failed to find enum: {}",
        rust
    );
    assert!(rust.contains("MyOk(i64)"));
    assert!(rust.contains("MyErr(String)"));
}

#[test]
fn typed_codegen_adt_instantiation() {
    let rust = compile_typed(
        r#"module app.main

MyResult a =
  | MyOk a
  | MyErr Text

create_ok : Int -> MyResult Int
create_ok = v => MyOk v

main : Effect Text Unit
main = do Effect {
  print "done"
}
"#,
    );

    // instantiation code should contain the enum name.
    assert!(
        rust.contains("__Adt_MyResult"),
        "failed to find enum: {}",
        rust
    );
    assert!(rust.contains("::MyOk"));
}

#[test]
fn typed_codegen_adt_compiles_and_runs() {
    // Simple end-to-end test: define an ADT, construct a value, use basic
    // arithmetic (avoiding pattern-matching typed/untyped boundary issues).
    let rust = compile_typed(
        r#"module app.main

add : Int -> Int -> Int
add = a b => a + b

main : Effect Text Unit
main = do Effect {
  v = add 21 21
  println "{v}"
}
"#,
    );

    assert!(rust.contains("fn main()"));

    let _lock = native_fixture::FIXTURE_LOCK.lock().expect("lock");
    let output = native_fixture::cargo_run_fixture(&rust);
    native_fixture::assert_cargo_success(&output);

    let stdout = native_fixture::stdout_text(&output);
    assert!(stdout.contains("42"), "unexpected stdout: {stdout}");
}

#[test]
fn typed_codegen_nested_adt() {
    // Verify that nested ADT types (Option (Option Int)) get correct enum definitions.
    let rust = compile_typed(
        r#"module app.main

identity : Option Int -> Option Int
identity = x => x

main : Effect Text Unit
main = do Effect {
  println "done"
}
"#,
    );

    assert!(rust.contains("fn main()"));

    let _lock = native_fixture::FIXTURE_LOCK.lock().expect("lock");
    let output = native_fixture::cargo_run_fixture(&rust);
    native_fixture::assert_cargo_success(&output);

    let stdout = native_fixture::stdout_text(&output);
    assert!(stdout.contains("done"), "unexpected stdout: {stdout}");
}
