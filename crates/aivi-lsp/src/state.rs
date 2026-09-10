use std::sync::{
    Arc, Mutex, RwLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use aivi_query::{RootDatabase, SourceFile};
use dashmap::{DashMap, mapref::entry::Entry};
use ropey::Rope;
use serde::Deserialize;
use tokio::sync::{Mutex as AsyncMutex, Notify, RwLock as AsyncRwLock};
use tower_lsp::lsp_types::Url;

use crate::analysis_pool::{AnalysisPool, CancellationToken};
use crate::semantic_tokens::SemanticTokenHistory;
use crate::workspace_index::WorkspaceIndex;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServerConfig {
    pub diagnostics_debounce_ms: u64,
    pub inlay_hints_enabled: bool,
    pub inlay_hints_max_length: usize,
    pub code_lens_enabled: bool,
}

impl ServerConfig {
    pub fn from_initialization_options(raw: Option<serde_json::Value>) -> Self {
        let defaults = Self::default();
        let options = raw
            .and_then(|value| serde_json::from_value::<InitializationOptions>(value).ok())
            .unwrap_or_default();
        Self {
            diagnostics_debounce_ms: options
                .diagnostics_debounce_ms
                .unwrap_or(defaults.diagnostics_debounce_ms),
            inlay_hints_enabled: options
                .inlay_hints_enabled
                .unwrap_or(defaults.inlay_hints_enabled),
            inlay_hints_max_length: options
                .inlay_hints_max_length
                .unwrap_or(defaults.inlay_hints_max_length)
                .max(4),
            code_lens_enabled: options
                .code_lens_enabled
                .unwrap_or(defaults.code_lens_enabled),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            diagnostics_debounce_ms: 200,
            inlay_hints_enabled: true,
            inlay_hints_max_length: 30,
            code_lens_enabled: true,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializationOptions {
    diagnostics_debounce_ms: Option<u64>,
    inlay_hints_enabled: Option<bool>,
    inlay_hints_max_length: Option<usize>,
    code_lens_enabled: Option<bool>,
}

/// Shared state for the language server.
pub struct ServerState {
    pub db: RootDatabase,
    pub(crate) documents: DashMap<Url, DocumentState>,
    pub analysis_pool: AnalysisPool,
    pub analysis_access: std::sync::Arc<AsyncRwLock<()>>,
    pub(crate) workspace_index: WorkspaceIndex,
    pub(crate) semantic_tokens: SemanticTokenHistory,
    pub diagnostic_publication: AsyncMutex<()>,
    pending_diagnostics: DashMap<Url, PendingDiagnostics>,
    diagnostic_lifecycle: Arc<DiagnosticLifecycle>,
    next_diagnostic_request: AtomicU64,
    config: RwLock<ServerConfig>,
    pub(crate) workspace_complete: AtomicBool,
    pub(crate) workspace_roots: RwLock<Vec<std::path::PathBuf>>,
    pub(crate) disk_files: RwLock<std::collections::BTreeMap<Url, SourceFile>>,
}

#[derive(Clone)]
pub(crate) struct DocumentState {
    pub file: SourceFile,
    pub version: i32,
    pub text: Rope,
}

#[derive(Clone, Debug)]
pub struct DocumentSnapshot {
    pub file: SourceFile,
    pub version: i32,
    pub text: Rope,
}

struct PendingDiagnostics {
    request_id: u64,
    cancellation: CancellationToken,
}

#[derive(Debug)]
struct DiagnosticLifecycle {
    state: Mutex<DiagnosticLifecycleState>,
    idle: Notify,
}

#[derive(Debug)]
struct DiagnosticLifecycleState {
    accepting: bool,
    active: usize,
}

pub(crate) struct DiagnosticTaskPermit {
    lifecycle: Arc<DiagnosticLifecycle>,
}

impl DiagnosticLifecycle {
    fn new() -> Self {
        Self {
            state: Mutex::new(DiagnosticLifecycleState {
                accepting: true,
                active: 0,
            }),
            idle: Notify::new(),
        }
    }

    fn try_begin(self: &Arc<Self>) -> Option<DiagnosticTaskPermit> {
        let mut state = self
            .state
            .lock()
            .expect("diagnostic lifecycle lock should not be poisoned");
        if !state.accepting {
            return None;
        }
        state.active = state
            .active
            .checked_add(1)
            .expect("diagnostic task count overflowed usize");
        Some(DiagnosticTaskPermit {
            lifecycle: Arc::clone(self),
        })
    }

    fn close(&self) {
        self.state
            .lock()
            .expect("diagnostic lifecycle lock should not be poisoned")
            .accepting = false;
    }

    fn is_accepting(&self) -> bool {
        self.state
            .lock()
            .expect("diagnostic lifecycle lock should not be poisoned")
            .accepting
    }

    async fn wait_until_idle(&self) {
        loop {
            let notified = self.idle.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self
                .state
                .lock()
                .expect("diagnostic lifecycle lock should not be poisoned")
                .active
                == 0
            {
                return;
            }
            notified.await;
        }
    }
}

impl Drop for DiagnosticTaskPermit {
    fn drop(&mut self) {
        let became_idle = {
            let mut state = self
                .lifecycle
                .state
                .lock()
                .expect("diagnostic lifecycle lock should not be poisoned");
            state.active = state
                .active
                .checked_sub(1)
                .expect("diagnostic task permit must correspond to one active task");
            state.active == 0
        };
        if became_idle {
            self.lifecycle.idle.notify_waiters();
        }
    }
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            db: RootDatabase::new(),
            documents: DashMap::new(),
            analysis_pool: AnalysisPool::default(),
            analysis_access: std::sync::Arc::new(AsyncRwLock::new(())),
            workspace_index: WorkspaceIndex::default(),
            semantic_tokens: SemanticTokenHistory::default(),
            diagnostic_publication: AsyncMutex::new(()),
            pending_diagnostics: DashMap::new(),
            diagnostic_lifecycle: Arc::new(DiagnosticLifecycle::new()),
            next_diagnostic_request: AtomicU64::new(0),
            config: RwLock::new(ServerConfig::default()),
            workspace_complete: AtomicBool::new(true),
            workspace_roots: RwLock::new(Vec::new()),
            disk_files: RwLock::new(std::collections::BTreeMap::new()),
        }
    }

    pub fn contains_document(&self, uri: &Url) -> bool {
        self.documents.contains_key(uri)
    }

    pub fn file(&self, uri: &Url) -> Option<SourceFile> {
        self.documents.get(uri).map(|document| document.file)
    }

    pub fn document_snapshot(&self, uri: &Url) -> Option<DocumentSnapshot> {
        self.documents.get(uri).map(|document| DocumentSnapshot {
            file: document.file,
            version: document.version,
            text: document.text.clone(),
        })
    }

    pub fn open_files(&self) -> Vec<(Url, SourceFile)> {
        let mut files = self
            .documents
            .iter()
            .map(|document| (document.key().clone(), document.file))
            .collect::<Vec<_>>();
        files.sort_by(|(left, _), (right, _)| left.as_str().cmp(right.as_str()));
        files
    }

    pub fn document_is_current(&self, uri: &Url, snapshot: &DocumentSnapshot) -> bool {
        self.documents.get(uri).is_some_and(|document| {
            document.file == snapshot.file && document.version == snapshot.version
        })
    }

    pub(crate) fn start_diagnostics(
        &self,
        uri: &Url,
    ) -> Option<(u64, CancellationToken, DiagnosticTaskPermit)> {
        let permit = self.diagnostic_lifecycle.try_begin()?;
        let request_id = self.next_diagnostic_request.fetch_add(1, Ordering::Relaxed);
        let cancellation = CancellationToken::default();
        let pending = PendingDiagnostics {
            request_id,
            cancellation: cancellation.clone(),
        };
        if let Some(previous) = self.pending_diagnostics.insert(uri.clone(), pending) {
            previous.cancellation.cancel();
        }
        // Shutdown can race the short interval between reserving the permit and
        // publishing this request in the per-document map. The lifecycle permit
        // makes shutdown wait for us; this check makes the request stop promptly.
        if !self.diagnostic_lifecycle.is_accepting() {
            cancellation.cancel();
        }
        Some((request_id, cancellation, permit))
    }

    pub fn finish_diagnostics(&self, uri: &Url, request_id: u64) {
        if let Entry::Occupied(entry) = self.pending_diagnostics.entry(uri.clone())
            && entry.get().request_id == request_id
        {
            entry.remove();
        }
    }

    pub fn cancel_diagnostics(&self, uri: &Url) {
        if let Some((_, pending)) = self.pending_diagnostics.remove(uri) {
            pending.cancellation.cancel();
        }
    }

    pub fn diagnostics_are_accepting(&self) -> bool {
        self.diagnostic_lifecycle.is_accepting()
    }

    fn begin_diagnostics_shutdown(&self) {
        self.diagnostic_lifecycle.close();
        let cancellations = self
            .pending_diagnostics
            .iter()
            .map(|pending| pending.cancellation.clone())
            .collect::<Vec<_>>();
        for cancellation in cancellations {
            cancellation.cancel();
        }
    }

    pub async fn shutdown_diagnostics(&self) {
        self.begin_diagnostics_shutdown();

        // Wait for a publication already in progress, but release the mutex
        // before draining: cancelled tasks may be waiting for this same lock so
        // that they can observe the closed lifecycle and exit.
        {
            let _publication = self.diagnostic_publication.lock().await;
        }
        self.diagnostic_lifecycle.wait_until_idle().await;
        self.pending_diagnostics.clear();
    }

    pub fn config(&self) -> ServerConfig {
        *self
            .config
            .read()
            .expect("server config lock should not be poisoned")
    }

    pub fn set_config(&self, config: ServerConfig) {
        *self
            .config
            .write()
            .expect("server config lock should not be poisoned") = config;
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use tower_lsp::lsp_types::Url;

    use super::{ServerConfig, ServerState};

    #[test]
    fn initialization_options_override_defaults() {
        let config = ServerConfig::from_initialization_options(Some(serde_json::json!({
            "diagnosticsDebounceMs": 75,
            "inlayHintsEnabled": false,
            "inlayHintsMaxLength": 12,
            "codeLensEnabled": false
        })));

        assert_eq!(config.diagnostics_debounce_ms, 75);
        assert!(!config.inlay_hints_enabled);
        assert_eq!(config.inlay_hints_max_length, 12);
        assert!(!config.code_lens_enabled);
    }

    #[test]
    fn newer_diagnostic_request_cancels_older_request_only() {
        let state = ServerState::new();
        let uri = Url::from_file_path(PathBuf::from("/state-tests/diagnostics.aivi"))
            .expect("test URI should be valid");

        let (first_id, first, first_permit) = state
            .start_diagnostics(&uri)
            .expect("running server should accept diagnostics");
        let (second_id, second, second_permit) = state
            .start_diagnostics(&uri)
            .expect("running server should accept replacement diagnostics");

        assert!(first.is_cancelled());
        assert!(!second.is_cancelled());
        state.finish_diagnostics(&uri, first_id);
        state.cancel_diagnostics(&uri);
        assert!(second.is_cancelled());
        drop(first_permit);
        drop(second_permit);
        state.finish_diagnostics(&uri, second_id);
    }

    #[tokio::test]
    async fn diagnostic_shutdown_rejects_new_work_and_drains_active_permits() {
        let state = Arc::new(ServerState::new());
        let uri = Url::from_file_path(PathBuf::from("/state-tests/shutdown.aivi"))
            .expect("test URI should be valid");
        let (_, cancellation, permit) = state
            .start_diagnostics(&uri)
            .expect("running server should accept diagnostics");

        state.begin_diagnostics_shutdown();

        assert!(cancellation.is_cancelled());
        assert!(!state.diagnostics_are_accepting());
        assert!(state.start_diagnostics(&uri).is_none());

        let draining = state.diagnostic_lifecycle.wait_until_idle();
        tokio::pin!(draining);
        tokio::select! {
            biased;
            () = &mut draining => panic!("shutdown must wait for the active diagnostic permit"),
            () = tokio::task::yield_now() => {}
        }
        drop(permit);
        draining.await;
    }
}
