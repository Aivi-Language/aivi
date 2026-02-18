use std::path::PathBuf;
use std::process::Command;

use aivi::{compile_rust_native, desugar_target};
use tempfile::tempdir;

#[test]
fn native_codegen_smoke_compiles_and_runs() {
    let dir = tempdir().expect("tempdir");
    let source_path = dir.path().join("main.aivi");
    std::fs::write(
        &source_path,
        r#"module app.main
main : Effect Text Unit
main = do Effect {
  print "Hello from AIVI!"
}
"#,
    )
    .expect("write aivi source");

    let source_path_str = source_path.to_string_lossy().to_string();
    let program = desugar_target(&source_path_str).expect("desugar");
    let rust = compile_rust_native(program).expect("compile_rust_native");
    assert!(rust.contains("fn main()"));
    assert!(!rust.contains("PROGRAM_JSON"));

    let cargo_toml = format!(
        "[package]\nname = \"aivi-native-smoke\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\naivi_native_runtime = {{ path = {:?} }}\n",
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../aivi_native_runtime")
            .display()
            .to_string()
    );
    std::fs::write(dir.path().join("Cargo.toml"), cargo_toml).expect("write Cargo.toml");
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("create src dir");
    std::fs::write(src_dir.join("main.rs"), rust).expect("write main.rs");

    let output = Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("--offline")
        .env("RUSTFLAGS", "-Awarnings")
        .current_dir(dir.path())
        .output()
        .expect("cargo run");
    assert!(
        output.status.success(),
        "cargo run failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Hello from AIVI!"),
        "unexpected stdout: {stdout}"
    );
}
