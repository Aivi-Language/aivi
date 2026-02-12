use std::collections::HashMap;
use std::path::PathBuf;

use aivi::Module;
use tower_lsp::lsp_types::Url;

#[derive(Default)]
pub(super) struct DocumentState {
    pub(super) text: String,
}

#[derive(Debug, Clone, Default)]
pub(super) struct DiskIndex {
    pub(super) modules_by_uri: HashMap<Url, Vec<String>>,
    pub(super) module_index: HashMap<String, IndexedModule>,
}

#[derive(Default)]
pub(super) struct BackendState {
    pub(super) documents: HashMap<Url, DocumentState>,
    pub(super) workspace_root: Option<PathBuf>,
    pub(super) workspace_folders: Vec<PathBuf>,
    pub(super) open_modules_by_uri: HashMap<Url, Vec<String>>,
    pub(super) open_module_index: HashMap<String, IndexedModule>,
    pub(super) disk_indexes: HashMap<PathBuf, DiskIndex>,
    pub(super) format_options: aivi::FormatOptions,
    pub(super) format_options_from_config: bool,
}

#[derive(Debug, Clone)]
pub(super) struct IndexedModule {
    pub(super) uri: Url,
    pub(super) module: Module,
    pub(super) text: Option<String>,
}
