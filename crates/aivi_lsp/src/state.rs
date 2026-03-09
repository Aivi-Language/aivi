use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use aivi::Module;
use tower_lsp::lsp_types::Url;

use crate::doc_index::{DocIndex, DOC_INDEX_JSON};
use crate::gtk_index::{GtkIndex, GTK_INDEX_JSON};
use crate::strict::StrictConfig;

#[derive(Clone, Default)]
pub(super) struct DocumentState {
    pub(super) text: String,
    pub(super) version: i32,
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
    pub(super) sessions: HashMap<PathBuf, Arc<StdMutex<aivi_driver::WorkspaceSession>>>,
    pub(super) format_options: aivi::FormatOptions,
    pub(super) format_options_from_config: bool,
    pub(super) diagnostics_in_specs_snippets: bool,
    pub(super) strict: StrictConfig,
    pub(super) doc_index: Arc<DocIndex>,
    pub(super) gtk_index: Arc<GtkIndex>,
    /// Pre-built stdlib typecheck checkpoint; populated lazily on first diagnostic run.
    pub(super) typecheck_checkpoint: Option<aivi::CheckTypesCheckpoint>,
    /// Last known export-surface summaries for open modules, keyed by module name.
    pub(super) module_export_summaries: HashMap<String, aivi::ModuleExportSurfaceSummary>,
    /// Abort handle for the in-flight diagnostic task; used for per-keystroke cancellation.
    pub(super) pending_diagnostics: Option<tokio::task::AbortHandle>,
    /// Monotonic workspace snapshot token incremented on every semantic recheck request.
    pub(super) diagnostics_snapshot: u64,
}

impl Default for BackendState {
    fn default() -> Self {
        let doc_index = DocIndex::from_json(DOC_INDEX_JSON).unwrap_or_default();
        let gtk_index = GtkIndex::from_json(GTK_INDEX_JSON).unwrap_or_default();
        Self {
            documents: HashMap::new(),
            workspace_root: None,
            workspace_folders: Vec::new(),
            open_modules_by_uri: HashMap::new(),
            open_module_index: HashMap::new(),
            disk_indexes: HashMap::new(),
            sessions: HashMap::new(),
            format_options: aivi::FormatOptions::default(),
            format_options_from_config: false,
            diagnostics_in_specs_snippets: false,
            strict: StrictConfig::default(),
            doc_index: Arc::new(doc_index),
            gtk_index: Arc::new(gtk_index),
            typecheck_checkpoint: None,
            module_export_summaries: HashMap::new(),
            pending_diagnostics: None,
            diagnostics_snapshot: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct IndexedModule {
    pub(super) uri: Url,
    pub(super) module: Module,
    pub(super) text: Option<String>,
}
