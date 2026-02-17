use std::path::PathBuf;

use crate::AiviError;

use super::resolve::resolve_target_path;
use super::walk::{collect_files, is_aivi_source};

pub(crate) fn expand_target(target: &str) -> Result<Vec<PathBuf>, AiviError> {
    let mut paths = Vec::new();
    let (base, recursive) = if let Some(base) = target.strip_suffix("/...") {
        (if base.is_empty() { "." } else { base }, true)
    } else if let Some(base) = target.strip_suffix("/**") {
        (if base.is_empty() { "." } else { base }, true)
    } else {
        (target, false)
    };

    let Some(path) = resolve_target_path(base) else {
        return Err(AiviError::InvalidPath(target.to_string()));
    };

    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    if path.is_dir() {
        if recursive {
            collect_files(&path, &mut paths)?;
        } else {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let entry_path = entry.path();
                if entry_path.is_file() && is_aivi_source(&entry_path) {
                    paths.push(entry_path);
                }
            }
        }
    }

    paths.sort();
    if paths.is_empty() {
        return Err(AiviError::InvalidPath(target.to_string()));
    }

    Ok(paths)
}
