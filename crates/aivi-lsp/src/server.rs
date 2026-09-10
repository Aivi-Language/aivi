use std::{sync::Arc, time::Duration};

use tower_lsp::{
    Client, LanguageServer,
    jsonrpc::{Error, Result},
    lsp_types::request::{GotoImplementationParams, GotoImplementationResponse},
    lsp_types::{
        CodeActionOptions, CodeActionParams, CodeActionProviderCapability, CodeLens,
        CodeLensOptions, CodeLensParams, CompletionOptions, CompletionParams, CompletionResponse,
        DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
        DocumentFormattingParams, DocumentHighlight, DocumentHighlightParams, DocumentSymbolParams,
        DocumentSymbolResponse, FoldingRange, FoldingRangeParams, FoldingRangeProviderCapability,
        GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability,
        ImplementationProviderCapability, InitializeParams, InitializeResult, InitializedParams,
        InlayHint, InlayHintParams, Location, MessageType, OneOf, PrepareRenameResponse,
        ReferenceParams, RenameOptions, RenameParams, SemanticTokensDeltaParams,
        SemanticTokensFullDeltaResult, SemanticTokensFullOptions, SemanticTokensLegend,
        SemanticTokensOptions, SemanticTokensParams, SemanticTokensRangeParams,
        SemanticTokensRangeResult, SemanticTokensResult, SemanticTokensServerCapabilities,
        ServerCapabilities, SignatureHelp, SignatureHelpOptions, SignatureHelpParams,
        SymbolInformation, SymbolKind, TextDocumentPositionParams, TextDocumentSyncCapability,
        TextDocumentSyncKind, TextDocumentSyncOptions, TextEdit, WorkDoneProgressOptions,
        WorkspaceEdit, WorkspaceSymbolParams,
    },
};

use crate::{
    analysis_pool::{AnalysisPoolError, CancellationToken},
    state::{DocumentSnapshot, ServerConfig, ServerState},
};

pub struct Backend {
    pub client: Client,
    pub state: Arc<ServerState>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            state: Arc::new(ServerState::new()),
        }
    }

    fn schedule_diagnostics(&self, uri: tower_lsp::lsp_types::Url, debounce: Duration) {
        let Some(snapshot) = self.state.document_snapshot(&uri) else {
            tracing::error!(
                "schedule_diagnostics: URI {} is not tracked; diagnostics will not be published",
                uri
            );
            return;
        };

        let Some((request_id, cancellation, task_permit)) = self.state.start_diagnostics(&uri)
        else {
            return;
        };
        let state = Arc::clone(&self.state);
        let client = self.client.clone();
        tokio::spawn(async move {
            let _task_permit = task_permit;
            run_diagnostics_request(
                Arc::clone(&state),
                client,
                uri.clone(),
                snapshot,
                cancellation,
                debounce,
            )
            .await;
            state.finish_diagnostics(&uri, request_id);
        });
    }

    fn schedule_project_diagnostics(&self, debounce: Duration) {
        for (uri, _) in self.state.open_files() {
            self.schedule_diagnostics(uri, debounce);
        }
    }

    async fn update<T, F>(&self, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&ServerState) -> T + Send + 'static,
    {
        let state = Arc::clone(&self.state);
        self.state
            .analysis_pool
            .execute(CancellationToken::default(), move || {
                let _lease = state.analysis_access.blocking_write();
                work(&state)
            })
            .await
            .map_err(|error| {
                tracing::error!(?error, "workspace update failed");
                Error::internal_error()
            })
    }

    async fn analyze<T, F>(&self, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(Arc<ServerState>) -> T + Send + 'static,
    {
        let cancellation = CancellationToken::default();
        let cancel_on_drop = CancelOnDrop::new(cancellation.clone());
        let worker_cancellation = cancellation.clone();
        let state = Arc::clone(&self.state);
        let analysis_access = Arc::clone(&state.analysis_access);
        let result = self
            .state
            .analysis_pool
            .execute(cancellation, move || {
                let _analysis = analysis_access.blocking_read();
                (!worker_cancellation.is_cancelled()).then(|| work(state))
            })
            .await;
        cancel_on_drop.disarm();

        match result {
            Ok(Some(value)) => Ok(value),
            Ok(None) | Err(AnalysisPoolError::Cancelled) => Err(Error::request_cancelled()),
            Err(error) => {
                tracing::error!(?error, "LSP request analysis failed");
                Err(Error::internal_error())
            }
        }
    }
}

