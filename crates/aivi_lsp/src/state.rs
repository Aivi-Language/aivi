use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use aivi::Module;
use tower_lsp::lsp_types::Url;

use crate::doc_index::{DocIndex, DOC_INDEX_JSON};
use crate::strict::StrictConfig;

#[derive(Default)]
pub(super) struct DocumentState {
    pub(super) text: String,
    /// Parse diagnostics from the last `update_document` call; avoids re-parsing in diagnostics.
    pub(super) parse_diags: Vec<aivi::FileDiagnostic>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct DiskIndex {
    pub(super) modules_by_uri: HashMap<Url, Vec<String>>,
    pub(super) module_index: HashMap<String, IndexedModule>,
}

pub(super) struct BackendState {
    pub(super) documents: HashMap<Url, DocumentState>,
    pub(super) workspace_root: Option<PathBuf>,
    pub(super) workspace_folders: Vec<PathBuf>,
    pub(super) open_modules_by_uri: HashMap<Url, Vec<String>>,
    pub(super) open_module_index: HashMap<String, IndexedModule>,
    pub(super) disk_indexes: HashMap<PathBuf, DiskIndex>,
    pub(super) format_options: aivi::FormatOptions,
    pub(super) format_options_from_config: bool,
    pub(super) diagnostics_in_specs_snippets: bool,
    pub(super) strict: StrictConfig,
    pub(super) doc_index: Arc<DocIndex>,
    /// Pre-built stdlib typecheck checkpoint; populated lazily on first diagnostic run.
    pub(super) typecheck_checkpoint: Option<aivi::CheckTypesCheckpoint>,
    /// Abort handle for the in-flight diagnostic task; used for per-keystroke cancellation.
    pub(super) pending_diagnostics: Option<tokio::task::AbortHandle>,
    /// Monotonic counter incremented on every `didChange`; guards stale diagnostic publishes.
    pub(super) diagnostics_version: u64,
}

impl Default for BackendState {
    fn default() -> Self {
        let doc_index = DocIndex::from_json(DOC_INDEX_JSON).unwrap_or_default();
        Self {
            documents: HashMap::new(),
            workspace_root: None,
            workspace_folders: Vec::new(),
            open_modules_by_uri: HashMap::new(),
            open_module_index: HashMap::new(),
            disk_indexes: HashMap::new(),
            format_options: aivi::FormatOptions::default(),
            format_options_from_config: false,
            diagnostics_in_specs_snippets: false,
            strict: StrictConfig::default(),
            doc_index: Arc::new(doc_index),
            typecheck_checkpoint: None,
            pending_diagnostics: None,
            diagnostics_version: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct IndexedModule {
    pub(super) uri: Url,
    pub(super) module: Module,
    pub(super) text: Option<String>,
}
