use std::sync::Arc;

use aivi_base::LspPosition;
use aivi_hir::LspSymbolKind;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse,
};

use crate::{analysis::FileAnalysis, state::ServerState};

pub fn completion(params: CompletionParams, state: Arc<ServerState>) -> Option<CompletionResponse> {
    let uri = &params.text_document_position.text_document.uri;
    let lsp_pos = params.text_document_position.position;

    let file = state.file(uri)?;
    let current_analysis = FileAnalysis::load(&state.db, file);

    // Reject out-of-range cursor positions before returning any items.
    current_analysis
        .source
        .lsp_position_to_offset(LspPosition {
            line: lsp_pos.line,
            character: lsp_pos.character,
        })?;

    let workspace = state.workspace_index.snapshot(&state);
    let mut items: Vec<CompletionItem> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    // Preserve the current file's declarations when another open file uses
    // the same label, then append the remaining workspace declarations.
    for current_file in [true, false] {
        for symbol in workspace
            .symbols()
            .iter()
            .filter(|symbol| symbol.top_level && ((symbol.location.uri == *uri) == current_file))
        {
            if !seen.insert(symbol.name.clone()) {
                continue;
            }
            items.push(CompletionItem {
                label: symbol.name.clone(),
                kind: Some(lsp_symbol_kind_to_completion_kind(symbol.kind)),
                detail: symbol.detail.clone(),
                ..Default::default()
            });
        }
    }

    if items.is_empty() {
        None
    } else {
        Some(CompletionResponse::Array(items))
    }
}

fn lsp_symbol_kind_to_completion_kind(kind: LspSymbolKind) -> CompletionItemKind {
    match kind {
        LspSymbolKind::Function | LspSymbolKind::Method => CompletionItemKind::FUNCTION,
        LspSymbolKind::Variable | LspSymbolKind::Constant => CompletionItemKind::VARIABLE,
        LspSymbolKind::Field | LspSymbolKind::Property => CompletionItemKind::FIELD,
        LspSymbolKind::Enum => CompletionItemKind::ENUM,
        LspSymbolKind::EnumMember => CompletionItemKind::ENUM_MEMBER,
        LspSymbolKind::Struct => CompletionItemKind::STRUCT,
        LspSymbolKind::Class | LspSymbolKind::Interface => CompletionItemKind::CLASS,
        LspSymbolKind::Module | LspSymbolKind::Namespace | LspSymbolKind::Package => {
            CompletionItemKind::MODULE
        }
        LspSymbolKind::Constructor => CompletionItemKind::CONSTRUCTOR,
        LspSymbolKind::TypeParameter => CompletionItemKind::TYPE_PARAMETER,
        LspSymbolKind::Operator => CompletionItemKind::OPERATOR,
        _ => CompletionItemKind::TEXT,
    }
}
