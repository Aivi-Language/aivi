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
