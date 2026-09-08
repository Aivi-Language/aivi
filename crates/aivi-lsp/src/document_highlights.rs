use std::sync::Arc;

use aivi_base::LspPosition;
use tower_lsp::lsp_types::{DocumentHighlight, DocumentHighlightKind, DocumentHighlightParams};

use crate::{
    navigation::{NavigationAnalysis, NavigationLookup},
    state::ServerState,
};

/// Return declaration and use occurrences for the selected semantic target in
/// the active document. Cross-file references remain available through the
/// references request but are intentionally excluded from document highlights.
pub fn document_highlights(
    params: DocumentHighlightParams,
    state: Arc<ServerState>,
) -> Option<Vec<DocumentHighlight>> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;
    let file = state.file(uri)?;
    let navigation = NavigationAnalysis::load(&state.db, file);
    let targets = match navigation.definition_targets_at_lsp_position(
        &state.db,
        LspPosition {
            line: position.line,
            character: position.character,
        },
    ) {
        NavigationLookup::Targets(targets) => targets,
        NavigationLookup::NoSite | NavigationLookup::NoTargets => return None,
    };

    let highlights = state
        .workspace_index
        .snapshot(&state)
        .reference_locations(&targets)
        .into_iter()
        .filter(|location| location.uri == *uri)
        .map(|location| DocumentHighlight {
            range: location.range,
            kind: Some(DocumentHighlightKind::TEXT),
        })
        .collect::<Vec<_>>();
    (!highlights.is_empty()).then_some(highlights)
}
