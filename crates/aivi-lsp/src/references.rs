use std::sync::Arc;

use aivi_base::LspPosition;
use tower_lsp::lsp_types::{Location, ReferenceParams};

use crate::{
    navigation::{NavigationAnalysis, NavigationLookup},
    state::ServerState,
};

/// Find all reference locations for the symbol under the cursor.
///
/// Resolves the cursor once, then looks the target up in the immutable
/// revision-aware workspace reference index.
pub fn references(params: ReferenceParams, state: Arc<ServerState>) -> Option<Vec<Location>> {
    let uri = &params.text_document_position.text_document.uri;
    let lsp_pos = params.text_document_position.position;

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

    let mut locations: Vec<Location> = state
        .workspace_index
        .snapshot(&state)
        .reference_locations(&targets);
    if !params.context.include_declaration {
        let declarations = targets
            .iter()
            .filter_map(|target| target.location(&state.db))
            .collect::<Vec<_>>();
        locations.retain(|location| !declarations.contains(location));
    }

    if locations.is_empty() {
        None
    } else {
        Some(locations)
    }
}
