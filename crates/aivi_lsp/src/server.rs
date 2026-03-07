use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::request::{
    GotoDeclarationParams, GotoDeclarationResponse, GotoImplementationParams,
    GotoImplementationResponse,
};
use tower_lsp::lsp_types::{
    CodeActionOrCommand, CodeActionParams, CompletionItem, CompletionParams, CompletionResponse,
    DeclarationCapability, DidChangeConfigurationParams, DidChangeWatchedFilesParams,
    DocumentFormattingParams, DocumentRangeFormattingParams, DocumentSymbolParams,
    DocumentSymbolResponse, FileChangeType, FoldingRange, FoldingRangeParams,
    FoldingRangeProviderCapability, GotoDefinitionParams, GotoDefinitionResponse, Hover,
    HoverParams, HoverProviderCapability, ImplementationProviderCapability, InitializeParams,
    InitializeResult, InitializedParams, InlayHint, InlayHintParams, InlayHintServerCapabilities,
    Location, MessageType, OneOf, PrepareRenameResponse, ReferenceParams, RenameOptions,
    RenameParams, SelectionRange, SelectionRangeParams, SelectionRangeProviderCapability,
    SemanticTokensFullOptions, SemanticTokensOptions, SemanticTokensParams, SemanticTokensResult,
    SemanticTokensServerCapabilities, ServerCapabilities, SignatureHelp, SignatureHelpOptions,
    SignatureHelpParams, SymbolInformation, TextDocumentPositionParams, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextEdit, Url, WorkspaceEdit, WorkspaceSymbolParams,
};
use tower_lsp::{LanguageServer, LspService, Server};

