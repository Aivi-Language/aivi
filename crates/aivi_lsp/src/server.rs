use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
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
    Location, OneOf, ReferenceParams, RenameParams, SelectionRange, SelectionRangeParams,
    SelectionRangeProviderCapability, SemanticTokensFullOptions, SemanticTokensOptions,
    SemanticTokensParams, SemanticTokensResult, SemanticTokensServerCapabilities,
    ServerCapabilities, SignatureHelp, SignatureHelpOptions, SignatureHelpParams,
    SymbolInformation, TextDocumentPositionParams, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextEdit, Url, WorkspaceEdit, WorkspaceSymbolParams,
};
use tower_lsp::{LanguageServer, LspService, Server};

use crate::backend::Backend;
use crate::state::BackendState;
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

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
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
        self.update_document(uri.clone(), text.clone()).await;
        let workspace = self.workspace_modules_for_diagnostics(&uri).await;
        let (include_specs_snippets, strict, parse_diags, checkpoint) = {
            let state = self.state.lock().await;
            let parse_diags = state
                .documents
                .get(&uri)
                .map(|doc| doc.parse_diags.clone())
                .unwrap_or_default();
            (
                state.diagnostics_in_specs_snippets,
                state.strict.clone(),
                parse_diags,
                state.typecheck_checkpoint.clone(),
            )
        };
        let uri2 = uri.clone();
        let (diagnostics, new_checkpoint) = tokio::task::spawn_blocking(move || {
            let (cp, is_new) = match checkpoint {
                Some(cp) => (cp, false),
                None => {
                    let stdlib = aivi::embedded_stdlib_modules();
                    (aivi::check_types_stdlib_checkpoint(&stdlib), true)
                }
            };
            let diags = Backend::build_diagnostics_with_workspace(
                &text,
                &uri2,
                &workspace,
                include_specs_snippets,
                &strict,
                Some(parse_diags),
                Some(&cp),
            );
            (diags, is_new.then_some(cp))
        })
        .await
        .unwrap_or_default();
        if let Some(cp) = new_checkpoint {
            let mut state = self.state.lock().await;
            state.typecheck_checkpoint.get_or_insert(cp);
        }
        self.client
            .publish_diagnostics(uri, diagnostics, Some(version))
            .await;
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

        self.update_document(uri.clone(), text.clone()).await;

        // Phase 1: debounce — cancel the previous in-flight task and start a fresh timer.
        let current_version = {
            let mut state = self.state.lock().await;
            if let Some(handle) = state.pending_diagnostics.take() {
                handle.abort();
            }
            state.diagnostics_version += 1;
            state.diagnostics_version
        };

        tokio::time::sleep(Duration::from_millis(150)).await;

        // If another keystroke arrived while we were sleeping, bail out.
        {
            let state = self.state.lock().await;
            if state.diagnostics_version != current_version {
                return;
            }
        }

        let workspace = self.workspace_modules_for_diagnostics(&uri).await;
        let (include_specs_snippets, strict, parse_diags, checkpoint) = {
            let state = self.state.lock().await;
            let parse_diags = state
                .documents
                .get(&uri)
                .map(|doc| doc.parse_diags.clone())
                .unwrap_or_default();
            (
                state.diagnostics_in_specs_snippets,
                state.strict.clone(),
                parse_diags,
                state.typecheck_checkpoint.clone(),
            )
        };

        let uri2 = uri.clone();
        let text2 = text.clone();
        let state_arc = Arc::clone(&self.state);
        let client = self.client.clone();

        let handle = tokio::spawn(async move {
            let (diagnostics, new_checkpoint) = tokio::task::spawn_blocking(move || {
                let (cp, is_new) = match checkpoint {
                    Some(cp) => (cp, false),
                    None => {
                        let stdlib = aivi::embedded_stdlib_modules();
                        (aivi::check_types_stdlib_checkpoint(&stdlib), true)
                    }
                };
                let diags = Backend::build_diagnostics_with_workspace(
                    &text2,
                    &uri2,
                    &workspace,
                    include_specs_snippets,
                    &strict,
                    Some(parse_diags),
                    Some(&cp),
                );
                (diags, is_new.then_some(cp))
            })
            .await
            .unwrap_or_default();

            let should_publish = {
                let mut state = state_arc.lock().await;
                if let Some(cp) = new_checkpoint {
                    state.typecheck_checkpoint.get_or_insert(cp);
                }
                state.pending_diagnostics = None;
                state.diagnostics_version == current_version
            };

            if should_publish {
                client
                    .publish_diagnostics(uri, diagnostics, Some(version))
                    .await;
            }
        });

        // Store the abort handle so the next keystroke can cancel this task.
        {
            let mut state = self.state.lock().await;
            if state.diagnostics_version == current_version {
                state.pending_diagnostics = Some(handle.abort_handle());
            } else {
                handle.abort();
            }
        }
    }

    async fn did_close(&self, params: tower_lsp::lsp_types::DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.remove_document(&uri).await;
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
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

        let (open_uris, include_specs_snippets, strict) = {
            let state = self.state.lock().await;
            (
                state.documents.keys().cloned().collect::<Vec<_>>(),
                state.diagnostics_in_specs_snippets,
                state.strict.clone(),
            )
        };

        for uri in open_uris {
            let doc_path = PathBuf::from(Self::path_from_uri(&uri));
            let Some(root) = Self::project_root_for_path(&doc_path, &workspace_folders) else {
                continue;
            };
            if !affected_roots.contains(&root) {
                continue;
            }

            let workspace = self.workspace_modules_for_diagnostics(&uri).await;
            let Some(text) = self
                .with_document_text(&uri, |content| content.to_string())
                .await
            else {
                continue;
            };
            let uri2 = uri.clone();
            let strict2 = strict.clone();
            let checkpoint = {
                let state = self.state.lock().await;
                state.typecheck_checkpoint.clone()
            };
            let diagnostics = tokio::task::spawn_blocking(move || {
                let (cp_opt, is_new) = match checkpoint {
                    Some(cp) => (cp, false),
                    None => {
                        let stdlib = aivi::embedded_stdlib_modules();
                        (aivi::check_types_stdlib_checkpoint(&stdlib), true)
                    }
                };
                let diags = Self::build_diagnostics_with_workspace(
                    &text,
                    &uri2,
                    &workspace,
                    include_specs_snippets,
                    &strict2,
                    None,
                    Some(&cp_opt),
                );
                (diags, is_new.then_some(cp_opt))
            })
            .await
            .unwrap_or_default();
            if let Some(cp) = diagnostics.1 {
                let mut state = self.state.lock().await;
                state.typecheck_checkpoint.get_or_insert(cp);
            }
            self.client
                .publish_diagnostics(uri, diagnostics.0, None)
                .await;
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
        let Some(text) = self
            .with_document_text(&uri, |content| content.to_string())
            .await
        else {
            return Ok(Some(CompletionResponse::Array(Vec::new())));
        };
        let workspace = self.workspace_modules_for(&uri).await;
        let uri2 = uri.clone();
        let items = tokio::task::spawn_blocking(move || {
            Self::build_completion_items(&text, &uri2, position, &workspace)
        })
        .await
        .unwrap_or_default();
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
