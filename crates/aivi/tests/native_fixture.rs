#![allow(dead_code)]

//! Shared helpers for native-codegen integration tests.
//!
//! The fixture crate at `tests/fixtures/native_smoke/` provides a **stable**
//! `Cargo.toml` (with the `aivi_native_runtime` dependency) and a
//! `.cargo/config.toml` that selects the `mold` linker.  Because the crate
//! identity never changes, Cargo's incremental / dependency cache is reused
//! across runs, drastically reducing link times.
//!
//! A static `Mutex` serialises tests that write to `src/main.rs` so parallel
//! `cargo test` invocations don't race on the same file.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Mutex;

/// Serialise access to the fixture crate so only one test writes
/// `src/main.rs` at a time.
pub static FIXTURE_LOCK: Mutex<()> = Mutex::new(());

/// Absolute path to the fixture crate.
pub fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/native_smoke")
}

/// Write `code` into the fixture's `src/main.rs` and `cargo run --quiet`.
/// Returns the `Output` of the child process.
pub fn cargo_run_fixture(code: &str) -> Output {
    let dir = fixture_dir();
    let src_main = dir.join("src/main.rs");
    // Remove any stale lib.rs left by a previous test to avoid compiling it
    let _ = std::fs::remove_file(dir.join("src/lib.rs"));

    std::fs::write(&src_main, code).expect("write src/main.rs");

    Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .env("RUSTFLAGS", "-Awarnings")
        .current_dir(&dir)
        .output()
        .expect("cargo run")
}

/// Write `code` into the fixture's `src/lib.rs` and `cargo build --quiet`.
/// Returns the `Output` of the child process.
pub fn cargo_build_fixture_lib(code: &str) -> Output {
    let dir = fixture_dir();
    let src_lib = dir.join("src/lib.rs");
    // ensure main.rs exists (Cargo needs at least one target)
    let src_main = dir.join("src/main.rs");
    if !src_main.exists() {
        std::fs::write(&src_main, "fn main() {}").ok();
    }

    std::fs::write(&src_lib, code).expect("write src/lib.rs");

    let out = Command::new("cargo")
        .arg("build")
        .arg("--lib")
        .arg("--quiet")
        .env("RUSTFLAGS", "-Awarnings")
        .current_dir(&dir)
        .output()
        .expect("cargo build");

    // remove lib.rs so subsequent tests don't pick it up
    let _ = std::fs::remove_file(&src_lib);
    out
}

// ---------------------------------------------------------------------------
// Multi-crate workspace for parallel `cargo check`
// ---------------------------------------------------------------------------

/// Number of shard crates to split generated Rust into.
/// Each shard is a separate crate → Cargo checks them in parallel.
const NUM_SHARDS: usize = 8;

/// Stable workspace directory for the multi-crate check.
/// Lives under the repo `target/` so `cargo clean` wipes it and `.gitignore`
/// already covers it.
fn check_ws_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join("target/native-check-ws")
}

/// Stable shared `CARGO_TARGET_DIR` so compiled `aivi_native_runtime` (and
/// other deps) are reused across runs and across shard crates.
fn check_target_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join("target/native-check-target")
}

/// A collected module ready to be placed into a shard.
pub struct GeneratedModule {
    pub mod_name: String,
    pub rust_code: String,
}

/// Write (or update) the multi-crate workspace and run `cargo check --workspace`.
///
/// Returns `Ok(())` on success, or `Err(message)` with combined stdout/stderr.
pub fn cargo_check_sharded(
    workspace_root: &Path,
    modules: &[GeneratedModule],
) -> Result<(), String> {
    let ws = check_ws_dir(workspace_root);
    let target = check_target_dir(workspace_root);
    let runtime_path = workspace_root.join("crates/aivi_native_runtime");

    // Determine actual shard count (don't create empty crates).
    let shard_count = NUM_SHARDS.min(modules.len()).max(1);
    let shard_size = (modules.len() + shard_count - 1) / shard_count;

    // -- workspace Cargo.toml --
    std::fs::create_dir_all(&ws).expect("create ws dir");
    let members: Vec<String> = (0..shard_count).map(|i| format!("\"shard_{i}\"")).collect();
    let ws_toml = format!(
        "[workspace]\nresolver = \"2\"\nmembers = [{}]\n",
        members.join(", ")
    );
    write_if_changed(&ws.join("Cargo.toml"), &ws_toml);

    // -- .cargo/config.toml (mold linker) --
    let cargo_dir = ws.join(".cargo");
    std::fs::create_dir_all(&cargo_dir).expect("create .cargo dir");
    let linker_cfg = fixture_dir().join(".cargo/config.toml");
    if linker_cfg.exists() {
        let content = std::fs::read_to_string(&linker_cfg).unwrap_or_default();
        write_if_changed(&cargo_dir.join("config.toml"), &content);
    }

    // -- shard crates --
    for shard_idx in 0..shard_count {
        let crate_name = format!("shard_{shard_idx}");
        let crate_dir = ws.join(&crate_name);
        let src_dir = crate_dir.join("src");
        std::fs::create_dir_all(&src_dir).expect("create shard src dir");

        let shard_toml = format!(
            "[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\naivi_native_runtime = {{ path = {:?} }}\n",
            runtime_path.display().to_string()
        );
        write_if_changed(&crate_dir.join("Cargo.toml"), &shard_toml);

        // Build lib.rs for this shard
        let start = shard_idx * shard_size;
        let end = (start + shard_size).min(modules.len());
        let mut lib_rs = String::new();
        for m in &modules[start..end] {
            lib_rs.push_str(&format!("pub mod {} {{\n", m.mod_name));
            for line in m.rust_code.lines() {
                lib_rs.push_str("    ");
                lib_rs.push_str(line);
                lib_rs.push('\n');
            }
            lib_rs.push_str("}\n\n");
        }
        write_if_changed(&src_dir.join("lib.rs"), &lib_rs);
    }

    // Clean up stale shard dirs from previous runs with more shards.
    for idx in shard_count.. {
        let stale = ws.join(format!("shard_{idx}"));
        if stale.exists() {
            let _ = std::fs::remove_dir_all(&stale);
        } else {
            break;
        }
    }

    // -- cargo check --workspace --
    let output = Command::new("cargo")
        .arg("check")
        .arg("--workspace")
        .arg("--quiet")
        .env("CARGO_TARGET_DIR", &target)
        .env("RUSTFLAGS", "-Awarnings")
        .current_dir(&ws)
        .output()
        .expect("cargo check");

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "cargo check failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        ))
    }
}

/// Only write a file if its content actually changed — avoids invalidating
/// Cargo's fingerprints and triggering unnecessary recompilation.
fn write_if_changed(path: &Path, content: &str) {
    if path.exists() {
        if let Ok(existing) = std::fs::read_to_string(path) {
            if existing == content {
                return;
            }
        }
    }
    std::fs::write(path, content).unwrap_or_else(|e| {
        panic!("failed to write {}: {e}", path.display());
    });
}

/// Assert that `output` succeeded, panicking with stdout/stderr on failure.
pub fn assert_cargo_success(output: &Output) {
    assert!(
        output.status.success(),
        "cargo failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Return the `stdout` of the given `Output` as a `String`.
pub fn stdout_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Write an `.aivi` source file to a temporary location and return its path
/// string.  The caller still owns the `TempDir`.
pub fn write_aivi_source(dir: &Path, name: &str, source: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, source).expect("write aivi source");
    path.to_string_lossy().into_owned()
}
