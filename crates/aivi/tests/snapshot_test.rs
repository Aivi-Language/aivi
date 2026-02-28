/// Integration tests for snapshot testing: `assertSnapshot` and `mock snapshot`.
///
/// These tests exercise the full round-trip:
///   1. Run with `update_snapshots = true` → creates `.snap` files on disk.
///   2. Run with `update_snapshots = false` → replays from disk and verifies.
use std::sync::OnceLock;

use aivi::{
    check_modules, desugar_modules, elaborate_stdlib_checkpoint, elaborate_with_checkpoint,
    embedded_stdlib_modules, file_diagnostics_have_errors, parse_modules, run_test_suite,
    ElaborationCheckpoint,
};

#[path = "test_support.rs"]
#[allow(dead_code)]
mod test_support;

fn stdlib_checkpoint() -> &'static (Vec<aivi::surface::Module>, ElaborationCheckpoint) {
    static CACHE: OnceLock<(Vec<aivi::surface::Module>, ElaborationCheckpoint)> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut stdlib = embedded_stdlib_modules();
        let ckpt = elaborate_stdlib_checkpoint(&mut stdlib);
        (stdlib, ckpt)
    })
}

fn run_snapshot_test(
    source: &str,
    update: bool,
    project_root: &std::path::Path,
) -> aivi::TestReport {
    let (cached_stdlib, checkpoint) = stdlib_checkpoint();
    let path = std::path::Path::new("test.aivi");
    let (file_mods, _) = parse_modules(path, source);
    let mut modules = cached_stdlib.clone();
    modules.extend(file_mods);

    let mut diags = check_modules(&modules);
    if !file_diagnostics_have_errors(&diags) {
        diags.extend(elaborate_with_checkpoint(&mut modules, checkpoint));
    }
    diags.retain(|d| !d.path.starts_with("<embedded:"));
    assert!(
        !file_diagnostics_have_errors(&diags),
        "compile errors: {:?}",
        diags
            .iter()
            .filter(|d| d.diagnostic.severity == aivi::DiagnosticSeverity::Error)
            .map(|d| format!("{}: {}", d.path, d.diagnostic.message))
            .collect::<Vec<_>>()
    );

    let tests = test_support::collect_test_entries(&modules);
    assert!(!tests.is_empty(), "no @test entries found");

    let program = desugar_modules(&modules);
    run_test_suite(
        program,
        &tests,
        &modules,
        update,
        Some(project_root.to_path_buf()),
    )
    .expect("run_test_suite failed")
}

#[test]
fn assert_snapshot_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let source = r#"
@no_prelude
module test.snapshot

use aivi
use aivi.testing

@test "snapshot a record"
snap_record = do Effect {
  data = { name: "Ada", age: 42 }
  assertSnapshot "record_test" data
}
"#;

    // 1. Record (update mode)
    let report = run_snapshot_test(source, true, dir.path());
    assert_eq!(
        report.failures.len(),
        0,
        "update pass should succeed: {:?}",
        report.failures
    );

    // 2. Verify snapshot file was created
    let snap_file = dir
        .path()
        .join("__snapshots__/test.snapshot/snap_record/record_test.snap");
    assert!(
        snap_file.exists(),
        "snapshot file should exist at {}",
        snap_file.display()
    );
    let contents = std::fs::read_to_string(&snap_file).unwrap();
    assert!(contents.contains("Ada"), "snapshot should contain 'Ada'");

    // 3. Replay (verify mode)
    let report = run_snapshot_test(source, false, dir.path());
    assert_eq!(
        report.failures.len(),
        0,
        "replay pass should succeed: {:?}",
        report.failures
    );
}

#[test]
fn assert_snapshot_mismatch_fails() {
    let dir = tempfile::tempdir().unwrap();
    let source_v1 = r#"
@no_prelude
module test.mismatch

use aivi
use aivi.testing

@test "snapshot mismatch"
snap_mismatch = do Effect {
  data = { value: 1 }
  assertSnapshot "mismatch_test" data
}
"#;

    // 1. Record initial snapshot
    let report = run_snapshot_test(source_v1, true, dir.path());
    assert_eq!(report.failures.len(), 0, "recording should pass");

    // 2. Change the value and replay — should fail
    let source_v2 = r#"
@no_prelude
module test.mismatch

use aivi
use aivi.testing

@test "snapshot mismatch"
snap_mismatch = do Effect {
  _ <- assertSnapshot "mismatch_test" { value: 999 }
  pure Unit
}
"#;
    let report = run_snapshot_test(source_v2, false, dir.path());
    assert!(
        !report.failures.is_empty(),
        "mismatch should be reported as test failure, report: passed={}, failed={}, successes={:?}, failures={:?}",
        report.passed,
        report.failed,
        report.successes.iter().map(|s| &s.name).collect::<Vec<_>>(),
        report.failures.iter().map(|f| (&f.name, &f.message)).collect::<Vec<_>>()
    );
}
