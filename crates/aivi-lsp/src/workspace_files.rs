//! Disk discovery and unsaved-buffer precedence for editor project snapshots.
use crate::state::ServerState;
use std::{collections::BTreeMap, path::PathBuf};
use tower_lsp::lsp_types::Url;

impl ServerState {
    pub fn set_workspace_roots(&self, mut roots: Vec<PathBuf>) {
        roots.sort();
        roots.dedup();
        *self.workspace_roots.write().expect("workspace roots lock") = roots;
    }

    /// Called on an analysis worker under the exclusive database lease.
    /// Symlink directories are not followed; traversal has bounded stack depth.
    pub fn refresh_workspace_files(&self) {
        let mut roots = self
            .workspace_roots
            .read()
            .expect("workspace roots lock")
            .clone();
        if roots.is_empty() {
            for (uri, _) in self.open_files() {
                let Ok(path) = uri.to_file_path() else {
                    continue;
                };
                let Some(parent) = path.parent() else {
                    continue;
                };
                let root = parent
                    .ancestors()
                    .find(|dir| dir.join("aivi.toml").is_file())
                    .unwrap_or(parent);
                roots.push(root.to_path_buf());
            }
        }
        roots.sort();
        roots.dedup();
        let mut complete = true;
        let mut pending = roots;
        let mut paths = std::collections::BTreeSet::new();
        let mut visited = std::collections::BTreeSet::new();
        while let Some(directory) = pending.pop() {
            if !visited.insert(directory.clone()) {
                continue;
            }
            let Ok(entries) = std::fs::read_dir(&directory) else {
                tracing::warn!(
                    ?directory,
                    "cannot index workspace directory; rename disabled"
                );
                complete = false;
                continue;
            };
            for entry in entries {
                let Ok(entry) = entry else {
                    complete = false;
                    continue;
                };
                let Ok(kind) = entry.file_type() else {
                    complete = false;
                    continue;
                };
                let path = entry.path();
                if kind.is_dir() {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if !name.starts_with('.') && !matches!(name.as_ref(), "target" | "node_modules")
                    {
                        pending.push(path);
                    }
                } else if kind.is_file()
                    && path
                        .extension()
                        .is_some_and(|extension| extension == "aivi")
                {
                    paths.insert(path);
                }
            }
        }
        let mut files = BTreeMap::new();
        for path in paths {
            let Ok(uri) = Url::from_file_path(&path) else {
                continue;
            };
            let file = if let Some(snapshot) = self.document_snapshot(&uri) {
                snapshot.file
            } else {
                let Ok(text) = std::fs::read_to_string(&path) else {
                    tracing::warn!(?path, "cannot read workspace source; rename disabled");
                    complete = false;
                    continue;
                };
                self.db.open_file(path, text)
            };
            files.insert(uri, file);
        }
        let mut previous = self.disk_files.write().expect("workspace files lock");
        for (uri, file) in previous.iter() {
            if !files.contains_key(uri) && !self.contains_document(uri) {
                self.db.remove_file(*file);
            }
        }
        *previous = files;
        self.workspace_complete
            .store(complete, std::sync::atomic::Ordering::Release);
    }

    pub fn project_files(&self) -> Vec<(Url, aivi_query::SourceFile)> {
        let mut files = self
            .disk_files
            .read()
            .expect("workspace files lock")
            .clone();
        files.extend(self.open_files());
        files.into_iter().collect()
    }
}
