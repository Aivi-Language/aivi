use std::path::PathBuf;

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
