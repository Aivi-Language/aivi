mod native_fixture;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use aivi::{compile_rust_native_lib, desugar_target, desugar_target_lenient};
use aivi_native_runtime::get_builtin;
use native_fixture::GeneratedModule;
use walkdir::WalkDir;

const PER_FILE_TIMEOUT: Duration = Duration::from_secs(30);

/// Run `f` on a background thread, aborting if it exceeds `timeout`.
fn with_timeout<T: Send + 'static>(
    timeout: Duration,
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(timeout)
        .map_err(|_| format!("timed out after {timeout:?}"))
}

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

        let rel_owned = rel_str.to_string();
        let result = with_timeout(PER_FILE_TIMEOUT, move || {
            let program = desugar_target(&rel_owned)?;
            compile_rust_native_lib(program)
        });

        let rust = match result {
            Ok(Ok(rust)) => rust,
            Ok(Err(err)) => {
                let msg = format!("{err}");
                if msg.contains("desugar") || msg.contains("Diagnostics") {
                    failures.push(format!("{rel_str}: desugar failed: {err}"));
                } else {
                    failures.push(format!("{rel_str}: native codegen failed: {err}"));
                }
                continue;
            }
            Err(timeout_msg) => {
                failures.push(format!("{rel_str}: {timeout_msg}"));
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

    // Compile ALL integration-test files together as a single program.
    // This mirrors real usage: the driver loads every .aivi file (plus embedded stdlib) into a
    // flat module set so cross-module `use` references resolve correctly.  Previous per-file
    // compilation produced hundreds of E0425 errors because each file's codegen was missing
    // definitions from every other module it imported.
    eprintln!("[native_codegen] compiling all integration-tests as whole program …");
    let t0 = Instant::now();

    let program = desugar_target_lenient("integration-tests/...")
        .expect("desugar_target_lenient(integration-tests/...) failed");
    let rust_code =
        compile_rust_native_lib(program).expect("compile_rust_native_lib failed for whole-program");

    eprintln!(
        "[native_codegen] whole-program codegen ok ({:?})",
        Instant::now().duration_since(t0)
    );

    // Check the combined Rust output via the sharded workspace.
    let modules = vec![GeneratedModule {
        mod_name: "integration_tests_all".to_string(),
        rust_code,
    }];
    if let Err(msg) = native_fixture::cargo_check_sharded(&root, &modules) {
        panic!("native codegen whole-program cargo check failed:\n{msg}");
    }
}
