use std::collections::HashMap;
use std::path::PathBuf;

use aivi::Module;
use tower_lsp::lsp_types::Url;

#[derive(Default)]
pub(super) struct DocumentState {
    pub(super) text: String,
}

#[derive(Default)]
pub(super) struct BackendState {
    pub(super) documents: HashMap<Url, DocumentState>,
    pub(super) workspace_root: Option<PathBuf>,
    pub(super) open_modules_by_uri: HashMap<Url, Vec<String>>,
    pub(super) open_module_index: HashMap<String, IndexedModule>,
    pub(super) disk_module_index: HashMap<String, IndexedModule>,
    pub(super) disk_index_root: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub(super) struct IndexedModule {
    pub(super) uri: Url,
    pub(super) module: Module,
}