struct CancelOnDrop {
    cancellation: Option<CancellationToken>,
}

impl CancelOnDrop {
    fn new(cancellation: CancellationToken) -> Self {
        Self {
            cancellation: Some(cancellation),
        }
    }

    fn disarm(mut self) {
        self.cancellation.take();
    }
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if let Some(cancellation) = self.cancellation.take() {
            cancellation.cancel();
        }
    }
}

async fn run_diagnostics_request(
    state: Arc<ServerState>,
    client: Client,
    uri: tower_lsp::lsp_types::Url,
    snapshot: DocumentSnapshot,
    cancellation: CancellationToken,
    debounce: Duration,
) {
    if !debounce.is_zero() {
        tokio::select! {
            () = tokio::time::sleep(debounce) => {}
            () = cancellation.cancelled() => return,
        }
    }

    let analysis_state = Arc::clone(&state);
    let analysis_access = Arc::clone(&state.analysis_access);
    let analysis_uri = uri.clone();
    let analysis_file = snapshot.file;
    let analysis_snapshot = snapshot.clone();
    let worker_cancellation = cancellation.clone();
    let result = state
        .analysis_pool
        .execute(cancellation.clone(), move || {
            let _analysis_access = analysis_access.blocking_read();
            if worker_cancellation.is_cancelled()
                || !analysis_state.document_is_current(&analysis_uri, &analysis_snapshot)
            {
                None
            } else {
                Some(crate::diagnostics::collect_lsp_diagnostics(
                    &analysis_state.db,
                    analysis_file,
                    &analysis_uri,
                ))
            }
        })
        .await;
    let diagnostics = match result {
        Ok(Some(diagnostics)) => diagnostics,
        Ok(None) => return,
        Err(AnalysisPoolError::Cancelled) => return,
        Err(error) => {
            tracing::error!(?error, %uri, "diagnostic analysis failed");
            return;
        }
    };

    // Serialize the final version check with document notifications. This
    // closes the check/publish race without keeping a document-map guard over
    // an await. The version travels with the notification as a second client-
    // side stale-result defense.
    let _publication = state.diagnostic_publication.lock().await;
    if cancellation.is_cancelled()
        || !state.diagnostics_are_accepting()
        || !state.document_is_current(&uri, &snapshot)
    {
        return;
    }
    client
        .publish_diagnostics(uri.clone(), diagnostics, Some(snapshot.version))
        .await;
    tracing::debug!(
        version = snapshot.version,
        "published diagnostics for {uri}"
    );
}

