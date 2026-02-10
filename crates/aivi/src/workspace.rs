use std::fs;
use std::path::{Path, PathBuf};

use crate::AiviError;

pub(crate) fn expand_target(target: &str) -> Result<Vec<PathBuf>, AiviError> {
    let mut paths = Vec::new();
    let (base, recursive) = match target.strip_suffix("/...") {
        Some(base) => (if base.is_empty() { "." } else { base }, true),
        None => (target, false),
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
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let entry_path = entry.path();
                if entry_path.is_file() {
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

fn resolve_target_path(target: &str) -> Option<PathBuf> {
    let target_path = Path::new(target);
    if target_path.is_absolute() {
        return target_path.exists().then(|| target_path.to_path_buf());
    }

    if target_path.exists() {
        return Some(target_path.to_path_buf());
    }

    let Ok(mut dir) = std::env::current_dir() else {
        return None;
    };

    loop {
        if dir.join("Cargo.toml").exists() {
            let candidate = dir.join(target);
            if candidate.exists() {
                return Some(candidate);
            }
        }

        let Some(parent) = dir.parent() else {
            break;
        };
        dir = parent.to_path_buf();
    }

    None
}

fn collect_files(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<(), AiviError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            collect_files(&entry_path, paths)?;
            continue;
        }

        if entry_path.extension().and_then(|ext| ext.to_str()) == Some("aivi") {
            paths.push(entry_path);
        }
    }
    Ok(())
}
