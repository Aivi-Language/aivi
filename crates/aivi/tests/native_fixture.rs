#![allow(dead_code)]

//! Shared helpers for integration tests.

use std::path::Path;

/// Write an `.aivi` source file to a temporary location and return its path
/// string.  The caller still owns the `TempDir`.
pub fn write_aivi_source(dir: &Path, name: &str, source: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, source).expect("write aivi source");
    path.to_string_lossy().into_owned()
}