fn server_capabilities(config: ServerConfig) -> ServerCapabilities {
    ServerCapabilities {
        workspace: Some(tower_lsp::lsp_types::WorkspaceServerCapabilities {
            workspace_folders: Some(tower_lsp::lsp_types::WorkspaceFoldersServerCapabilities {
                supported: Some(true),
                change_notifications: Some(OneOf::Left(true)),
            }),
            ..Default::default()
        }),
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::INCREMENTAL),
                ..Default::default()
            },
        )),
        document_symbol_provider: Some(OneOf::Left(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        completion_provider: Some(CompletionOptions {
            resolve_provider: Some(false),
            trigger_characters: Some(vec![".".to_owned()]),
            ..Default::default()
        }),
        signature_help_provider: Some(SignatureHelpOptions {
            trigger_characters: Some(vec![" ".to_owned(), "(".to_owned()]),
            retrigger_characters: Some(vec![" ".to_owned()]),
            work_done_progress_options: WorkDoneProgressOptions::default(),
        }),
        definition_provider: Some(OneOf::Left(true)),
        implementation_provider: Some(ImplementationProviderCapability::Simple(true)),
        references_provider: Some(OneOf::Left(true)),
        document_highlight_provider: Some(OneOf::Left(true)),
        folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: WorkDoneProgressOptions::default(),
        })),
        inlay_hint_provider: config.inlay_hints_enabled.then_some(OneOf::Left(true)),
        code_action_provider: Some(CodeActionProviderCapability::Options(
            CodeActionOptions::default(),
        )),
        workspace_symbol_provider: Some(OneOf::Left(true)),
        code_lens_provider: config.code_lens_enabled.then_some(CodeLensOptions {
            resolve_provider: Some(false),
        }),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                work_done_progress_options: WorkDoneProgressOptions::default(),
                legend: SemanticTokensLegend {
                    token_types: crate::semantic_tokens::TOKEN_TYPES.to_vec(),
                    token_modifiers: Vec::new(),
                },
                range: Some(true),
                full: Some(SemanticTokensFullOptions::Delta { delta: Some(true) }),
            },
        )),
        ..Default::default()
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let config = ServerConfig::from_initialization_options(params.initialization_options);
        self.state.set_config(config);
        #[allow(deprecated)]
        let roots = params
            .workspace_folders
            .map(|folders| {
                folders
                    .into_iter()
                    .map(|folder| folder.uri)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| params.root_uri.into_iter().collect())
            .into_iter()
            .filter_map(|uri| uri.to_file_path().ok())
            .collect();
        self.update(move |state| {
            state.set_workspace_roots(roots);
            state.refresh_workspace_files();
        })
        .await?;

        Ok(InitializeResult {
            capabilities: server_capabilities(config),
            ..Default::default()
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "aivi language server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        self.state.shutdown_diagnostics().await;
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        let text = params.text_document.text;
        let _publication = self.state.diagnostic_publication.lock().await;
        let result = self
            .update(move |state| {
                crate::documents::open_document(state, &uri, version, text);
                state.refresh_workspace_files();
            })
            .await;
        if result.is_ok() {
            self.schedule_project_diagnostics(Duration::ZERO);
        }
        drop(_publication);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let _publication = self.state.diagnostic_publication.lock().await;
        let result = self
            .update(move |state| {
                crate::documents::change_document(
                    state,
                    &params.text_document.uri,
                    params.text_document.version,
                    &params.content_changes,
                )
            })
            .await;
        match result {
            Ok(Ok(_)) => self.schedule_project_diagnostics(Duration::from_millis(
                self.state.config().diagnostics_debounce_ms,
            )),
            result => tracing::warn!(?result, "rejected document change"),
        }
        drop(_publication);
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        let _publication = self.state.diagnostic_publication.lock().await;
        let closed_uri = uri.clone();
        let version = self
            .update(move |state| {
                state.cancel_diagnostics(&closed_uri);
                let version = crate::documents::close_document(state, &closed_uri)
                    .map(|document| document.version);
                state.refresh_workspace_files();
                version
            })
            .await
            .ok()
            .flatten();
        self.client
            .publish_diagnostics(uri, Vec::new(), version)
            .await;
        self.schedule_project_diagnostics(Duration::ZERO);
        drop(_publication);
    }

    async fn did_change_watched_files(
        &self,
        _params: tower_lsp::lsp_types::DidChangeWatchedFilesParams,
    ) {
        let _publication = self.state.diagnostic_publication.lock().await;
        let result = self.update(ServerState::refresh_workspace_files).await;
        if result.is_ok() {
            self.schedule_project_diagnostics(Duration::ZERO);
        }
        drop(_publication);
    }

    async fn did_change_workspace_folders(
        &self,
        params: tower_lsp::lsp_types::DidChangeWorkspaceFoldersParams,
    ) {
        let _publication = self.state.diagnostic_publication.lock().await;
        let result = self
            .update(move |state| {
                let mut roots = state
                    .workspace_roots
                    .read()
                    .expect("workspace roots lock")
                    .clone();
                for folder in params.event.removed {
                    if let Ok(path) = folder.uri.to_file_path() {
                        roots.retain(|root| root != &path);
                    }
                }
                roots.extend(
                    params
                        .event
                        .added
                        .into_iter()
                        .filter_map(|folder| folder.uri.to_file_path().ok()),
                );
                state.set_workspace_roots(roots);
                state.refresh_workspace_files();
            })
            .await;
        if result.is_ok() {
            self.schedule_project_diagnostics(Duration::ZERO);
        }
        drop(_publication);
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        self.analyze(move |state| {
            let file = state.file(&params.text_document.uri)?;
            let analysis = crate::analysis::FileAnalysis::load(&state.db, file);
            let symbols = crate::symbols::convert_symbols(
                analysis.symbols.as_ref(),
                analysis.source.as_ref(),
            );
            Some(DocumentSymbolResponse::Nested(symbols))
        })
        .await
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        self.analyze(move |state| {
            let file = state.file(&params.text_document.uri)?;
            crate::formatting::format_document(&state.db, file)
        })
        .await
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        self.analyze(move |state| crate::hover::hover(params, state))
            .await
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        self.analyze(move |state| crate::completion::completion(params, state))
            .await
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        self.analyze(move |state| crate::signature_help::signature_help(params, state))
            .await
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        self.analyze(move |state| crate::definition::definition(params, state))
            .await
    }

    async fn goto_implementation(
        &self,
        params: GotoImplementationParams,
    ) -> Result<Option<GotoImplementationResponse>> {
        self.analyze(move |state| crate::implementation::implementation(params, state))
            .await
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        self.analyze(move |state| crate::references::references(params, state))
            .await
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        self.analyze(move |state| crate::document_highlights::document_highlights(params, state))
            .await
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        self.analyze(move |state| crate::folding_ranges::folding_ranges(params, state))
            .await
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        self.analyze(move |state| crate::rename::prepare_rename(params, state))
            .await
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        self.analyze(move |state| crate::rename::rename(params, state)).await?
            .map(Some).ok_or_else(|| Error::invalid_params("rename requires one project-owned symbol, an identifier fresh in every affected module, and unaliased references"))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        self.analyze(move |state| crate::inlay_hints::inlay_hints(params, state))
            .await
    }

    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> Result<Option<tower_lsp::lsp_types::CodeActionResponse>> {
        self.analyze(move |state| crate::code_actions::code_actions(params, state))
            .await
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        self.analyze(move |state| {
            let query = params.query.to_ascii_lowercase();
            let workspace = state.workspace_index.snapshot(&state);
            let results = workspace
                .symbols()
                .iter()
                .filter(|symbol| query.is_empty() || symbol.normalized_name.contains(&query))
                .map(|symbol| {
                    #[allow(deprecated)]
                    SymbolInformation {
                        name: symbol.name.clone(),
                        kind: aivi_lsp_kind_to_symbol_kind(symbol.kind),
                        tags: None,
                        deprecated: None,
                        location: symbol.location.clone(),
                        container_name: symbol.container_name.clone(),
                    }
                })
                .collect::<Vec<_>>();

            Some(results)
        })
        .await
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        self.analyze(move |state| crate::semantic_tokens::semantic_tokens_full(params, state))
            .await
    }

    async fn semantic_tokens_full_delta(
        &self,
        params: SemanticTokensDeltaParams,
    ) -> Result<Option<SemanticTokensFullDeltaResult>> {
        self.analyze(move |state| crate::semantic_tokens::semantic_tokens_full_delta(params, state))
            .await
    }

    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        self.analyze(move |state| crate::semantic_tokens::semantic_tokens_range(params, state))
            .await
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        self.analyze(move |state| {
            if !state.config().code_lens_enabled {
                return None;
            }

            let uri = &params.text_document.uri;
            let file = state.file(uri)?;
            let hir = aivi_query::hir_module(&state.db, file);
            let lenses = crate::code_lens::collect_code_lenses(hir.module(), hir.source(), uri);
            (!lenses.is_empty()).then_some(lenses)
        })
        .await
    }
}

