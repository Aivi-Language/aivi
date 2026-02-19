mod native_fixture;

use aivi::{compile_rust_native, desugar_target};
use native_fixture::{
    assert_cargo_success, cargo_run_fixture, stdout_text, write_aivi_source, FIXTURE_LOCK,
};
use tempfile::tempdir;

#[test]
fn native_codegen_smoke_compiles_and_runs() {
    let dir = tempdir().expect("tempdir");
    let source_path_str = write_aivi_source(
        dir.path(),
        "main.aivi",
        r#"module app.main
main : Effect Text Unit
main = do Effect {
  print "Hello from AIVI!"
}
"#,
    );

    let program = desugar_target(&source_path_str).expect("desugar");
    let rust = compile_rust_native(program).expect("compile_rust_native");
    assert!(rust.contains("fn main()"));
    assert!(!rust.contains("PROGRAM_JSON"));

    let _lock = FIXTURE_LOCK.lock().unwrap();
    let output = cargo_run_fixture(&rust);
    assert_cargo_success(&output);

    let stdout = stdout_text(&output);
    assert!(
        stdout.contains("Hello from AIVI!"),
        "unexpected stdout: {stdout}"
    );
}
