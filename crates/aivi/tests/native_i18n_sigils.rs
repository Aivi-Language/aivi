mod native_fixture;

use aivi::{compile_rust_native, desugar_target};
use native_fixture::{
    assert_cargo_success, cargo_run_fixture, stdout_text, write_aivi_source, FIXTURE_LOCK,
};
use tempfile::tempdir;

#[test]
fn native_codegen_compiles_i18n_sigils_with_parts() {
    let dir = tempdir().expect("tempdir");
    let source_path_str = write_aivi_source(
        dir.path(),
        "main.aivi",
        r#"module app.main
main : Effect Text Unit
main = do Effect {
  k = ~k"app.welcome"
  msg = ~m"Hello, {name:Text}!"
  rendered = i18n.render msg { name: "Alice" } or "ERR"
  _ <- println rendered
  _ <- println (k.body)
  pure Unit
}
"#,
    );

    let program = desugar_target(&source_path_str).expect("desugar");
    let rust = compile_rust_native(program).expect("compile_rust_native");

    let _lock = FIXTURE_LOCK.lock().unwrap();
    let output = cargo_run_fixture(&rust);
    assert_cargo_success(&output);

    let stdout = stdout_text(&output);
    for want in ["Hello, Alice!", "app.welcome"] {
        assert!(
            stdout.lines().any(|l| l.trim() == want),
            "stdout missing line {want:?}\nstdout:\n{stdout}"
        );
    }
}
