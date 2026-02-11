use std::path::PathBuf;
use std::time::Instant;

use aivi::{desugar_target, run_native};

fn set_workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root");
    std::env::set_current_dir(workspace_root).expect("set cwd");
    workspace_root.to_path_buf()
}

#[test]
fn run_native_concurrency_example() {
    let _root = set_workspace_root();
    let program = desugar_target("examples/11_concurrency.aivi").expect("desugar");
    run_native(program).expect("run native");
}

#[test]
fn run_native_effects_core_ops_example() {
    let _root = set_workspace_root();
    let program = desugar_target("examples/08_effects_core_ops.aivi").expect("desugar");
    run_native(program).expect("run native");
}

#[test]
fn run_native_system_log_database_example() {
    let _root = set_workspace_root();
    let program = desugar_target("examples/18_system_log_database.aivi").expect("desugar");
    run_native(program).expect("run native");
}

#[test]
fn run_native_crypto_example() {
    let _root = set_workspace_root();
    let program = desugar_target("examples/20_crypto.aivi").expect("desugar");
    run_native(program).expect("run native");
}

#[test]
fn run_native_quaternion_example() {
    let _root = set_workspace_root();
    eprintln!("[DEBUG_LOG] quaternion: desugar start");
    let t0 = Instant::now();
    let program = desugar_target("examples/21_quaternion.aivi").expect("desugar");
    eprintln!("[DEBUG_LOG] quaternion: desugar done in {:?}", t0.elapsed());

    eprintln!("[DEBUG_LOG] quaternion: run_native start");
    let t1 = Instant::now();
    run_native(program).expect("run native");
    eprintln!(
        "[DEBUG_LOG] quaternion: run_native done in {:?}",
        t1.elapsed()
    );
}

#[test]
fn run_native_fantasyland_law_tests() {
    let _root = set_workspace_root();
    eprintln!("[DEBUG_LOG] fantasyland laws: desugar start");
    let t0 = Instant::now();
    let program = desugar_target("examples/22_fantasyland_laws.aivi").expect("desugar");
    eprintln!(
        "[DEBUG_LOG] fantasyland laws: desugar done in {:?}",
        t0.elapsed()
    );

    eprintln!("[DEBUG_LOG] fantasyland laws: run_native start");
    let t1 = Instant::now();
    run_native(program).expect("run native");
    eprintln!(
        "[DEBUG_LOG] fantasyland laws: run_native done in {:?}",
        t1.elapsed()
    );
}