fn aivi_lsp_kind_to_symbol_kind(kind: aivi_hir::LspSymbolKind) -> SymbolKind {
    match kind {
        aivi_hir::LspSymbolKind::File => SymbolKind::FILE,
        aivi_hir::LspSymbolKind::Module => SymbolKind::MODULE,
        aivi_hir::LspSymbolKind::Namespace => SymbolKind::NAMESPACE,
        aivi_hir::LspSymbolKind::Package => SymbolKind::PACKAGE,
        aivi_hir::LspSymbolKind::Class => SymbolKind::CLASS,
        aivi_hir::LspSymbolKind::Method => SymbolKind::METHOD,
        aivi_hir::LspSymbolKind::Property => SymbolKind::PROPERTY,
        aivi_hir::LspSymbolKind::Field => SymbolKind::FIELD,
        aivi_hir::LspSymbolKind::Constructor => SymbolKind::CONSTRUCTOR,
        aivi_hir::LspSymbolKind::Enum => SymbolKind::ENUM,
        aivi_hir::LspSymbolKind::Interface => SymbolKind::INTERFACE,
        aivi_hir::LspSymbolKind::Function => SymbolKind::FUNCTION,
        aivi_hir::LspSymbolKind::Variable => SymbolKind::VARIABLE,
        aivi_hir::LspSymbolKind::Constant => SymbolKind::CONSTANT,
        aivi_hir::LspSymbolKind::String => SymbolKind::STRING,
        aivi_hir::LspSymbolKind::Number => SymbolKind::NUMBER,
        aivi_hir::LspSymbolKind::Boolean => SymbolKind::BOOLEAN,
        aivi_hir::LspSymbolKind::Array => SymbolKind::ARRAY,
        aivi_hir::LspSymbolKind::Object => SymbolKind::OBJECT,
        aivi_hir::LspSymbolKind::Key => SymbolKind::KEY,
        aivi_hir::LspSymbolKind::Null => SymbolKind::NULL,
        aivi_hir::LspSymbolKind::EnumMember => SymbolKind::ENUM_MEMBER,
        aivi_hir::LspSymbolKind::Struct => SymbolKind::STRUCT,
        aivi_hir::LspSymbolKind::Event => SymbolKind::EVENT,
        aivi_hir::LspSymbolKind::Operator => SymbolKind::OPERATOR,
        aivi_hir::LspSymbolKind::TypeParameter => SymbolKind::TYPE_PARAMETER,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use futures::StreamExt;
    use serde_json::json;
    use tower::{Service, ServiceExt};
    use tower_lsp::lsp_types::{
        FoldingRangeProviderCapability, PublishDiagnosticsParams, SemanticTokensFullOptions,
        SemanticTokensServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
        TextDocumentSyncOptions, Url,
    };
    use tower_lsp::{LspService, jsonrpc::Request};

    use super::{Backend, server_capabilities};
    use crate::state::ServerConfig;

    #[test]
    fn advertises_incremental_document_synchronization() {
        let capabilities = server_capabilities(ServerConfig::default());
        let Some(TextDocumentSyncCapability::Options(TextDocumentSyncOptions {
            change: Some(change),
            ..
        })) = capabilities.text_document_sync
        else {
            panic!("server should advertise text document sync options");
        };

        assert_eq!(change, TextDocumentSyncKind::INCREMENTAL);
    }

    #[test]
    fn advertises_the_complete_protocol_quality_surface() {
        let capabilities = server_capabilities(ServerConfig::default());
        assert!(capabilities.signature_help_provider.is_some());
        assert!(capabilities.document_highlight_provider.is_some());
        assert!(matches!(
            capabilities.folding_range_provider,
            Some(FoldingRangeProviderCapability::Simple(true))
        ));
        let Some(SemanticTokensServerCapabilities::SemanticTokensOptions(options)) =
            capabilities.semantic_tokens_provider
        else {
            panic!("server should advertise semantic token options");
        };
        assert_eq!(options.range, Some(true));
        assert!(matches!(
            options.full,
            Some(SemanticTokensFullOptions::Delta { delta: Some(true) })
        ));
    }

    #[tokio::test]
    async fn queued_request_does_not_starve_tokio_and_drop_cancels_its_work() {
        let (service, _) = LspService::new(Backend::new);
        let state = Arc::clone(&service.inner().state);
        let write_access = state.analysis_access.write().await;
        let work_ran = Arc::new(AtomicBool::new(false));
        let work_flag = Arc::clone(&work_ran);
        let mut request = Box::pin(service.inner().analyze(move |_| {
            work_flag.store(true, Ordering::Release);
        }));

        tokio::select! {
            result = &mut request => panic!("analysis unexpectedly completed: {result:?}"),
            () = tokio::time::sleep(Duration::from_millis(25)) => {}
        }
        drop(request);
        drop(write_access);

        tokio::time::sleep(Duration::from_millis(25)).await;
        assert!(
            !work_ran.load(Ordering::Acquire),
            "dropping the request future must cancel work queued behind the query writer"
        );
    }

    #[tokio::test]
    async fn rapid_edits_publish_only_the_latest_debounced_version() {
        let (mut service, mut client_messages) = LspService::new(Backend::new);
        let uri = Url::from_file_path(PathBuf::from("/server-tests/rapid.aivi"))
            .expect("test URI should be valid");

        service
            .ready()
            .await
            .expect("service should initialize")
            .call(
                Request::build("initialize")
                    .params(json!({
                        "capabilities": {},
                        "initializationOptions": { "diagnosticsDebounceMs": 50 }
                    }))
                    .id(1)
                    .finish(),
            )
            .await
            .expect("initialize request should succeed");
        service
            .ready()
            .await
            .expect("service should accept didOpen")
            .call(
                Request::build("textDocument/didOpen")
                    .params(json!({
                        "textDocument": {
                            "uri": uri,
                            "languageId": "aivi",
                            "version": 1,
                            "text": "value answer = 42\n"
                        }
                    }))
                    .finish(),
            )
            .await
            .expect("didOpen notification should succeed");

        let initial = tokio::time::timeout(Duration::from_secs(2), client_messages.next())
            .await
            .expect("initial diagnostics should not starve the runtime")
            .expect("client channel should stay open");
        let initial: PublishDiagnosticsParams = serde_json::from_value(
            initial
                .params()
                .expect("diagnostic notification should have params")
                .clone(),
        )
        .expect("diagnostic params should deserialize");
        assert_eq!(initial.version, Some(1));

        for (version, text) in [(2, "invalid ="), (3, "value latest = 3\n")] {
            service
                .ready()
                .await
                .expect("service should accept didChange")
                .call(
                    Request::build("textDocument/didChange")
                        .params(json!({
                            "textDocument": { "uri": uri, "version": version },
                            "contentChanges": [{ "text": text }]
                        }))
                        .finish(),
                )
                .await
                .expect("didChange notification should succeed");
        }

        let latest = tokio::time::timeout(Duration::from_secs(2), client_messages.next())
            .await
            .expect("latest diagnostics should not starve the runtime")
            .expect("client channel should stay open");
        assert_eq!(latest.method(), "textDocument/publishDiagnostics");
        let latest: PublishDiagnosticsParams = serde_json::from_value(
            latest
                .params()
                .expect("diagnostic notification should have params")
                .clone(),
        )
        .expect("diagnostic params should deserialize");
        assert_eq!(latest.version, Some(3));

        assert!(
            tokio::time::timeout(Duration::from_millis(150), client_messages.next())
                .await
                .is_err(),
            "superseded version 2 diagnostics must not be published later"
        );
    }

    #[tokio::test]
    async fn shutdown_cancels_debounced_diagnostics_before_returning() {
        let (mut service, mut client_messages) = LspService::new(Backend::new);
        let state = Arc::clone(&service.inner().state);
        let uri = Url::from_file_path(PathBuf::from("/server-tests/shutdown.aivi"))
            .expect("test URI should be valid");

        service
            .ready()
            .await
            .expect("service should initialize")
            .call(
                Request::build("initialize")
                    .params(json!({
                        "capabilities": {},
                        "initializationOptions": { "diagnosticsDebounceMs": 50 }
                    }))
                    .id(1)
                    .finish(),
            )
            .await
            .expect("initialize request should succeed");
        service
            .ready()
            .await
            .expect("service should accept didOpen")
            .call(
                Request::build("textDocument/didOpen")
                    .params(json!({
                        "textDocument": {
                            "uri": uri,
                            "languageId": "aivi",
                            "version": 1,
                            "text": "value answer = 42\n"
                        }
                    }))
                    .finish(),
            )
            .await
            .expect("didOpen notification should succeed");
        tokio::time::timeout(Duration::from_secs(2), client_messages.next())
            .await
            .expect("initial diagnostics should be published")
            .expect("client channel should stay open");

        service
            .ready()
            .await
            .expect("service should accept didChange")
            .call(
                Request::build("textDocument/didChange")
                    .params(json!({
                        "textDocument": { "uri": uri, "version": 2 },
                        "contentChanges": [{ "text": "invalid =" }]
                    }))
                    .finish(),
            )
            .await
            .expect("didChange notification should succeed");

        tokio::time::timeout(
            Duration::from_secs(2),
            tower_lsp::LanguageServer::shutdown(service.inner()),
        )
        .await
        .expect("shutdown should drain diagnostic ownership")
        .expect("shutdown should succeed");

        assert!(!state.diagnostics_are_accepting());
        assert!(state.start_diagnostics(&uri).is_none());
        assert!(
            tokio::time::timeout(Duration::from_millis(150), client_messages.next())
                .await
                .is_err(),
            "cancelled version 2 diagnostics must not publish after shutdown"
        );
    }
}
