use crate::AiviError;
use std::path::{Path, PathBuf};

pub fn collect_aivi_sources(src_dir: &Path) -> Result<Vec<PathBuf>, AiviError> {
    let mut paths = Vec::new();
    if !src_dir.exists() {
        return Ok(paths);
    }
    collect_aivi_sources_inner(src_dir, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn collect_aivi_sources_inner(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), AiviError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_aivi_sources_inner(&path, out)?;
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) == Some("aivi") {
            out.push(path);
        }
    }
    Ok(())
}
