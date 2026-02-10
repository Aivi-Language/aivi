use std::path::PathBuf;

use aivi::{compile_rust, desugar_target};

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
fn compile_rust_example_emits_main() {
    let _root = set_workspace_root();
    let program = desugar_target("examples/11_concurrency.aivi").expect("desugar");
    let rust = compile_rust(program).expect("compile rust");
    assert!(rust.contains("fn main()"));
    assert!(rust.contains("PROGRAM_JSON"));
}
