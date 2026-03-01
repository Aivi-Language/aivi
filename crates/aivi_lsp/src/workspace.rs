use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use aivi::{embedded_stdlib_modules, parse_modules};
use tower_lsp::lsp_types::Url;

use crate::backend::Backend;
use crate::state::{DiskIndex, DocumentState, IndexedModule};

impl Backend {
    pub(super) fn build_disk_index(root: &Path) -> DiskIndex {
        let mut index = DiskIndex {
            modules_by_uri: HashMap::new(),
            module_index: HashMap::new(),
        };
        for path in Self::collect_aivi_paths(root) {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let (file_modules, _) = parse_modules(&path, &text);
            let Ok(uri) = Url::from_file_path(&path) else {
                continue;
            };
            let mut module_names = Vec::new();
            for module in file_modules {
                module_names.push(module.name.name.clone());
                index
                    .module_index
                    .entry(module.name.name.clone())
                    .or_insert_with(|| IndexedModule {
                        uri: uri.clone(),
                        module,
                        text: Some(text.clone()),
                    });
            }
            index.modules_by_uri.insert(uri, module_names);
        }

        index
    }

    fn collect_aivi_paths(root: &Path) -> Vec<PathBuf> {
        fn should_skip_dir(name: &str) -> bool {
            matches!(
                name,
                ".git"
                    | "target"
                    | "node_modules"
                    | "dist"
                    | "out"
                    | ".idea"
                    | ".junie"
                    | ".gemini"
                    | ".pnpm-store"
                    | ".ai"
                    | ".aiassistant"
                    | "specs"
                    | "vscode"
            )
        }

        let mut out = Vec::new();
        // Use an explicit stack (vs recursion) so very deep trees or symlink cycles can't
        // overflow the stack in tests / large workspaces.
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                // Avoid following symlinked directories (can introduce cycles).
                if file_type.is_symlink() {
                    continue;
                }
                let path = entry.path();
                if file_type.is_dir() {
                    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                        if should_skip_dir(name) {
                            continue;
                        }
                    }
                    stack.push(path);
                    continue;
                }
                if file_type.is_file() && path.extension().and_then(|e| e.to_str()) == Some("aivi")
                {
                    out.push(path);
                }
            }
        }
        out.sort();
        out
    }

    pub(super) fn project_root_for_path(
        path: &Path,
        workspace_folders: &[PathBuf],
    ) -> Option<PathBuf> {
        // Prefer an AIVI project root defined by `aivi.toml` (per `specs/tools/packaging.md`).
        for ancestor in path.ancestors() {
            if ancestor.join("aivi.toml").is_file() {
                return Some(ancestor.to_path_buf());
            }
        }

        // Fall back to whichever workspace folder contains the file.
        for folder in workspace_folders {
            if path.starts_with(folder) {
                return Some(folder.clone());
            }
        }

        // Final fallback: the file's parent directory (or the path itself if it's already a dir).
        if path.is_dir() {
            Some(path.to_path_buf())
        } else {
            path.parent().map(|p| p.to_path_buf())
        }
    }

    pub(super) async fn invalidate_disk_index_for_path(&self, path: &Path) {
        let workspace_folders = {
            let state = self.state.lock().await;
            state.workspace_folders.clone()
        };
        let Some(root) = Self::project_root_for_path(path, &workspace_folders) else {
            return;
        };
        let mut state = self.state.lock().await;
        state.disk_indexes.remove(&root);
    }

    pub(super) async fn refresh_disk_index_file(&self, path: &Path) {
        let workspace_folders = {
            let state = self.state.lock().await;
            state.workspace_folders.clone()
        };
        let Some(root) = Self::project_root_for_path(path, &workspace_folders) else {
            return;
        };

        let Ok(uri) = Url::from_file_path(path) else {
            return;
        };

        let Ok(text) = fs::read_to_string(path) else {
            // If the file can't be read (deleted, permissions), just invalidate and rebuild later.
            self.invalidate_disk_index_for_path(path).await;
            return;
        };
        let (file_modules, _) = parse_modules(path, &text);

        let mut state = self.state.lock().await;
        let Some(index) = state.disk_indexes.get_mut(&root) else {
            // We'll rebuild lazily on demand.
            return;
        };

        if let Some(existing) = index.modules_by_uri.remove(&uri) {
            for module_name in existing {
                // Only remove if it belonged to this file; duplicates from other files should stay.
                if let Some(existing_module) = index.module_index.get(&module_name) {
                    if existing_module.uri == uri {
                        index.module_index.remove(&module_name);
                    }
                }
            }
        }

        let mut module_names = Vec::new();
        for module in file_modules {
            let module_name = module.name.name.clone();
            module_names.push(module_name.clone());

            match index.module_index.get(&module_name) {
                Some(existing_module) if existing_module.uri != uri => {
                    // Duplicate module name from another file; keep first-seen mapping.
                    continue;
                }
                _ => {
                    index.module_index.insert(
                        module_name,
                        IndexedModule {
                            uri: uri.clone(),
                            module,
                            text: Some(text.clone()),
                        },
                    );
                }
            }
        }
        index.modules_by_uri.insert(uri, module_names);
    }

    pub(super) async fn remove_from_disk_index(&self, uri: &Url) {
        let path = PathBuf::from(Self::path_from_uri(uri));
        let workspace_folders = {
            let state = self.state.lock().await;
            state.workspace_folders.clone()
        };
        let Some(root) = Self::project_root_for_path(&path, &workspace_folders) else {
            return;
        };

        let mut state = self.state.lock().await;
        let Some(index) = state.disk_indexes.get_mut(&root) else {
            return;
        };

        if let Some(existing) = index.modules_by_uri.remove(uri) {
            for module_name in existing {
                if let Some(existing_module) = index.module_index.get(&module_name) {
                    if existing_module.uri == *uri {
                        index.module_index.remove(&module_name);
                    }
                }
            }
        }
    }

    pub(super) async fn workspace_modules_for(&self, uri: &Url) -> HashMap<String, IndexedModule> {
        let (workspace_folders, open_modules) = {
            let state = self.state.lock().await;
            (
                state.workspace_folders.clone(),
                state.open_module_index.clone(),
            )
        };

        let file_path = PathBuf::from(Self::path_from_uri(uri));
        let root = Self::project_root_for_path(&file_path, &workspace_folders);

        let disk_modules = if let Some(root) = root {
            let existing = {
                let state = self.state.lock().await;
                state.disk_indexes.get(&root).cloned()
            };
            let index = if let Some(existing) = existing {
                existing
            } else {
                let root_clone = root.clone();
                let built =
                    tokio::task::spawn_blocking(move || Self::build_disk_index(&root_clone))
                        .await
                        .ok();
                if let Some(built) = built {
                    let mut state = self.state.lock().await;
                    state.disk_indexes.insert(root.clone(), built.clone());
                    built
                } else {
                    DiskIndex::default()
                }
            };
            index.module_index
        } else {
            HashMap::new()
        };

        let mut merged = HashMap::new();
        for module in embedded_stdlib_modules() {
            let name = module.name.name.clone();
            merged.insert(
                name.clone(),
                IndexedModule {
                    uri: Self::stdlib_uri(&name),
                    module,
                    text: None,
                },
            );
        }
        // Keep embedded stdlib authoritative for `aivi.*` module names.
        for (name, indexed) in disk_modules {
            if name.starts_with("aivi.") {
                continue;
            }
            merged.entry(name).or_insert(indexed);
        }
        for (name, indexed) in open_modules {
            if name.starts_with("aivi.") {
                continue;
            }
            merged.insert(name, indexed);
        }
        merged
    }

    pub(super) async fn workspace_modules_for_diagnostics(
        &self,
        uri: &Url,
    ) -> HashMap<String, IndexedModule> {
        // Use the same module set as navigation/completions so that diagnostics can resolve
        // imports from files that aren't currently open.  The disk index is already cached after
        // the first build, so the only per-call cost is cloning the HashMap.
        self.workspace_modules_for(uri).await
    }

    pub(super) async fn update_document(&self, uri: Url, text: String) {
        let path = PathBuf::from(Self::path_from_uri(&uri));
        let text_clone = text.clone();
        let (modules, parse_diags) =
            tokio::task::spawn_blocking(move || parse_modules(&path, &text_clone))
                .await
                .unwrap_or_default();

        let mut state = self.state.lock().await;

        if let Some(existing) = state.open_modules_by_uri.remove(&uri) {
            for module_name in existing {
                state.open_module_index.remove(&module_name);
            }
        }

        let mut module_names = Vec::new();
        for module in modules {
            module_names.push(module.name.name.clone());
            state.open_module_index.insert(
                module.name.name.clone(),
                IndexedModule {
                    uri: uri.clone(),
                    module,
                    text: Some(text.clone()),
                },
            );
        }
        state.open_modules_by_uri.insert(uri.clone(), module_names);
        state
            .documents
            .insert(uri, DocumentState { text, parse_diags });
    }

    pub(super) async fn remove_document(&self, uri: &Url) {
        let mut state = self.state.lock().await;
        state.documents.remove(uri);
        if let Some(existing) = state.open_modules_by_uri.remove(uri) {
            for module_name in existing {
                state.open_module_index.remove(&module_name);
            }
        }
    }

    pub(super) async fn with_document_text<F, R>(&self, uri: &Url, f: F) -> Option<R>
    where
        F: FnOnce(&str) -> R,
    {
        let state = self.state.lock().await;
        state.documents.get(uri).map(|doc| f(&doc.text))
    }
}
