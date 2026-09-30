use std::sync::Arc;

use tower_lsp::lsp_types::{InlayHint, InlayHintKind, InlayHintLabel, InlayHintParams, Position};

use crate::{analysis::FileAnalysis, state::ServerState};

/// Produce inlay hints for the visible range of the document.
///
/// Emit type hints at unannotated declaration names with known inferred types,
/// restricted to the requested range (start inclusive, end exclusive).
pub fn inlay_hints(params: InlayHintParams, state: Arc<ServerState>) -> Option<Vec<InlayHint>> {
    let config = state.config();
    if !config.inlay_hints_enabled || params.range.start >= params.range.end {
        return None;
    }

    let uri = &params.text_document.uri;
    let file = state.file(uri)?;
    let analysis = FileAnalysis::load(&state.db, file);
    let source = &analysis.source;

    let mut hints = Vec::new();

    for declaration in analysis.typed_declarations.iter() {
        if declaration.annotation.is_some() {
            continue;
        }
        let Some(inferred) = &declaration.inferred_type else {
            continue;
        };
        let lsp_range = source.span_to_lsp_range(declaration.name_span.span());
        let position = Position {
            line: lsp_range.end.line,
            character: lsp_range.end.character,
        };
        if position < params.range.start || position >= params.range.end {
            continue;
        }
        hints.push(InlayHint {
            position,
            label: InlayHintLabel::String(truncate_inlay_hint_label(
                inferred,
                config.inlay_hints_max_length,
            )),
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            tooltip: None,
            padding_left: Some(true),
            padding_right: None,
            data: None,
        });
    }

    if hints.is_empty() { None } else { Some(hints) }
}

fn truncate_inlay_hint_label(inferred: &str, max_length: usize) -> String {
    let label = format!(": {}", inferred);
    if label.chars().count() <= max_length {
        return label;
    }

    let truncated: String = label
        .chars()
        .take(max_length.saturating_sub(1))
        .collect::<String>();
    format!("{truncated}…")
}
