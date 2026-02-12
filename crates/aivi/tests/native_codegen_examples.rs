use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use aivi::{compile_rust_native, desugar_target};
use aivi_native_runtime::get_builtin;
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
    let examples_dir = root.join("examples");
    assert!(examples_dir.exists(), "missing examples/ directory");

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

        let rust = match compile_rust_native(program) {
            Ok(rust) => rust,
            Err(err) => {
                failures.push(format!("{rel_str}: native codegen failed: {err}"));
                continue;
            }
        };

        for builtin in extract_builtin_names(&rust) {
            if get_builtin(&builtin).is_none() {
                failures.push(format!("{rel_str}: missing builtin {builtin:?} in aivi_native_runtime"));
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
#[ignore = "native codegen is experimental; run with `cargo test -p aivi --test native_codegen_examples -- --ignored`"]
fn native_codegen_examples_compile_with_rustc() {
    let root = set_workspace_root();
    let examples_dir = root.join("examples");
    assert!(examples_dir.exists(), "missing examples/ directory");

    let mut failures = Vec::new();
    let mut compiled = 0usize;

    let dir = tempdir().expect("tempdir");
    let cargo_toml = format!(
        "[package]\nname = \"aivi-native-examples\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\naivi_native_runtime = {{ path = {:?} }}\n",
        root.join("crates/aivi_native_runtime")
            .display()
            .to_string()
    );
    if let Err(err) = std::fs::write(dir.path().join("Cargo.toml"), cargo_toml) {
        panic!("write Cargo.toml failed: {err}");
    }

    let bin_dir = dir.path().join("src").join("bin");
    if let Err(err) = std::fs::create_dir_all(&bin_dir) {
        panic!("create src/bin dir failed: {err}");
    }

    for entry in WalkDir::new(&examples_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| is_aivi_source(e.path()))
    {
        let path = entry.path();
        let rel = path.strip_prefix(&root).unwrap_or(path);
        let rel_str = rel.to_string_lossy();
        let bin_name = rel_str
            .replace(std::path::MAIN_SEPARATOR, "_")
            .replace('/', "_")
            .replace('\\', "_")
            .replace('.', "_");

        eprintln!("[native_codegen] compile {rel_str}");
        let t0 = Instant::now();

        let program = match desugar_target(&rel_str) {
            Ok(p) => p,
            Err(err) => {
                failures.push(format!("{rel_str}: desugar failed: {err}"));
                continue;
            }
        };

        let rust = match compile_rust_native(program) {
            Ok(rust) => rust,
            Err(err) => {
                failures.push(format!("{rel_str}: native codegen failed: {err}"));
                continue;
            }
        };

        if let Err(err) = std::fs::write(bin_dir.join(format!("{bin_name}.rs")), rust) {
            failures.push(format!("{rel_str}: write src/bin/{bin_name}.rs failed: {err}"));
            continue;
        }

        compiled += 1;
        eprintln!(
            "[native_codegen] ok {rel_str} ({:?})",
            Instant::now().duration_since(t0)
        );
    }

    let output = match Command::new("cargo")
        .arg("build")
        .arg("--quiet")
        .arg("--offline")
        .current_dir(dir.path())
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            panic!("cargo spawn failed: {err}");
        }
    };

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
