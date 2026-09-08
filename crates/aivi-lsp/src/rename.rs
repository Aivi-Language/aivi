use std::{collections::HashMap, sync::Arc};

use aivi_base::LspPosition;
use tower_lsp::lsp_types::{
    PrepareRenameResponse, RenameParams, TextDocumentPositionParams, TextEdit, Url, WorkspaceEdit,
};

use crate::{
    navigation::{NavigationAnalysis, NavigationLookup},
    state::ServerState,
};

/// Confirm the cursor is on a renameable position (any site that resolves to
/// definition targets).  Returns `DefaultBehavior` if valid.
pub fn prepare_rename(
    params: TextDocumentPositionParams,
    state: Arc<ServerState>,
) -> Option<PrepareRenameResponse> {
    let uri = &params.text_document.uri;
    let lsp_pos = params.position;

    let file = state.file(uri)?;
    let navigation = NavigationAnalysis::load(&state.db, file);
    match navigation.definition_targets_at_lsp_position(
        &state.db,
        LspPosition {
            line: lsp_pos.line,
            character: lsp_pos.character,
        },
    ) {
        NavigationLookup::Targets(_) => Some(PrepareRenameResponse::DefaultBehavior {
            default_behavior: true,
        }),
        NavigationLookup::NoSite | NavigationLookup::NoTargets => None,
    }
}

/// Collect all reference locations (same algorithm as find-all-references) and
/// produce a `WorkspaceEdit` that replaces every occurrence with `new_name`.
pub fn rename(params: RenameParams, state: Arc<ServerState>) -> Option<WorkspaceEdit> {
    let uri = &params.text_document_position.text_document.uri;
    let lsp_pos = params.text_document_position.position;
    let new_name = &params.new_name;

    let file = state.file(uri)?;
    let navigation = NavigationAnalysis::load(&state.db, file);

    let targets = match navigation.definition_targets_at_lsp_position(
        &state.db,
        LspPosition {
            line: lsp_pos.line,
            character: lsp_pos.character,
        },
    ) {
        NavigationLookup::Targets(t) => t,
        NavigationLookup::NoSite | NavigationLookup::NoTargets => return None,
    };

    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    for location in state
        .workspace_index
        .snapshot(&state)
        .reference_locations(&targets)
    {
        changes.entry(location.uri).or_default().push(TextEdit {
            range: location.range,
            new_text: new_name.clone(),
        });
    }

    if changes.is_empty() {
        None
    } else {
        Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        })
    }
}
