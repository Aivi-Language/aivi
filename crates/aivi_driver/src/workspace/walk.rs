use std::fs;
use std::path::{Path, PathBuf};

use crate::AiviError;

/// Filters filesystem entries to AIVI source files so workspace expansion only returns compilable inputs.
pub(super) fn is_aivi_source(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("aivi")
}

/// Recursively walks directories and accumulates `.aivi` files for target expansion.
pub(super) fn collect_files(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<(), AiviError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            collect_files(&entry_path, paths)?;
            continue;
        }

        if is_aivi_source(&entry_path) {
            paths.push(entry_path);
        }
    }
    Ok(())
}
