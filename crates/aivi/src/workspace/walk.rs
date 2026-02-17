use std::fs;
use std::path::{Path, PathBuf};

use crate::AiviError;

pub(super) fn is_aivi_source(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("aivi")
}

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