use crate::backend::Backend;
use crate::state::{BackendState, IndexedModule};
use crate::strict::StrictLevel;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiviFormatConfig {
    indent_size: Option<usize>,
    max_blank_lines: Option<usize>,
    brace_style: Option<String>,
    max_width: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiviDiagnosticsConfig {
    include_specs_snippets: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiviStrictConfig {
    level: Option<u8>,
    forbid_implicit_coercions: Option<bool>,
    warnings_as_errors: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct AiviConfig {
    format: Option<AiviFormatConfig>,
    diagnostics: Option<AiviDiagnosticsConfig>,
    strict: Option<AiviStrictConfig>,
}

static PANIC_HOOK_INSTALLED: Once = Once::new();

#[derive(Clone)]
struct DiagnosticTarget {
    uri: Url,
    version: Option<i32>,
    text: String,
    parse_diags: Option<Vec<aivi::FileDiagnostic>>,
}

impl Backend {
    fn changed_module_names(
        previous: &HashMap<String, aivi::ModuleExportSurfaceSummary>,
        current: &HashMap<String, aivi::ModuleExportSurfaceSummary>,
    ) -> HashSet<String> {
        let mut changed = HashSet::new();
        for name in previous.keys().chain(current.keys()) {
            if previous.get(name) != current.get(name) {
                changed.insert(name.clone());
            }
        }
        changed
    }

    async fn document_module_export_summaries(
        &self,
        uri: &Url,
    ) -> HashMap<String, aivi::ModuleExportSurfaceSummary> {
        let state = self.state.lock().await;
        state
            .open_modules_by_uri
            .get(uri)
            .into_iter()
            .flat_map(|module_names| module_names.iter())
            .filter_map(|module_name| {
                state
                    .module_export_summaries
                    .get(module_name)
                    .cloned()
                    .map(|summary| (module_name.clone(), summary))
            })
            .collect()
    }

    fn module_export_summaries_from_workspace(
        uri: &Url,
        workspace: &HashMap<String, IndexedModule>,
    ) -> HashMap<String, aivi::ModuleExportSurfaceSummary> {
        workspace
            .iter()
            .filter(|(_, indexed)| indexed.uri == *uri)
            .map(|(module_name, indexed)| {
                (
                    module_name.clone(),
                    aivi::summarize_module_export_surface(&indexed.module),
                )
            })
            .collect()
    }

    async fn open_dependents_for_recheck(
        &self,
        source_uri: &Url,
        changed_modules: &HashSet<String>,
        workspace: &HashMap<String, IndexedModule>,
    ) -> Vec<DiagnosticTarget> {
        if changed_modules.is_empty() {
            return Vec::new();
        }

        let (documents, open_module_index) = {
            let state = self.state.lock().await;
            (state.documents.clone(), state.open_module_index.clone())
        };

        let all_modules: Vec<aivi::Module> = workspace
            .values()
            .map(|indexed| indexed.module.clone())
            .collect();
        let reverse_deps = aivi::reverse_module_dependencies(&all_modules);
        let ordered_names = aivi::ordered_module_names(&all_modules);

        let mut dirty_modules: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<String> = changed_modules.iter().cloned().collect();
        while let Some(module_name) = queue.pop_front() {
            let Some(dependents) = reverse_deps.get(&module_name) else {
                continue;
            };
            for dependent in dependents {
                if dirty_modules.insert(dependent.clone()) {
                    queue.push_back(dependent.clone());
                }
            }
        }

        let mut seen_uris = HashSet::new();
        let mut targets = Vec::new();
        for module_name in ordered_names {
            if !dirty_modules.contains(&module_name) {
                continue;
            }
            let Some(indexed) = open_module_index.get(&module_name) else {
                continue;
            };
            if &indexed.uri == source_uri || !seen_uris.insert(indexed.uri.clone()) {
                continue;
            }
            let Some(document) = documents.get(&indexed.uri) else {
                continue;
            };
            targets.push(DiagnosticTarget {
                uri: indexed.uri.clone(),
                version: Some(document.version),
                text: document.text.clone(),
                parse_diags: Some(document.parse_diags.clone()),
            });
        }
        targets
    }

    async fn begin_diagnostics_snapshot(&self) -> u64 {
        let mut state = self.state.lock().await;
        if let Some(handle) = state.pending_diagnostics.take() {
            handle.abort();
        }
        state.diagnostics_snapshot = state.diagnostics_snapshot.wrapping_add(1);
        state.diagnostics_snapshot
    }

    async fn diagnostics_context(
        &self,
    ) -> (
        bool,
        StrictLevel,
        crate::strict::StrictConfig,
        Option<aivi::CheckTypesCheckpoint>,
    ) {
        let state = self.state.lock().await;
        (
            state.diagnostics_in_specs_snippets,
            state.strict.level,
            state.strict.clone(),
            state.typecheck_checkpoint.clone(),
        )
    }

    async fn compute_target_diagnostics(
        client: &tower_lsp::Client,
        operation: &str,
        target: DiagnosticTarget,
        workspace: HashMap<String, IndexedModule>,
        include_specs_snippets: bool,
        strict: crate::strict::StrictConfig,
        checkpoint: Option<aivi::CheckTypesCheckpoint>,
    ) -> (
        Vec<tower_lsp::lsp_types::Diagnostic>,
        Option<aivi::CheckTypesCheckpoint>,
    ) {
        match tokio::task::spawn_blocking(move || {
            let (cp, is_new) = match checkpoint {
                Some(cp) => (cp, false),
                None => {
                    let stdlib = aivi::embedded_stdlib_modules();
                    (aivi::check_types_stdlib_checkpoint(&stdlib), true)
                }
            };
            let diags = Backend::build_diagnostics_with_workspace(
                &target.text,
                &target.uri,
                &workspace,
                include_specs_snippets,
                &strict,
                target.parse_diags,
                Some(&cp),
            );
            (diags, is_new.then_some(cp))
        })
        .await
        {
            Ok(result) => result,
            Err(err) => {
                Backend::log_join_error_with_client(client, operation, &err).await;
                (Vec::new(), None)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_diagnostics_publish_task(
        client: tower_lsp::Client,
        state_arc: Arc<Mutex<crate::state::BackendState>>,
        snapshot: u64,
        operation: &'static str,
        started: Instant,
        telemetry_detail: String,
        workspace: HashMap<String, IndexedModule>,
        current_target: Option<DiagnosticTarget>,
        dependent_targets: Vec<DiagnosticTarget>,
        include_specs_snippets: bool,
        strict: crate::strict::StrictConfig,
        checkpoint: Option<aivi::CheckTypesCheckpoint>,
    ) {
        let mut checkpoint = checkpoint;
        let mut published_targets = 0usize;
        let dependent_target_count = dependent_targets.len();

        if let Some(target) = current_target {
            let (diagnostics, new_checkpoint) = Backend::compute_target_diagnostics(
                &client,
                operation,
                target.clone(),
                workspace.clone(),
                include_specs_snippets,
                strict.clone(),
                checkpoint.clone(),
            )
            .await;
            if checkpoint.is_none() {
                checkpoint = new_checkpoint.clone();
            }

            tokio::task::yield_now().await;
            let should_publish = {
                let state = state_arc.lock().await;
                state.diagnostics_snapshot == snapshot
                    && target.version.is_none_or(|version| {
                        state
                            .documents
                            .get(&target.uri)
                            .map(|document| document.version == version)
                            .unwrap_or(false)
                    })
            };
            if !should_publish {
                let mut state = state_arc.lock().await;
                if let Some(cp) = checkpoint {
                    state.typecheck_checkpoint.get_or_insert(cp);
                }
                if state.diagnostics_snapshot == snapshot {
                    state.pending_diagnostics = None;
                }
                Backend::log_telemetry_with_client(
                    &client,
                    operation,
                    started.elapsed(),
                    format!(
                        "{telemetry_detail} count={} snapshot={snapshot} published=false targets={published_targets}",
                        diagnostics.len()
                    ),
                )
                .await;
                return;
            }

            Backend::log_telemetry_with_client(
                &client,
                operation,
                started.elapsed(),
                format!(
                    "{telemetry_detail} count={} snapshot={snapshot} published=true targets={}",
                    diagnostics.len(),
                    1 + dependent_target_count
                ),
            )
            .await;
            client
                .publish_diagnostics(target.uri, diagnostics, target.version)
                .await;
            published_targets += 1;
        }

        for target in dependent_targets {
            let is_current = {
                let state = state_arc.lock().await;
                state.diagnostics_snapshot == snapshot
            };
            if !is_current {
                break;
            }

            let (diagnostics, new_checkpoint) = Backend::compute_target_diagnostics(
                &client,
                operation,
                target.clone(),
                workspace.clone(),
                include_specs_snippets,
                strict.clone(),
                checkpoint.clone(),
            )
            .await;
            if checkpoint.is_none() {
                checkpoint = new_checkpoint.clone();
            }

            tokio::task::yield_now().await;
            let should_publish = {
                let state = state_arc.lock().await;
                state.diagnostics_snapshot == snapshot
                    && target.version.is_none_or(|version| {
                        state
                            .documents
                            .get(&target.uri)
                            .map(|document| document.version == version)
                            .unwrap_or(false)
                    })
            };
            if !should_publish {
                break;
            }

            client
                .publish_diagnostics(target.uri, diagnostics, target.version)
                .await;
            published_targets += 1;
        }

        let still_current = {
            let mut state = state_arc.lock().await;
            if let Some(cp) = checkpoint {
                state.typecheck_checkpoint.get_or_insert(cp);
            }
            let is_current = state.diagnostics_snapshot == snapshot;
            if is_current {
                state.pending_diagnostics = None;
            }
            is_current
        };
        Backend::log_telemetry_with_client(
            &client,
            operation,
            started.elapsed(),
            format!(
                "{telemetry_detail} count=0 snapshot={snapshot} published={still_current} targets={published_targets}"
            ),
        )
        .await;
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_diagnostics_publish_task(
        &self,
        snapshot: u64,
        operation: &'static str,
        started: Instant,
        initial_delay: Option<Duration>,
        telemetry_detail: String,
        workspace: HashMap<String, IndexedModule>,
        current_target: Option<DiagnosticTarget>,
        dependent_targets: Vec<DiagnosticTarget>,
        include_specs_snippets: bool,
        strict: crate::strict::StrictConfig,
        checkpoint: Option<aivi::CheckTypesCheckpoint>,
    ) -> tokio::task::JoinHandle<()> {
        let client = self.client.clone();
        let state_arc = Arc::clone(&self.state);
        tokio::spawn(async move {
            if let Some(delay) = initial_delay {
                tokio::time::sleep(delay).await;
                let is_current = {
                    let state = state_arc.lock().await;
                    state.diagnostics_snapshot == snapshot
                };
                if !is_current {
                    return;
                }
            }
            Backend::run_diagnostics_publish_task(
                client,
                state_arc,
                snapshot,
                operation,
                started,
                telemetry_detail,
                workspace,
                current_target,
                dependent_targets,
                include_specs_snippets,
                strict,
                checkpoint,
            )
            .await;
        })
    }

    fn install_panic_hook(&self) {
        let client = self.client.clone();
        let runtime = tokio::runtime::Handle::current();
        PANIC_HOOK_INSTALLED.call_once(move || {
            let previous = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |panic_info| {
                let message = Self::panic_telemetry_message(panic_info);
                let client = client.clone();
                std::mem::drop(runtime.spawn(async move {
                    client.log_message(MessageType::ERROR, message).await;
                }));
                previous(panic_info);
            }));
        });
    }

    pub(crate) fn format_telemetry_message(
        operation: &str,
        elapsed: Duration,
        detail: &str,
    ) -> String {
        if detail.is_empty() {
            format!(
                "[telemetry] {operation} duration_ms={}",
                elapsed.as_millis()
            )
        } else {
            format!(
                "[telemetry] {operation} duration_ms={} {detail}",
                elapsed.as_millis()
            )
        }
    }

    fn panic_telemetry_message(info: &std::panic::PanicHookInfo<'_>) -> String {
        let payload = if let Some(message) = info.payload().downcast_ref::<&str>() {
            (*message).to_string()
        } else if let Some(message) = info.payload().downcast_ref::<String>() {
            message.clone()
        } else {
            "non-string panic payload".to_string()
        };
        let location = info
            .location()
            .map(|location| {
                format!(
                    "{}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            })
            .unwrap_or_else(|| "unknown location".to_string());
        format!("[telemetry] panic event at {location}: {payload}")
    }

    async fn log_telemetry(&self, operation: &str, elapsed: Duration, detail: String) {
        Self::log_telemetry_with_client(&self.client, operation, elapsed, detail).await;
    }

    async fn log_telemetry_with_client(
        client: &tower_lsp::Client,
        operation: &str,
        elapsed: Duration,
        detail: String,
    ) {
        client
            .log_message(
                MessageType::LOG,
                Self::format_telemetry_message(operation, elapsed, &detail),
            )
            .await;
    }

    async fn log_join_error(&self, operation: &str, err: &tokio::task::JoinError) {
        Self::log_join_error_with_client(&self.client, operation, err).await;
    }

    async fn log_join_error_with_client(
        client: &tower_lsp::Client,
        operation: &str,
        err: &tokio::task::JoinError,
    ) {
        if err.is_cancelled() {
            return;
        }
        let level = if err.is_panic() {
            MessageType::ERROR
        } else {
            MessageType::WARNING
        };
        client
            .log_message(level, format!("[telemetry] {operation} task failed: {err}"))
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        self.install_panic_hook();
        let mut workspace_folders: Vec<std::path::PathBuf> = Vec::new();
        if let Some(folders) = params.workspace_folders.as_ref() {
            for folder in folders {
                if let Ok(path) = folder.uri.to_file_path() {
                    workspace_folders.push(path);
                }
            }
        }
        if workspace_folders.is_empty() {
            if let Some(root) = params.root_uri.and_then(|uri| uri.to_file_path().ok()) {
                workspace_folders.push(root);
            }
        }

        {
            let mut state = self.state.lock().await;
            state.workspace_root = workspace_folders.first().cloned();
            state.workspace_folders = workspace_folders.clone();
        }

        // Indexing can be expensive; build caches in the background.
        for root in workspace_folders {
            let state = Arc::clone(&self.state);
            tokio::spawn(async move {
                let root_clone = root.clone();
                let built =
                    tokio::task::spawn_blocking(move || Backend::build_disk_index(&root_clone))
                        .await
                        .ok();
                let Some(built) = built else { return };
                let mut locked = state.lock().await;
                locked.disk_indexes.insert(root, built);
            });
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                document_symbol_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                declaration_provider: Some(DeclarationCapability::Simple(true)),
                implementation_provider: Some(ImplementationProviderCapability::Simple(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec![" ".to_string()]),
                    // AIVI uses whitespace application (`f x y`), so space is the natural trigger.
                    // Also retrigger on space so editors can refresh the active-parameter highlight.
                    retrigger_characters: Some(vec![" ".to_string()]),
                    work_done_progress_options: Default::default(),
                }),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: Default::default(),
                })),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: Self::semantic_tokens_legend(),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            range: None,
                            work_done_progress_options: Default::default(),
                        },
                    ),
                ),
                code_action_provider: Some(
                    tower_lsp::lsp_types::CodeActionProviderCapability::Simple(true),
                ),
                completion_provider: Some(tower_lsp::lsp_types::CompletionOptions {
                    resolve_provider: Some(true),
                    trigger_characters: Some(vec![".".to_string(), "(".to_string()]),
                    ..tower_lsp::lsp_types::CompletionOptions::default()
                }),
                document_formatting_provider: Some(OneOf::Right(
                    tower_lsp::lsp_types::DocumentFormattingOptions {
                        work_done_progress_options: Default::default(),
                    },
                )),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
                inlay_hint_provider: Some(OneOf::Right(InlayHintServerCapabilities::Options(
                    tower_lsp::lsp_types::InlayHintOptions {
                        resolve_provider: Some(false),
                        work_done_progress_options: Default::default(),
                    },
                ))),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
            server_info: Some(tower_lsp::lsp_types::ServerInfo {
                name: "aivi-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(
                tower_lsp::lsp_types::MessageType::INFO,
                "aivi-lsp initialized",
            )
            .await;
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        let config: AiviConfig = match serde_json::from_value(params.settings) {
            Ok(cfg) => cfg,
            Err(err) => {
                self.client
                    .log_message(
                        tower_lsp::lsp_types::MessageType::WARNING,
                        format!("Failed to parse configuration: {err}"),
                    )
                    .await;
                return;
            }
        };

        let mut state = self.state.lock().await;
        state.format_options_from_config = true;

        if let Some(format) = config.format {
            if let Some(indent_size) = format.indent_size {
                state.format_options.indent_size = indent_size;
            }
            if let Some(max_blank_lines) = format.max_blank_lines {
                state.format_options.max_blank_lines = max_blank_lines;
            }
            if let Some(brace_style) = format.brace_style {
                let v = brace_style.to_ascii_lowercase();
                state.format_options.brace_style = match v.as_str() {
                    "kr" | "k&r" | "knr" | "ts" | "java" => aivi::BraceStyle::Kr,
                    "allman" => aivi::BraceStyle::Allman,
                    _ => state.format_options.brace_style,
                };
            }
            if let Some(max_width) = format.max_width {
                state.format_options.max_width = max_width;
            }
        }

        if let Some(diagnostics) = config.diagnostics {
            if let Some(include) = diagnostics.include_specs_snippets {
                state.diagnostics_in_specs_snippets = include;
            }
        }

        if let Some(strict) = config.strict {
            if let Some(level) = strict.level {
                state.strict.level = StrictLevel::from_u8(level);
            }
            if let Some(forbid) = strict.forbid_implicit_coercions {
                state.strict.forbid_implicit_coercions = forbid;
            }
            if let Some(warnings_as_errors) = strict.warnings_as_errors {
                state.strict.warnings_as_errors = warnings_as_errors;
            }
        }
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: tower_lsp::lsp_types::DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let version = params.text_document.version;
        let uri_display = uri.to_string();
        let previous_workspace = self.workspace_modules_for(&uri).await;
        let previous_summaries =
            Self::module_export_summaries_from_workspace(&uri, &previous_workspace);
        self.update_document(uri.clone(), text.clone(), version)
            .await;
        let workspace = self.workspace_modules_for_diagnostics(&uri).await;
        let current_summaries = Self::module_export_summaries_from_workspace(&uri, &workspace);
        let changed_modules = Self::changed_module_names(&previous_summaries, &current_summaries);
        let export_changed = !changed_modules.is_empty();
        let dependent_targets = self
            .open_dependents_for_recheck(&uri, &changed_modules, &workspace)
            .await;
        let (include_specs_snippets, strict_level, strict, checkpoint) =
            self.diagnostics_context().await;
        let parse_diags = {
            let state = self.state.lock().await;
            state
                .documents
                .get(&uri)
                .map(|doc| doc.parse_diags.clone())
                .unwrap_or_default()
        };
        let mut changed_module_names: Vec<String> = changed_modules.into_iter().collect();
        changed_module_names.sort();
        let diagnostics_started = Instant::now();
        let snapshot = self.begin_diagnostics_snapshot().await;
        let current_target = DiagnosticTarget {
            uri: uri.clone(),
            version: Some(version),
            text,
            parse_diags: Some(parse_diags),
        };
        let handle = self.spawn_diagnostics_publish_task(
            snapshot,
            "diagnostics.did_open",
            diagnostics_started,
            None,
            format!(
                "uri={uri_display} version={version} strict={} export_changed={export_changed} dependents={} changed_modules={}",
                strict_level as u8,
                dependent_targets.len(),
                changed_module_names.join(",")
            ),
            workspace,
            Some(current_target),
            dependent_targets,
            include_specs_snippets,
            strict,
            checkpoint,
        );
        let mut state = self.state.lock().await;
        if state.diagnostics_snapshot == snapshot {
            state.pending_diagnostics = Some(handle.abort_handle());
        } else {
            handle.abort();
        }
    }

    async fn did_change(&self, params: tower_lsp::lsp_types::DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;

        // Apply incremental edits to the current document text.
        let text = {
            let state = self.state.lock().await;
            let mut text = state
                .documents
                .get(&uri)
                .map(|doc| doc.text.clone())
                .unwrap_or_default();
            for change in params.content_changes {
                if let Some(range) = change.range {
                    let start = Self::offset_at(&text, range.start).min(text.len());
                    let end = Self::offset_at(&text, range.end).min(text.len());
                    text.replace_range(start..end, &change.text);
                } else {
                    // Full content replacement (fallback).
                    text = change.text;
                }
            }
            text
        };

        // Capture parse diagnostics for *this* handler's text before the debounce sleep so
        // a concurrently executed stale handler cannot overwrite the document state with its
        // own (potentially broken) parse results and corrupt what we pass to the type-checker.
        let previous_summaries = self.document_module_export_summaries(&uri).await;
        let parse_diags = self
            .update_document(uri.clone(), text.clone(), version)
            .await;
        let current_summaries = self.document_module_export_summaries(&uri).await;
        let changed_modules = Self::changed_module_names(&previous_summaries, &current_summaries);
        let export_changed = !changed_modules.is_empty();

        // Phase 4: debounce against a workspace snapshot token so superseded work never publishes.
        let current_snapshot = self.begin_diagnostics_snapshot().await;
        let diagnostics_started = Instant::now();
        let uri_display = uri.to_string();
        let current_target = DiagnosticTarget {
            uri: uri.clone(),
            version: Some(version),
            text,
            parse_diags: Some(parse_diags),
        };
        let backend = Backend {
            client: self.client.clone(),
            state: Arc::clone(&self.state),
        };
        let changed_modules_for_task = changed_modules.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            {
                let state = backend.state.lock().await;
                if state.diagnostics_snapshot != current_snapshot {
                    return;
                }
            }

            let workspace = backend.workspace_modules_for_diagnostics(&uri).await;
            let dependent_targets = backend
                .open_dependents_for_recheck(&uri, &changed_modules_for_task, &workspace)
                .await;
            let (include_specs_snippets, strict_level, strict, checkpoint) =
                backend.diagnostics_context().await;
            let mut changed_module_names: Vec<String> =
                changed_modules_for_task.into_iter().collect();
            changed_module_names.sort();
            Backend::run_diagnostics_publish_task(
                backend.client.clone(),
                Arc::clone(&backend.state),
                current_snapshot,
                "diagnostics.did_change",
                diagnostics_started,
                format!(
                    "uri={uri_display} version={version} strict={} export_changed={export_changed} dependents={} changed_modules={}",
                    strict_level as u8,
                    dependent_targets.len(),
                    changed_module_names.join(",")
                ),
                workspace,
                Some(current_target),
                dependent_targets,
                include_specs_snippets,
                strict,
                checkpoint,
            )
            .await;
        });

        // Store the abort handle so the next keystroke can cancel this task.
        let mut state = self.state.lock().await;
        if state.diagnostics_snapshot == current_snapshot {
            state.pending_diagnostics = Some(handle.abort_handle());
        } else {
            handle.abort();
        }
    }

    async fn did_close(&self, params: tower_lsp::lsp_types::DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        let snapshot = self.begin_diagnostics_snapshot().await;
        let previous_summaries = self.document_module_export_summaries(&uri).await;
        self.remove_document(&uri).await;
        self.client
            .publish_diagnostics(uri.clone(), Vec::new(), None)
            .await;
        let workspace = self.workspace_modules_for_diagnostics(&uri).await;
        let current_summaries = Self::module_export_summaries_from_workspace(&uri, &workspace);
        let changed_modules = Self::changed_module_names(&previous_summaries, &current_summaries);
        let dependent_targets = self
            .open_dependents_for_recheck(&uri, &changed_modules, &workspace)
            .await;
        let (include_specs_snippets, strict_level, strict, checkpoint) =
            self.diagnostics_context().await;
        let mut changed_module_names: Vec<String> = changed_modules.into_iter().collect();
        changed_module_names.sort();
        if dependent_targets.is_empty() {
            let mut state = self.state.lock().await;
            if state.diagnostics_snapshot == snapshot {
                state.pending_diagnostics = None;
            }
            return;
        }

        let handle = self.spawn_diagnostics_publish_task(
            snapshot,
            "diagnostics.did_close",
            Instant::now(),
            None,
            format!(
                "strict={} dependents={} changed_modules={}",
                strict_level as u8,
                dependent_targets.len(),
                changed_module_names.join(",")
            ),
            workspace,
            None,
            dependent_targets,
            include_specs_snippets,
            strict,
            checkpoint,
        );
        let mut state = self.state.lock().await;
        if state.diagnostics_snapshot == snapshot {
            state.pending_diagnostics = Some(handle.abort_handle());
        } else {
            handle.abort();
        }
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        // Prefer client-side watchers (VS Code `FileSystemWatcher`) for reliability across OSes.
        // Keep the on-disk module index in sync so cross-file navigation stays fresh.
        let workspace_folders = {
            let state = self.state.lock().await;
            state.workspace_folders.clone()
        };
        let mut affected_roots: HashSet<PathBuf> = HashSet::new();
        for change in params.changes {
            let Ok(path) = change.uri.to_file_path() else {
                continue;
            };
            if let Some(root) = Self::project_root_for_path(&path, &workspace_folders) {
                affected_roots.insert(root);
            }
            match change.typ {
                FileChangeType::CREATED | FileChangeType::CHANGED => {
                    if path.extension().and_then(|e| e.to_str()) == Some("aivi") {
                        self.refresh_disk_index_file(&path).await;
                    } else if path.file_name().and_then(|n| n.to_str()) == Some("aivi.toml") {
                        // Project boundary changed; lazily rebuild on demand.
                        self.invalidate_disk_index_for_path(&path).await;
                    }
                }
                FileChangeType::DELETED => {
                    if path.extension().and_then(|e| e.to_str()) == Some("aivi") {
                        // Remove file modules from any existing disk index.
                        let Ok(uri) = Url::from_file_path(&path) else {
                            continue;
                        };
                        self.remove_from_disk_index(&uri).await;
                    } else {
                        self.invalidate_disk_index_for_path(&path).await;
                    }
                }
                _ => {}
            }
        }
        if affected_roots.is_empty() {
            return;
        }

        let (open_targets, include_specs_snippets, strict_level, strict, checkpoint) = {
            let state = self.state.lock().await;
            (
                state
                    .documents
                    .iter()
                    .map(|(uri, document)| DiagnosticTarget {
                        uri: uri.clone(),
                        version: Some(document.version),
                        text: document.text.clone(),
                        parse_diags: Some(document.parse_diags.clone()),
                    })
                    .collect::<Vec<_>>(),
                state.diagnostics_in_specs_snippets,
                state.strict.level,
                state.strict.clone(),
                state.typecheck_checkpoint.clone(),
            )
        };
        let current_snapshot = self.begin_diagnostics_snapshot().await;
        let diagnostics_started = Instant::now();
        let mut recheck_targets = Vec::new();

        for target in open_targets {
            let doc_path = PathBuf::from(Self::path_from_uri(&target.uri));
            let Some(root) = Self::project_root_for_path(&doc_path, &workspace_folders) else {
                continue;
            };
            if !affected_roots.contains(&root) {
                continue;
            }
            recheck_targets.push(target);
        }
        if recheck_targets.is_empty() {
            let mut state = self.state.lock().await;
            if state.diagnostics_snapshot == current_snapshot {
                state.pending_diagnostics = None;
            }
            return;
        }

        let workspace = {
            let mut merged = HashMap::new();
            for target in &recheck_targets {
                merged.extend(self.workspace_modules_for_diagnostics(&target.uri).await);
            }
            merged
        };
        let handle = self.spawn_diagnostics_publish_task(
            current_snapshot,
            "diagnostics.did_change_watched_files",
            diagnostics_started,
            None,
            format!(
                "roots={} strict={} docs={}",
                affected_roots.len(),
                strict_level as u8,
                recheck_targets.len()
            ),
            workspace,
            None,
            recheck_targets,
            include_specs_snippets,
            strict,
            checkpoint,
        );
        let mut state = self.state.lock().await;
        if state.diagnostics_snapshot == current_snapshot {
            state.pending_diagnostics = Some(handle.abort_handle());
        } else {
            handle.abort();
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let Some(text) = self
            .with_document_text(&uri, |content| content.to_string())
            .await
        else {
            return Ok(Some(DocumentSymbolResponse::Nested(Vec::new())));
        };
        let uri2 = uri.clone();
        let symbols =
            tokio::task::spawn_blocking(move || Self::build_document_symbols(&text, &uri2))
                .await
                .unwrap_or_default();
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let TextDocumentPositionParams {
            text_document,
            position,
        } = params.text_document_position_params;
        let uri = text_document.uri;
        let Some(text) = self
            .with_document_text(&uri, |content| content.to_string())
            .await
        else {
            return Ok(None);
        };
        let workspace = self.workspace_modules_for(&uri).await;
        let uri2 = uri.clone();
        let location = tokio::task::spawn_blocking(move || {
            Self::build_definition_with_workspace(&text, &uri2, position, &workspace)
        })
        .await
        .unwrap_or(None);
        Ok(location.map(|loc| GotoDefinitionResponse::Array(vec![loc])))
    }

    async fn goto_declaration(
        &self,
        params: GotoDeclarationParams,
    ) -> Result<Option<GotoDeclarationResponse>> {
        let TextDocumentPositionParams {
            text_document,
            position,
        } = params.text_document_position_params;
        let uri = text_document.uri;
        let Some(text) = self
            .with_document_text(&uri, |content| content.to_string())
            .await
        else {
            return Ok(None);
        };
        let workspace = self.workspace_modules_for(&uri).await;
        let uri2 = uri.clone();
        let location = tokio::task::spawn_blocking(move || {
            Self::build_definition_with_workspace(&text, &uri2, position, &workspace)
        })
        .await
        .unwrap_or(None);
        Ok(location.map(|loc| GotoDeclarationResponse::Array(vec![loc])))
    }

    async fn goto_implementation(
        &self,
        params: GotoImplementationParams,
    ) -> Result<Option<GotoImplementationResponse>> {
        let TextDocumentPositionParams {
            text_document,
            position,
        } = params.text_document_position_params;
        let uri = text_document.uri;
        let Some(text) = self
            .with_document_text(&uri, |content| content.to_string())
            .await
        else {
            return Ok(None);
        };
        let workspace = self.workspace_modules_for(&uri).await;
        let uri2 = uri.clone();
        let location = tokio::task::spawn_blocking(move || {
            Self::build_definition_with_workspace(&text, &uri2, position, &workspace)
        })
        .await
        .unwrap_or(None);
        Ok(location.map(|loc| GotoImplementationResponse::Array(vec![loc])))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let debug_hover = std::env::var_os("AIVI_LSP_DEBUG_HOVER").is_some();
        let started = Instant::now();
        let TextDocumentPositionParams {
            text_document,
            position,
        } = params.text_document_position_params;
        let uri = text_document.uri;
        let doc_index = { Arc::clone(&self.state.lock().await.doc_index) };
        let Some(text) = self
            .with_document_text(&uri, |content| content.to_string())
            .await
        else {
            if debug_hover {
                self.client
                    .log_message(
                        tower_lsp::lsp_types::MessageType::WARNING,
                        format!(
                            "[hover] missing open text for uri={uri}; elapsed={:?}",
                            started.elapsed()
                        ),
                    )
                    .await;
            }
            return Ok(None);
        };
        if debug_hover {
            self.client
                .log_message(
                    tower_lsp::lsp_types::MessageType::INFO,
                    format!(
                        "[hover] request uri={uri} position={}:{}",
                        position.line, position.character
                    ),
                )
                .await;
        }
        let workspace = self.workspace_modules_for(&uri).await;
        let workspace_len = workspace.len();
        let uri2 = uri.clone();
        let hover = tokio::task::spawn_blocking(move || {
            Self::build_hover_with_workspace(&text, &uri2, position, &workspace, doc_index.as_ref())
                .or_else(|| Self::build_hover(&text, &uri2, position, doc_index.as_ref()))
        })
        .await
        .unwrap_or(None);
        if debug_hover {
            self.client
                .log_message(
                    tower_lsp::lsp_types::MessageType::INFO,
                    format!(
                        "[hover] resolved={} workspace_modules={workspace_len} elapsed={:?}",
                        hover.is_some(),
                        started.elapsed()
                    ),
                )
                .await;
        }
        Ok(hover)
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let TextDocumentPositionParams {
            text_document,
            position,
        } = params.text_document_position_params;
        let uri = text_document.uri;
        let Some(text) = self
            .with_document_text(&uri, |content| content.to_string())
            .await
        else {
            return Ok(None);
        };
        let workspace = self.workspace_modules_for(&uri).await;
        let uri2 = uri.clone();
        let help = tokio::task::spawn_blocking(move || {
            Self::build_signature_help_with_workspace(&text, &uri2, position, &workspace)
        })
        .await
        .unwrap_or(None);
        Ok(help)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let TextDocumentPositionParams {
            text_document,
            position,
        } = params.text_document_position;
        let uri = text_document.uri;
        let include_declaration = params.context.include_declaration;
        let Some(text) = self
            .with_document_text(&uri, |content| content.to_string())
            .await
        else {
            return Ok(Some(Vec::new()));
        };
        let workspace = self.workspace_modules_for(&uri).await;
        let uri2 = uri.clone();
        let locations = tokio::task::spawn_blocking(move || {
            Self::build_references_with_workspace(
                &text,
                &uri2,
                position,
                include_declaration,
                &workspace,
            )
        })
        .await
        .unwrap_or_default();
        Ok(Some(locations))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = params.new_name;
        let Some(text) = self
            .with_document_text(&uri, |content| content.to_string())
            .await
        else {
            return Ok(None);
        };
        let workspace = self.workspace_modules_for(&uri).await;
        let uri2 = uri.clone();
        let edit = tokio::task::spawn_blocking(move || {
            Self::build_rename_with_workspace(&text, &uri2, position, &new_name, &workspace)
        })
        .await
        .unwrap_or(None);
        Ok(edit)
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let position = params.position;
        let Some(text) = self
            .with_document_text(&uri, |content| content.to_string())
            .await
        else {
            return Ok(None);
        };
        let workspace = self.workspace_modules_for(&uri).await;
        let uri2 = uri.clone();
        let response = match tokio::task::spawn_blocking(move || {
            Self::prepare_rename_with_workspace(&text, &uri2, position, &workspace)
        })
        .await
        {
            Ok(response) => response,
            Err(err) => {
                self.log_join_error("prepare_rename", &err).await;
                None
            }
        };
        Ok(response)
    }

    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> Result<Option<Vec<CodeActionOrCommand>>> {
        let uri = params.text_document.uri;
        let diagnostics = params.context.diagnostics;
        let cursor_range = params.range;
        let Some(text) = self
            .with_document_text(&uri, |content| content.to_string())
            .await
        else {
            return Ok(Some(Vec::new()));
        };
        let workspace = self.workspace_modules_for(&uri).await;
        let uri2 = uri.clone();
        let actions = tokio::task::spawn_blocking(move || {
            Self::build_code_actions_with_workspace(
                &text,
                &uri2,
                &diagnostics,
                &workspace,
                cursor_range,
            )
        })
        .await
        .unwrap_or_default();
        Ok(Some(actions))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let Some(source) = self
            .with_document_text(&uri, |content| content.to_string())
            .await
        else {
            return Ok(None);
        };
        let (mut options, from_config) = {
            let state = self.state.lock().await;
            (state.format_options, state.format_options_from_config)
        };
        if !from_config {
            options.indent_size = params.options.tab_size as usize;
        }
        let edits =
            tokio::task::spawn_blocking(move || Backend::build_formatting_edits(&source, options))
                .await
                .map_err(|e| tower_lsp::jsonrpc::Error {
                    code: tower_lsp::jsonrpc::ErrorCode::InternalError,
                    message: format!("formatting task failed: {e}").into(),
                    data: None,
                })?;
        Ok(Some(edits))
    }

    async fn range_formatting(
        &self,
        params: DocumentRangeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        // AIVI formatting is currently whole-document. Advertising range formatting while
        // ignoring the provided range is surprising; until we have a range-aware formatter,
        // return no edits and don't advertise range formatting capability.
        let _ = params;
        Ok(None)
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let Some(text) = self
            .with_document_text(&uri, |content| content.to_string())
            .await
        else {
            return Ok(None);
        };
        let tokens = tokio::task::spawn_blocking(move || Self::build_semantic_tokens(&text))
            .await
            .ok();
        Ok(tokens.map(SemanticTokensResult::Tokens))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let uri_display = uri.to_string();
        let Some(text) = self
            .with_document_text(&uri, |content| content.to_string())
            .await
        else {
            return Ok(Some(CompletionResponse::Array(Vec::new())));
        };
        let workspace = self.workspace_modules_for(&uri).await;
        let gtk_index = { Arc::clone(&self.state.lock().await.gtk_index) };
        let uri2 = uri.clone();
        let completion_started = Instant::now();
        let items = match tokio::task::spawn_blocking(move || {
            Self::build_completion_items(&text, &uri2, position, &workspace, &gtk_index)
        })
        .await
        {
            Ok(items) => items,
            Err(err) => {
                self.log_join_error("completion", &err).await;
                Vec::new()
            }
        };
        let item_count = items.len();
        self.log_telemetry(
            "completion",
            completion_started.elapsed(),
            format!("uri={uri_display} count={item_count}"),
        )
        .await;
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn completion_resolve(&self, item: CompletionItem) -> Result<CompletionItem> {
        let doc_index = { Arc::clone(&self.state.lock().await.doc_index) };
        let resolved =
            tokio::task::spawn_blocking(move || Self::resolve_completion_item(item, &doc_index))
                .await
                .unwrap_or_else(|_| CompletionItem::default());
        Ok(resolved)
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let uri = params.text_document.uri;
        let Some(text) = self
            .with_document_text(&uri, |content| content.to_string())
            .await
        else {
            return Ok(Some(Vec::new()));
        };
        let uri2 = uri.clone();
        let ranges = tokio::task::spawn_blocking(move || Self::build_folding_ranges(&text, &uri2))
            .await
            .unwrap_or_default();
        Ok(Some(ranges))
    }

    async fn selection_range(
        &self,
        params: SelectionRangeParams,
    ) -> Result<Option<Vec<SelectionRange>>> {
        let uri = params.text_document.uri;
        let positions = params.positions;
        let Some(text) = self
            .with_document_text(&uri, |content| content.to_string())
            .await
        else {
            return Ok(Some(Vec::new()));
        };
        let uri2 = uri.clone();
        let ranges = tokio::task::spawn_blocking(move || {
            Self::build_selection_ranges(&text, &uri2, &positions)
        })
        .await
        .unwrap_or_default();
        Ok(Some(ranges))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri;
        let range = params.range;
        let Some(text) = self
            .with_document_text(&uri, |content| content.to_string())
            .await
        else {
            return Ok(Some(Vec::new()));
        };
        let workspace = self.workspace_modules_for(&uri).await;
        let uri2 = uri.clone();
        let hints = tokio::task::spawn_blocking(move || {
            Self::build_inlay_hints(&text, &uri2, range, &workspace)
        })
        .await
        .unwrap_or_default();
        Ok(Some(hints))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let query = params.query;
        let modules: Vec<_> = {
            let state = self.state.lock().await;
            let mut all = Vec::new();
            // Collect from open documents.
            for indexed in state.open_module_index.values() {
                all.push(indexed.clone());
            }
            // Collect from disk indexes.
            for disk_index in state.disk_indexes.values() {
                for indexed in disk_index.module_index.values() {
                    if !all.iter().any(|m: &crate::state::IndexedModule| {
                        m.module.name.name == indexed.module.name.name
                    }) {
                        all.push(indexed.clone());
                    }
                }
            }
            all
        };
        let symbols =
            tokio::task::spawn_blocking(move || Self::build_workspace_symbols(&query, &modules))
                .await
                .unwrap_or_default();
        Ok(Some(symbols))
    }
}

pub async fn run() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend {
        client,
        state: Arc::new(Mutex::new(BackendState::default())),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
