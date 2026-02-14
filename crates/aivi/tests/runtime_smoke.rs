use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

fn aivi_bin() -> &'static str {
    env!("CARGO_BIN_EXE_aivi")
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

fn run_native_via_cli(input: &str) {
    let _root = set_workspace_root();
    let output = Command::new(aivi_bin())
        .args(["run", input, "--target", "native"])
        .output()
        .expect("spawn aivi run");
    assert!(
        output.status.success(),
        "aivi run failed for {input}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn run_native_concurrency_example() {
    run_native_via_cli("examples/11_concurrency.aivi");
}

#[test]
fn run_native_effects_core_ops_example() {
    run_native_via_cli("examples/08_effects_core_ops.aivi");
}

#[test]
fn run_native_system_log_database_example() {
    run_native_via_cli("examples/18_system_log_database.aivi");
}

#[test]
fn run_native_crypto_example() {
    run_native_via_cli("examples/20_crypto.aivi");
}

#[test]
fn run_native_quaternion_example() {
    eprintln!("[DEBUG_LOG] quaternion: run start");
    let t0 = Instant::now();
    run_native_via_cli("examples/21_quaternion.aivi");
    eprintln!("[DEBUG_LOG] quaternion: run done in {:?}", t0.elapsed());
}

#[test]
fn run_native_fantasyland_law_tests() {
    eprintln!("[DEBUG_LOG] fantasyland laws: run start");
    let t0 = Instant::now();
    run_native_via_cli("examples/22_fantasyland_laws.aivi");
    eprintln!(
        "[DEBUG_LOG] fantasyland laws: run done in {:?}",
        t0.elapsed()
    );
}

#[test]
fn run_native_i18n_example() {
    run_native_via_cli("examples/23_i18n.aivi");
}

#[test]
fn run_native_i18n_catalog_fallback_example() {
    run_native_via_cli("examples/25_i18n_catalog_fallback.aivi");
}

#[test]
fn run_native_algorithms_example() {
    run_native_via_cli("examples/27_algorithms.aivi");
}
