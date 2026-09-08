use std::sync::Arc;

use aivi_hir::LspSymbol;
use aivi_syntax::{TokenKind, lex_module};
use tower_lsp::lsp_types::{FoldingRange, FoldingRangeKind, FoldingRangeParams};

use crate::state::ServerState;

pub fn folding_ranges(
    params: FoldingRangeParams,
    state: Arc<ServerState>,
) -> Option<Vec<FoldingRange>> {
    let file = state.file(&params.text_document.uri)?;
    let hir = aivi_query::hir_module(&state.db, file);
    let mut ranges = symbol_ranges(hir.symbols(), hir.source());
    ranges.extend(comment_ranges(hir.source()));
    ranges.sort_by_key(|range| {
        (
            range.start_line,
            range.start_character,
            range.end_line,
            range.end_character,
        )
    });
    ranges.dedup_by(|left, right| {
        left.start_line == right.start_line
            && left.start_character == right.start_character
            && left.end_line == right.end_line
            && left.end_character == right.end_character
    });
    (!ranges.is_empty()).then_some(ranges)
}

fn symbol_ranges(roots: &[LspSymbol], source: &aivi_base::SourceFile) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    let mut stack = roots.iter().rev().collect::<Vec<_>>();
    while let Some(symbol) = stack.pop() {
        if let Some(range) = folding_range(source.span_to_lsp_range(symbol.span.span()), None) {
            ranges.push(range);
        }
        stack.extend(symbol.children.iter().rev());
    }
    ranges
}

fn comment_ranges(source: &aivi_base::SourceFile) -> Vec<FoldingRange> {
    lex_module(source)
        .tokens()
        .iter()
        .filter(|token| {
            matches!(
                token.kind(),
                TokenKind::BlockComment | TokenKind::DocComment
            )
        })
        .filter_map(|token| {
            folding_range(
                source.span_to_lsp_range(token.span()),
                Some(FoldingRangeKind::Comment),
            )
        })
        .collect()
}

fn folding_range(
    range: aivi_base::LspRange,
    kind: Option<FoldingRangeKind>,
) -> Option<FoldingRange> {
    let end_line = if range.end.character == 0 && range.end.line > range.start.line {
        range.end.line - 1
    } else {
        range.end.line
    };
    if end_line <= range.start.line {
        return None;
    }
    Some(FoldingRange {
        start_line: range.start.line,
        start_character: Some(range.start.character),
        end_line,
        end_character: (end_line == range.end.line).then_some(range.end.character),
        kind,
        collapsed_text: None,
    })
}
