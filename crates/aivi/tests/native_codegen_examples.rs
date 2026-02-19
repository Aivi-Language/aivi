mod native_fixture;

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use aivi::{compile_rust_native_lib, desugar_target};
use aivi_native_runtime::get_builtin;
use native_fixture::fixture_dir;
use tempfile::tempdir;
use walkdir::WalkDir;

fn set_workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root");
    std::env::set_current_dir(workspace_root).expect("set cwd");
    workspace_root.to_path_buf()
}

fn is_aivi_source(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|ext| ext == "aivi")
}

fn sanitize_rust_mod_name(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        let ok = ch == '_' || ch.is_ascii_alphanumeric();
        if ok {
            if i == 0 && ch.is_ascii_digit() {
                out.push('_');
            }
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "_".to_string()
    } else {
        out
    }
}

fn extract_builtin_names(rust: &str) -> Vec<String> {
    // The native backend emits builtins as `__builtin("name")`.
    let mut out = Vec::new();
    let mut i = 0usize;
    while let Some(start) = rust[i..].find("__builtin(\"") {
        let start = i + start + "__builtin(\"".len();
        if let Some(end) = rust[start..].find("\")") {
            out.push(rust[start..start + end].to_string());
            i = start + end + 2;
        } else {
            break;
        }
    }
    out.sort();
    out.dedup();
    out
}

#[test]
fn native_codegen_examples_emit_rust_and_check_builtins() {
    let root = set_workspace_root();
    let examples_dir = root.join("integration-tests");
    assert!(
        examples_dir.exists(),
        "missing integration-tests/ directory"
    );

    let mut failures = Vec::new();
    let mut compiled = 0usize;

    for entry in WalkDir::new(&examples_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| is_aivi_source(e.path()))
    {
        let path = entry.path();
        let rel = path.strip_prefix(&root).unwrap_or(path);
        let rel_str = rel.to_string_lossy();

        eprintln!("[native_codegen] emit {rel_str}");
        let t0 = Instant::now();

        let program = match desugar_target(&rel_str) {
            Ok(p) => p,
            Err(err) => {
                failures.push(format!("{rel_str}: desugar failed: {err}"));
                continue;
            }
        };

        let rust = match compile_rust_native_lib(program) {
            Ok(rust) => rust,
            Err(err) => {
                failures.push(format!("{rel_str}: native codegen failed: {err}"));
                continue;
            }
        };

        for builtin in extract_builtin_names(&rust) {
            if get_builtin(&builtin).is_none() {
                failures.push(format!(
                    "{rel_str}: missing builtin {builtin:?} in aivi_native_runtime"
                ));
            }
        }

        compiled += 1;
        eprintln!(
            "[native_codegen] ok {rel_str} ({:?})",
            Instant::now().duration_since(t0)
        );
    }

    if !failures.is_empty() {
        failures.sort();
        panic!(
            "native codegen failed for {}/{} example(s):\n{}",
            failures.len(),
            failures.len() + compiled,
            failures.join("\n\n")
        );
    }
}

#[test]
fn native_codegen_examples_compile_with_rustc() {
    let root = set_workspace_root();
    let examples_dir = root.join("integration-tests");
    assert!(
        examples_dir.exists(),
        "missing integration-tests/ directory"
    );

    let mut failures = Vec::new();
    let mut compiled = 0usize;

    let dir = tempdir().expect("tempdir");
    let cargo_toml = format!(
        "[package]\nname = \"aivi-native-examples\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n\n[dependencies]\naivi_native_runtime = {{ path = {:?} }}\n",
        root.join("crates/aivi_native_runtime")
            .display()
            .to_string()
    );
    std::fs::write(dir.path().join("Cargo.toml"), cargo_toml).expect("write Cargo.toml");

    // Copy mold linker config from fixture crate
    let cargo_config_dir = dir.path().join(".cargo");
    std::fs::create_dir_all(&cargo_config_dir).expect("create .cargo dir");
    let fixture_config = fixture_dir().join(".cargo/config.toml");
    if fixture_config.exists() {
        std::fs::copy(&fixture_config, cargo_config_dir.join("config.toml"))
            .expect("copy .cargo/config.toml");
    }

    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("create src dir");

    let mut lib_rs = String::new();

    for entry in WalkDir::new(&examples_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| is_aivi_source(e.path()))
    {
        let path = entry.path();
        let rel = path.strip_prefix(&root).unwrap_or(path);
        let rel_str = rel.to_string_lossy();
        let mod_name = sanitize_rust_mod_name(&rel_str);

        eprintln!("[native_codegen] compile {rel_str}");
        let t0 = Instant::now();

        let program = match desugar_target(&rel_str) {
            Ok(p) => p,
            Err(err) => {
                failures.push(format!("{rel_str}: desugar failed: {err}"));
                continue;
            }
        };

        let rust = match compile_rust_native_lib(program) {
            Ok(rust) => rust,
            Err(err) => {
                failures.push(format!("{rel_str}: native codegen failed: {err}"));
                continue;
            }
        };

        lib_rs.push_str(&format!("pub mod {mod_name} {{\n"));
        for line in rust.lines() {
            lib_rs.push_str("    ");
            lib_rs.push_str(line);
            lib_rs.push('\n');
        }
        lib_rs.push_str("}\n\n");

        compiled += 1;
        eprintln!(
            "[native_codegen] ok {rel_str} ({:?})",
            Instant::now().duration_since(t0)
        );
    }

    std::fs::write(src_dir.join("lib.rs"), lib_rs).expect("write src/lib.rs");

    let output = Command::new("cargo")
        .arg("build")
        .arg("--lib")
        .arg("--quiet")
        .env("RUSTFLAGS", "-Awarnings")
        .current_dir(dir.path())
        .output()
        .expect("cargo build");

    if !output.status.success() {
        failures.push(format!(
            "cargo build failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        ));
    }

    if !failures.is_empty() {
        failures.sort();
        panic!(
            "native codegen failed for {}/{} example(s):\n{}",
            failures.len(),
            failures.len() + compiled,
            failures.join("\n\n")
        );
    }
}
