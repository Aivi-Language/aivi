use crate::{
    navigation::{NavigationAnalysis, NavigationLookup, NavigationTarget},
    state::ServerState,
};
use aivi_base::{LspPosition, SourceDatabase};
use aivi_syntax::{TokenKind, lex_module};
use std::{collections::BTreeMap, sync::Arc};
use tower_lsp::lsp_types::{
    DocumentChanges, OneOf, OptionalVersionedTextDocumentIdentifier, PrepareRenameResponse,
    RenameParams, TextDocumentEdit, TextDocumentPositionParams, TextEdit, Url, WorkspaceEdit,
};

fn identifier(text: &str) -> bool {
    let mut sources = SourceDatabase::new();
    let id = sources.add_file("rename.aivi", text);
    let lexed = lex_module(&sources[id]);
    let tokens = lexed.tokens();
    tokens.len() == 1 && tokens[0].kind() == TokenKind::Identifier
}

fn target(
    params: &TextDocumentPositionParams,
    state: &ServerState,
) -> Option<(NavigationTarget, String)> {
    if !state
        .workspace_complete
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return None;
    }
    let file = state.file(&params.text_document.uri)?;
    let navigation = NavigationAnalysis::load(&state.db, file);
    let NavigationLookup::Targets(targets) = navigation.definition_targets_at_lsp_position(
        &state.db,
        LspPosition {
            line: params.position.line,
            character: params.position.character,
        },
    ) else {
        return None;
    };
    let [target] = targets.as_slice() else {
        return None;
    };
    // Library symbols and ambiguous dispatch are never edited implicitly.
    if !state
        .project_files()
        .iter()
        .any(|(_, file)| *file == target.file())
    {
        return None;
    }
    let source = target.file().source(&state.db);
    let span = target.span.span();
    let name = source
        .text()
        .get(span.start().as_usize()..span.end().as_usize())?;
    identifier(name).then(|| (*target, name.to_owned()))
}

pub fn prepare_rename(
    params: TextDocumentPositionParams,
    state: Arc<ServerState>,
) -> Option<PrepareRenameResponse> {
    let (_, name) = target(&params, &state)?;
    rename(
        RenameParams {
            text_document_position: params.clone(),
            new_name: name.clone(),
            work_done_progress_params: Default::default(),
        },
        Arc::clone(&state),
    )?;
    let navigation = NavigationAnalysis::load(&state.db, state.file(&params.text_document.uri)?);
    let range = navigation.reference_range_at_lsp_position(LspPosition {
        line: params.position.line,
        character: params.position.character,
    })?;
    Some(PrepareRenameResponse::RangeWithPlaceholder {
        range,
        placeholder: name,
    })
}

/// Conservative capture avoidance: a replacement identifier must be fresh in
/// every affected module. Aliased and ambiguous sites are refused, not guessed.
pub fn rename(params: RenameParams, state: Arc<ServerState>) -> Option<WorkspaceEdit> {
    if !identifier(&params.new_name) {
        return None;
    }
    let (target, old_name) = target(&params.text_document_position, &state)?;
    let locations = state
        .workspace_index
        .snapshot(&state)
        .reference_locations(&[target]);
    let files = state
        .project_files()
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    let mut changes: BTreeMap<Url, Vec<OneOf<TextEdit, tower_lsp::lsp_types::AnnotatedTextEdit>>> =
        BTreeMap::new();
    for location in locations {
        let file = *files.get(&location.uri)?;
        let source = file.source(&state.db);
        let hir = aivi_query::hir_module(&state.db, file);
        if hir
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.severity == aivi_base::Severity::Error)
        {
            return None;
        }
        let start = source.lsp_position_to_offset(LspPosition {
            line: location.range.start.line,
            character: location.range.start.character,
        })?;
        let end = source.lsp_position_to_offset(LspPosition {
            line: location.range.end.line,
            character: location.range.end.character,
        })?;
        if source.text().get(start.as_usize()..end.as_usize())? != old_name {
            return None;
        }
        // Renaming a shorthand binding must not also rename its record key.
        // Until structural rewrites are supported, refuse these coupled sites.
        let shorthand = hir
            .module()
            .exprs()
            .iter()
            .any(|(_, expr)| match &expr.kind {
                aivi_hir::ExprKind::Record(record) => record.fields.iter().any(|field| {
                    field.surface == aivi_hir::RecordFieldSurface::Shorthand
                        && field.label.span().span().contains(start)
                }),
                _ => false,
            })
            || hir
                .module()
                .patterns()
                .iter()
                .any(|(_, pattern)| match &pattern.kind {
                    aivi_hir::PatternKind::Record(fields) => fields.iter().any(|field| {
                        field.surface == aivi_hir::RecordFieldSurface::Shorthand
                            && field.label.span().span().contains(start)
                    }),
                    _ => false,
                });
        if shorthand {
            return None;
        }

        if !changes.contains_key(&location.uri) && old_name != params.new_name {
            let lexed = lex_module(&source);
            if lexed.tokens().iter().any(|token| {
                token.kind() == TokenKind::Identifier
                    && source
                        .text()
                        .get(token.span().start().as_usize()..token.span().end().as_usize())
                        == Some(params.new_name.as_str())
            }) {
                return None;
            }
        }
        changes
            .entry(location.uri)
            .or_default()
            .push(OneOf::Left(TextEdit {
                range: location.range,
                new_text: params.new_name.clone(),
            }));
    }
    if changes.is_empty() {
        return None;
    }
    Some(WorkspaceEdit {
        document_changes: Some(DocumentChanges::Edits(
            changes
                .into_iter()
                .map(|(uri, edits)| TextDocumentEdit {
                    text_document: OptionalVersionedTextDocumentIdentifier {
                        version: state
                            .document_snapshot(&uri)
                            .map(|document| document.version),
                        uri,
                    },
                    edits,
                })
                .collect(),
        )),
        ..Default::default()
    })
}
