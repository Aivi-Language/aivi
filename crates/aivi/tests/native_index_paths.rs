mod native_fixture;

use aivi::{compile_rust_native, desugar_target};
use native_fixture::{
    assert_cargo_success, cargo_run_fixture, stdout_text, write_aivi_source, FIXTURE_LOCK,
};
use tempfile::tempdir;

#[test]
fn native_codegen_supports_map_index_and_patch_selectors() {
    let dir = tempdir().expect("tempdir");
    let source_path_str = write_aivi_source(
        dir.path(),
        "main.aivi",
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
    );

    let program = desugar_target(&source_path_str).expect("desugar");
    let rust = compile_rust_native(program).expect("compile_rust_native");

    let _lock = FIXTURE_LOCK.lock().unwrap();
    let output = cargo_run_fixture(&rust);
    assert_cargo_success(&output);

    let stdout = stdout_text(&output);
    let want = ["1", "11", "102", "2", "3", "11", "20", "1", "12"];
    for line in want {
        assert!(
            stdout.lines().any(|l| l.trim() == line),
            "stdout missing line {line:?}\nstdout:\n{stdout}"
        );
    }
}
