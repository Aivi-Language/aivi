use std::path::{Path, PathBuf};

/// Resolves target paths relative to cwd and, for legacy compatibility, parent Cargo workspaces.
pub(super) fn resolve_target_path(target: &str) -> Option<PathBuf> {
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
