use std::sync::Arc;

use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::request::{
    GotoDeclarationParams, GotoDeclarationResponse, GotoImplementationParams,
    GotoImplementationResponse,
};
use tower_lsp::lsp_types::{
    CodeActionOrCommand, CodeActionParams, CompletionParams, CompletionResponse,
    DeclarationCapability, DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverProviderCapability, HoverParams,
    ImplementationProviderCapability, InitializeParams, InitializeResult, InitializedParams,
    Location, OneOf, ReferenceParams, RenameParams, SemanticTokensFullOptions,
    SemanticTokensOptions, SemanticTokensParams, SemanticTokensResult,
    SemanticTokensServerCapabilities, ServerCapabilities, SignatureHelp, SignatureHelpOptions,
    SignatureHelpParams, TextDocumentPositionParams, TextDocumentSyncCapability,
    TextDocumentSyncKind, WorkspaceEdit,
};
use tower_lsp::{LanguageServer, LspService, Server};

use crate::backend::Backend;
use crate::state::BackendState;

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let root = params
            .root_uri
            .and_then(|uri| uri.to_file_path().ok())
            .or_else(|| {
                params
                    .workspace_folders
                    .as_ref()
                    .and_then(|folders| folders.first())
                    .and_then(|folder| folder.uri.to_file_path().ok())
            });
        if let Some(root) = root.clone() {
            let indexed = Self::build_workspace_index(&root);
            let mut state = self.state.lock().await;
            state.workspace_root = Some(root.clone());
            state.disk_index_root = Some(root);
            state.disk_module_index = indexed;
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                document_symbol_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                declaration_provider: Some(DeclarationCapability::Simple(true)),
                implementation_provider: Some(ImplementationProviderCapability::Simple(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec![" ".to_string()]),
                    retrigger_characters: None,
                    work_done_progress_options: Default::default(),
                }),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
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
                code_action_provider: Some(tower_lsp::lsp_types::CodeActionProviderCapability::Simple(
                    true,
                )),
                completion_provider: Some(tower_lsp::lsp_types::CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: None,
                    ..tower_lsp::lsp_types::CompletionOptions::default()
                }),
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

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: tower_lsp::lsp_types::DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let version = params.text_document.version;
        self.update_document(uri.clone(), text).await;
        if let Some(diagnostics) = self
            .with_document_text(&uri, |content| Self::build_diagnostics(content, &uri))
            .await
        {
            self.client
                .publish_diagnostics(uri, diagnostics, Some(version))
                .await;
        }
    }

    async fn did_change(&self, params: tower_lsp::lsp_types::DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        if let Some(change) = params.content_changes.into_iter().next() {
            self.update_document(uri.clone(), change.text).await;
            if let Some(diagnostics) = self
                .with_document_text(&uri, |content| Self::build_diagnostics(content, &uri))
                .await
            {
                self.client
                    .publish_diagnostics(uri, diagnostics, Some(version))
                    .await;
            }
        }
    }

    async fn did_close(&self, params: tower_lsp::lsp_types::DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.remove_document(&uri).await;
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let symbols = self
            .with_document_text(&uri, |content| Self::build_document_symbols(content, &uri))
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
        let location = match self
            .with_document_text(&uri, |content| content.to_string())
            .await
        {
            Some(text) => {
                let workspace = self.workspace_modules_for(&uri).await;
                Self::build_definition_with_workspace(&text, &uri, position, &workspace)
            }
            None => None,
        };
        Ok(location.map(GotoDefinitionResponse::Scalar))
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
        let location = match self
            .with_document_text(&uri, |content| content.to_string())
            .await
        {
            Some(text) => {
                let workspace = self.workspace_modules_for(&uri).await;
                Self::build_definition_with_workspace(&text, &uri, position, &workspace)
            }
            None => None,
        };
        Ok(location.map(GotoDeclarationResponse::Scalar))
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
        let location = match self
            .with_document_text(&uri, |content| content.to_string())
            .await
        {
            Some(text) => {
                let workspace = self.workspace_modules_for(&uri).await;
                Self::build_definition_with_workspace(&text, &uri, position, &workspace)
            }
            None => None,
        };
        Ok(location.map(GotoImplementationResponse::Scalar))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let TextDocumentPositionParams {
            text_document,
            position,
        } = params.text_document_position_params;
        let uri = text_document.uri;
        let hover = match self
            .with_document_text(&uri, |content| content.to_string())
            .await
        {
            Some(text) => {
                let workspace = self.workspace_modules_for(&uri).await;
                Self::build_hover_with_workspace(&text, &uri, position, &workspace)
                    .or_else(|| Self::build_hover(&text, &uri, position))
            }
            None => None,
        };
        Ok(hover)
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let TextDocumentPositionParams {
            text_document,
            position,
        } = params.text_document_position_params;
        let uri = text_document.uri;
        let help = match self
            .with_document_text(&uri, |content| content.to_string())
            .await
        {
            Some(text) => {
                let workspace = self.workspace_modules_for(&uri).await;
                Self::build_signature_help_with_workspace(&text, &uri, position, &workspace)
            }
            None => None,
        };
        Ok(help)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let TextDocumentPositionParams {
            text_document,
            position,
        } = params.text_document_position;
        let uri = text_document.uri;
        let include_declaration = params.context.include_declaration;
        let locations = match self
            .with_document_text(&uri, |content| content.to_string())
            .await
        {
            Some(text) => {
                let workspace = self.workspace_modules_for(&uri).await;
                Self::build_references_with_workspace(
                    &text,
                    &uri,
                    position,
                    include_declaration,
                    &workspace,
                )
            }
            None => Vec::new(),
        };
        Ok(Some(locations))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = params.new_name;
        let edit = match self
            .with_document_text(&uri, |content| content.to_string())
            .await
        {
            Some(text) => {
                let workspace = self.workspace_modules_for(&uri).await;
                Self::build_rename_with_workspace(&text, &uri, position, &new_name, &workspace)
            }
            None => None,
        };
        Ok(edit)
    }

    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> Result<Option<Vec<CodeActionOrCommand>>> {
        let uri = params.text_document.uri;
        let diagnostics = params.context.diagnostics;
        let actions = self
            .with_document_text(&uri, |content| {
                Self::build_code_actions(content, &uri, &diagnostics)
            })
            .await
            .unwrap_or_default();
        Ok(Some(actions))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let tokens = self
            .with_document_text(&uri, Self::build_semantic_tokens)
            .await;
        Ok(tokens.map(SemanticTokensResult::Tokens))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let items = match self
            .with_document_text(&uri, |content| content.to_string())
            .await
        {
            Some(text) => {
                let workspace = self.workspace_modules_for(&uri).await;
                Self::build_completion_items(&text, &uri, &workspace)
            }
            None => Vec::new(),
        };
        Ok(Some(CompletionResponse::Array(items)))
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
