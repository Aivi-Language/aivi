use std::fs;
use std::path::PathBuf;

use aivi::parse_target;

#[test]
fn parse_hello_world_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root");
    std::env::set_current_dir(workspace_root).expect("set cwd");

    let bundle = parse_target("integration-tests/legacy/hello.aivi").expect("parse hello");
    let actual = serde_json::to_string_pretty(&bundle).expect("serialize");

    let golden_path = workspace_root.join("crates/aivi/tests/fixtures/hello.cst.json");
    let expected = fs::read_to_string(golden_path).expect("read golden");

    assert_eq!(actual.trim_end(), expected.trim_end());
}
