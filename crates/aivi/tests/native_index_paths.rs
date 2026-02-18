use std::path::PathBuf;
use std::process::Command;

use aivi::{compile_rust_native, desugar_target};
use tempfile::tempdir;

#[test]
fn native_codegen_supports_map_index_and_patch_selectors() {
    let dir = tempdir().expect("tempdir");
    let source_path = dir.path().join("main.aivi");
    std::fs::write(
        &source_path,
        r#"module app.main
main : Effect Text Unit
main = do Effect {
  m = ~map{ "a" => 1, "b" => 2 }
  _ <- println (m["a"])

  m2 = m <| { ["a"]: _ + 10 }
  _ <- println (m2["a"])

  m3 = m <| { [key == "b"]: _ + 100 }
  _ <- println (m3["b"])

  xs = [{ n: 1 }, { n: 2 }]
  ys = xs <| { [*].n: _ + 1 }
  _ <- println (ys[0].n)
  _ <- println (ys[1].n)

  xs2 = [{ active: True, n: 10 }, { active: False, n: 20 }]
  ys2 = xs2 <| { [active].n: _ + 1 }
  _ <- println (ys2[0].n)
  _ <- println (ys2[1].n)

  xs3 = xs <| { [n > 1].n: _ + 10 }
  _ <- println (xs3[0].n)
  _ <- println (xs3[1].n)

  pure Unit
}
"#,
    )
    .expect("write aivi source");

    let source_path_str = source_path.to_string_lossy().to_string();
    let program = desugar_target(&source_path_str).expect("desugar");
    let rust = compile_rust_native(program).expect("compile_rust_native");

    let cargo_toml = format!(
        "[package]\nname = \"aivi-native-index-paths\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\naivi_native_runtime = {{ path = {:?} }}\n",
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
    let want = ["1", "11", "102", "2", "3", "11", "20", "1", "12"];
    for line in want {
        assert!(
            stdout.lines().any(|l| l.trim() == line),
            "stdout missing line {line:?}\nstdout:\n{stdout}"
        );
    }
}
