use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use aivi::{embedded_stdlib_modules, parse_modules};
use tower_lsp::lsp_types::Url;

use crate::backend::Backend;
use crate::state::{DocumentState, IndexedModule};

impl Backend {
    pub(super) fn build_workspace_index(root: &Path) -> HashMap<String, IndexedModule> {
        let mut modules = HashMap::new();
        for path in Self::collect_aivi_paths(root) {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let (file_modules, _) = parse_modules(&path, &text);
            let Ok(uri) = Url::from_file_path(&path) else {
                continue;
            };
            for module in file_modules {
                modules
                    .entry(module.name.name.clone())
                    .or_insert_with(|| IndexedModule {
                        uri: uri.clone(),
                        module,
                        text: Some(text.clone()),
                    });
            }
        }

        modules
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
            )
        }

        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(entries) = fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                        if should_skip_dir(name) {
                            continue;
                        }
                    }
                    walk(&path, out);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) == Some("aivi") {
                    out.push(path);
                }
            }
        }

        let mut out = Vec::new();
        walk(root, &mut out);
        out.sort();
        out
    }

    pub(super) async fn workspace_modules_for(&self, uri: &Url) -> HashMap<String, IndexedModule> {
        let (workspace_root, open_modules, disk_modules, disk_root) = {
            let state = self.state.lock().await;
            (
                state.workspace_root.clone(),
                state.open_module_index.clone(),
                state.disk_module_index.clone(),
                state.disk_index_root.clone(),
            )
        };

        let fallback_root = PathBuf::from(Self::path_from_uri(uri))
            .parent()
            .map(|p| p.to_path_buf());
        let root = workspace_root.or(fallback_root);

        let disk_modules = if let Some(root) = root {
            let needs_rebuild = disk_root.as_ref() != Some(&root) || disk_modules.is_empty();
            if needs_rebuild {
                let indexed = Self::build_workspace_index(&root);
                let mut state = self.state.lock().await;
                state.disk_index_root = Some(root.clone());
                state.disk_module_index = indexed.clone();
                indexed
            } else {
                disk_modules
            }
        } else {
            disk_modules
        };

        let mut merged = disk_modules;
        merged.extend(open_modules);
        for module in embedded_stdlib_modules() {
            let name = module.name.name.clone();
            merged.entry(name.clone()).or_insert_with(|| IndexedModule {
                uri: Self::stdlib_uri(&name),
                module,
                text: None,
            });
        }
        merged
    }

    pub(super) async fn update_document(&self, uri: Url, text: String) {
        let path = PathBuf::from(Self::path_from_uri(&uri));
        let (modules, _) = parse_modules(&path, &text);

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
        state.documents.insert(uri, DocumentState { text });
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
