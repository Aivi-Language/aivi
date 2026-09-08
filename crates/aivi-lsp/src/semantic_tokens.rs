use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use aivi_base::LspPosition;
use aivi_syntax::{TokenKind, lex_module};
use tower_lsp::lsp_types::{
    Position, Range, SemanticToken, SemanticTokenType, SemanticTokens, SemanticTokensDelta,
    SemanticTokensDeltaParams, SemanticTokensEdit, SemanticTokensFullDeltaResult,
    SemanticTokensParams, SemanticTokensRangeParams, SemanticTokensRangeResult,
    SemanticTokensResult, Url,
};

use crate::state::ServerState;

/// Ordered list of token type names used in the legend.  The index in this
/// array is the `token_type` field emitted for each `SemanticToken`.
pub const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::TYPE,
    SemanticTokenType::FUNCTION,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::KEYWORD,
    SemanticTokenType::STRING,
    SemanticTokenType::NUMBER,
    SemanticTokenType::OPERATOR,
    SemanticTokenType::COMMENT,
];

const IDX_KEYWORD: u32 = 3;
const IDX_STRING: u32 = 4;
const IDX_NUMBER: u32 = 5;
const IDX_COMMENT: u32 = 7;
const SEMANTIC_TOKEN_HISTORY_LIMIT: usize = 4;

#[derive(Clone)]
struct CachedSemanticTokens {
    result_id: String,
    data: Arc<[SemanticToken]>,
}

/// Bounded per-document history backing full/delta semantic token requests.
#[derive(Default)]
pub(crate) struct SemanticTokenHistory {
    next_result_id: AtomicU64,
    documents: Mutex<HashMap<Url, VecDeque<CachedSemanticTokens>>>,
}

impl SemanticTokenHistory {
    fn record(&self, uri: &Url, data: &[SemanticToken]) -> String {
        let sequence = self.next_result_id.fetch_add(1, Ordering::Relaxed);
        let result_id = format!("aivi-semantic-{sequence}");
        let mut documents = self
            .documents
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let history = documents.entry(uri.clone()).or_default();
        history.push_back(CachedSemanticTokens {
            result_id: result_id.clone(),
            data: Arc::from(data),
        });
        while history.len() > SEMANTIC_TOKEN_HISTORY_LIMIT {
            history.pop_front();
        }
        result_id
    }

    fn get(&self, uri: &Url, result_id: &str) -> Option<Arc<[SemanticToken]>> {
        self.documents
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(uri)?
            .iter()
            .find(|cached| cached.result_id == result_id)
            .map(|cached| Arc::clone(&cached.data))
    }

    pub fn remove(&self, uri: &Url) {
        self.documents
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(uri);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AbsoluteSemanticToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
    token_modifiers_bitset: u32,
}

fn token_type_index(kind: TokenKind) -> Option<u32> {
    match kind {
        // Keywords
        // Keep `type` on TextMate scopes so type signatures/declarations can
        // opt into a uniform line color without a semantic-keyword override.
        TokenKind::TypeKw => None,
        TokenKind::FuncKw
        | TokenKind::ValueKw
        | TokenKind::SignalKw
        | TokenKind::FromKw
        | TokenKind::ClassKw
        | TokenKind::InstanceKw
        | TokenKind::DomainKw
        | TokenKind::ProviderKw
        | TokenKind::UseKw
        | TokenKind::ExportKw
        | TokenKind::HoistKw
        | TokenKind::PatchKw => Some(IDX_KEYWORD),

        // Identifiers: deferred to soft_or_hard_token_type_index.
        TokenKind::Identifier => None,

        // Literals
        TokenKind::StringLiteral | TokenKind::RegexLiteral => Some(IDX_STRING),
        TokenKind::Integer | TokenKind::Float | TokenKind::Decimal | TokenKind::BigInt => {
            Some(IDX_NUMBER)
        }

        // Operators and punctuation: let TextMate grammar handle these so that
        // per-operator colors (pipe variants, arrows, etc.) are preserved.
        TokenKind::Plus
        | TokenKind::Minus
        | TokenKind::Slash
        | TokenKind::Star
        | TokenKind::Percent
        | TokenKind::Less
        | TokenKind::Greater
        | TokenKind::LessEqual
        | TokenKind::GreaterEqual
        | TokenKind::Equals
        | TokenKind::EqualEqual
        | TokenKind::Bang
        | TokenKind::BangEqual
        | TokenKind::Ellipsis
        | TokenKind::Arrow
        | TokenKind::ThinArrow
        | TokenKind::LeftArrow
        | TokenKind::ColonEquals
        | TokenKind::PipeTransform
        | TokenKind::PipeGate
        | TokenKind::PipeCase
        | TokenKind::PipeMap
        | TokenKind::PipeApply
        | TokenKind::PipeRecurStart
        | TokenKind::PipeRecurStep
        | TokenKind::PipeTap
        | TokenKind::PipeFanIn
        | TokenKind::PatchApply
        | TokenKind::TruthyBranch
        | TokenKind::FalsyBranch
        | TokenKind::PipeValidate
        | TokenKind::PipePrevious
        | TokenKind::PipeAccumulate
        | TokenKind::PipeDiff
        | TokenKind::PipeDelay
        | TokenKind::PipeBurst
        | TokenKind::At
        | TokenKind::Hash
        | TokenKind::Colon
        | TokenKind::Dot
        | TokenKind::DotDot
        | TokenKind::Comma
        | TokenKind::LParen
        | TokenKind::RParen
        | TokenKind::LBrace
        | TokenKind::RBrace
        | TokenKind::LBracket
        | TokenKind::RBracket
        | TokenKind::CloseTagStart
        | TokenKind::SelfCloseTagEnd => None,

        // Comments
        TokenKind::LineComment | TokenKind::BlockComment | TokenKind::DocComment => {
            Some(IDX_COMMENT)
        }

        // Whitespace and unknown — not emitted.
        TokenKind::Whitespace | TokenKind::Newline | TokenKind::Unknown => None,
    }
}

pub fn semantic_tokens_full(
    params: SemanticTokensParams,
    state: Arc<ServerState>,
) -> Option<SemanticTokensResult> {
    let uri = &params.text_document.uri;
    let file = state.file(uri)?;
    let source = file.source(&state.db);
    let data = encode_tokens(&collect_absolute_tokens(&source, None));
    let result_id = state.semantic_tokens.record(uri, &data);
    Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: Some(result_id),
        data,
    }))
}

pub fn semantic_tokens_full_delta(
    params: SemanticTokensDeltaParams,
    state: Arc<ServerState>,
) -> Option<SemanticTokensFullDeltaResult> {
    let uri = &params.text_document.uri;
    let file = state.file(uri)?;
    let source = file.source(&state.db);
    let data = encode_tokens(&collect_absolute_tokens(&source, None));
    let previous = state.semantic_tokens.get(uri, &params.previous_result_id);
    let result_id = state.semantic_tokens.record(uri, &data);

    let Some(previous) = previous else {
        return Some(SemanticTokensFullDeltaResult::Tokens(SemanticTokens {
            result_id: Some(result_id),
            data,
        }));
    };
    let Some(edits) = semantic_token_delta(&previous, &data) else {
        return Some(SemanticTokensFullDeltaResult::Tokens(SemanticTokens {
            result_id: Some(result_id),
            data,
        }));
    };
    Some(SemanticTokensFullDeltaResult::TokensDelta(
        SemanticTokensDelta {
            result_id: Some(result_id),
            edits,
        },
    ))
}

pub fn semantic_tokens_range(
    params: SemanticTokensRangeParams,
    state: Arc<ServerState>,
) -> Option<SemanticTokensRangeResult> {
    let uri = &params.text_document.uri;
    let file = state.file(uri)?;
    let source = file.source(&state.db);
    validate_range(&source, params.range)?;
    let data = encode_tokens(&collect_absolute_tokens(&source, Some(params.range)));
    Some(SemanticTokensRangeResult::Tokens(SemanticTokens {
        result_id: None,
        data,
    }))
}

fn collect_absolute_tokens(
    source: &aivi_base::SourceFile,
    requested_range: Option<Range>,
) -> Vec<AbsoluteSemanticToken> {
    let lexed = lex_module(source);
    let mut result = Vec::new();

    for (index, token) in lexed.tokens().iter().copied().enumerate() {
        let Some(type_index) = soft_or_hard_token_type_index(token, lexed.tokens(), index, source)
        else {
            continue;
        };

        let lsp_range = source.span_to_lsp_range(token.span());
        let token_line = lsp_range.start.line;
        let token_char = lsp_range.start.character;
        let token_len = lsp_range
            .end
            .character
            .saturating_sub(lsp_range.start.character);

        // Multi-line tokens (e.g. block comments): skip — the LSP spec
        // requires single-line tokens in the full-tokens response.
        if lsp_range.start.line != lsp_range.end.line {
            continue;
        }

        let token_range = Range {
            start: Position {
                line: token_line,
                character: token_char,
            },
            end: Position {
                line: lsp_range.end.line,
                character: lsp_range.end.character,
            },
        };
        if requested_range.is_some_and(|range| !ranges_overlap(token_range, range)) {
            continue;
        }

        result.push(AbsoluteSemanticToken {
            line: token_line,
            start: token_char,
            length: token_len,
            token_type: type_index,
            token_modifiers_bitset: 0,
        });
    }
    result
}

fn encode_tokens(tokens: &[AbsoluteSemanticToken]) -> Vec<SemanticToken> {
    let mut result = Vec::with_capacity(tokens.len());
    let mut previous_line = 0;
    let mut previous_start = 0;
    for token in tokens {
        let delta_line = token.line - previous_line;
        let delta_start = if delta_line == 0 {
            token.start - previous_start
        } else {
            token.start
        };
        result.push(SemanticToken {
            delta_line,
            delta_start,
            length: token.length,
            token_type: token.token_type,
            token_modifiers_bitset: token.token_modifiers_bitset,
        });
        previous_line = token.line;
        previous_start = token.start;
    }
    result
}

fn semantic_token_delta(
    previous: &[SemanticToken],
    current: &[SemanticToken],
) -> Option<Vec<SemanticTokensEdit>> {
    let prefix = previous
        .iter()
        .zip(current)
        .take_while(|(left, right)| left == right)
        .count();
    let max_suffix = previous.len().min(current.len()).saturating_sub(prefix);
    let suffix = previous
        .iter()
        .rev()
        .zip(current.iter().rev())
        .take(max_suffix)
        .take_while(|(left, right)| left == right)
        .count();
    if prefix == previous.len() && prefix == current.len() {
        return Some(Vec::new());
    }

    let start = u32::try_from(prefix.checked_mul(5)?).ok()?;
    let removed_tokens = previous.len().checked_sub(prefix + suffix)?;
    let delete_count = u32::try_from(removed_tokens.checked_mul(5)?).ok()?;
    let inserted_end = current.len().checked_sub(suffix)?;
    let inserted = current[prefix..inserted_end].to_vec();
    Some(vec![SemanticTokensEdit {
        start,
        delete_count,
        data: (!inserted.is_empty()).then_some(inserted),
    }])
}

fn validate_range(source: &aivi_base::SourceFile, range: Range) -> Option<()> {
    if range.start > range.end {
        return None;
    }
    source.lsp_position_to_offset(LspPosition {
        line: range.start.line,
        character: range.start.character,
    })?;
    source.lsp_position_to_offset(LspPosition {
        line: range.end.line,
        character: range.end.character,
    })?;
    Some(())
}

fn ranges_overlap(left: Range, right: Range) -> bool {
    left.start < right.end && right.start < left.end
}

fn soft_or_hard_token_type_index(
    token: aivi_syntax::Token,
    tokens: &[aivi_syntax::Token],
    index: usize,
    source: &aivi_base::SourceFile,
) -> Option<u32> {
    match token.kind() {
        TokenKind::Identifier if token.text(source) == "when" => Some(IDX_KEYWORD),
        TokenKind::Identifier if temporal_stage_head(tokens, index, source) => Some(IDX_KEYWORD),
        // Interpolated string literals need TextMate's nested scopes so the
        // interpolation braces and body can be themed independently.
        TokenKind::StringLiteral if string_literal_has_interpolation(token.text(source)) => None,
        // Let TextMate grammar handle identifier coloring — it uses specific scopes
        // (e.g. variable.parameter.labeled, variable.other.field) that carry more
        // precise color intent than a blanket `variable` semantic token.
        TokenKind::Identifier => None,
        kind => token_type_index(kind),
    }
}

fn temporal_stage_head(
    tokens: &[aivi_syntax::Token],
    index: usize,
    source: &aivi_base::SourceFile,
) -> bool {
    let token = tokens[index];
    if token.kind() != TokenKind::Identifier {
        return false;
    }
    if !matches!(token.text(source), "delay" | "burst") {
        return false;
    }

    let mut cursor = index;
    loop {
        let Some(previous) = previous_significant_token(tokens, cursor) else {
            return false;
        };
        match tokens[previous].kind() {
            TokenKind::PipeTransform => return true,
            TokenKind::Identifier => {
                let Some(hash) = previous_significant_token(tokens, previous) else {
                    return false;
                };
                if tokens[hash].kind() != TokenKind::Hash {
                    return false;
                }
                cursor = hash;
            }
            _ => return false,
        }
    }
}

fn previous_significant_token(tokens: &[aivi_syntax::Token], before: usize) -> Option<usize> {
    (0..before).rev().find(|&index| {
        !matches!(
            tokens[index].kind(),
            TokenKind::Whitespace
                | TokenKind::Newline
                | TokenKind::LineComment
                | TokenKind::BlockComment
                | TokenKind::DocComment
        )
    })
}

fn string_literal_has_interpolation(literal: &str) -> bool {
    let mut chars = literal.chars();
    if chars.next() != Some('"') {
        return false;
    }

    let mut escaped = false;
    for ch in chars {
        if escaped {
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '"' => return false,
            '{' => return true,
            _ => {}
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::{
        IDX_KEYWORD, IDX_STRING, soft_or_hard_token_type_index, string_literal_has_interpolation,
        temporal_stage_head, token_type_index,
    };
    use aivi_base::{FileId, SourceFile};
    use aivi_syntax::{TokenKind, lex_module};

    #[test]
    fn leaves_patch_surface_operators_to_textmate() {
        assert_eq!(token_type_index(TokenKind::PatchKw), Some(IDX_KEYWORD));
        assert_eq!(token_type_index(TokenKind::PatchApply), None);
        assert_eq!(token_type_index(TokenKind::ColonEquals), None);
    }

    #[test]
    fn classifies_when_as_soft_keyword() {
        let source = SourceFile::new(FileId::new(0), "test.aivi", "when ready => total <- 1\n");
        let lexed = lex_module(&source);
        let when = lexed
            .tokens()
            .iter()
            .find(|token| token.kind() == TokenKind::Identifier && token.text(&source) == "when")
            .copied()
            .expect("expected `when` token");
        assert_eq!(
            soft_or_hard_token_type_index(when, lexed.tokens(), 0, &source),
            Some(IDX_KEYWORD)
        );
    }

    #[test]
    fn leaves_type_keyword_to_textmate() {
        assert_eq!(token_type_index(TokenKind::TypeKw), None);
    }

    #[test]
    fn treats_from_as_a_keyword() {
        assert_eq!(token_type_index(TokenKind::FromKw), Some(IDX_KEYWORD));
    }

    #[test]
    fn detects_unescaped_text_interpolation_holes() {
        assert!(string_literal_has_interpolation(
            r#""Final score: {game.score}""#
        ));
        assert!(!string_literal_has_interpolation(
            r#""use \{literal\} braces""#
        ));
    }

    #[test]
    fn leaves_interpolated_string_literals_to_textmate() {
        let source = SourceFile::new(
            FileId::new(0),
            "test.aivi",
            r#"value label = "Final score: {game.score}""#,
        );
        let lexed = lex_module(&source);
        let string = lexed
            .tokens()
            .iter()
            .find(|token| token.kind() == TokenKind::StringLiteral)
            .copied()
            .expect("expected a string literal token");

        let string_index = lexed
            .tokens()
            .iter()
            .position(|token| *token == string)
            .expect("expected string token index");

        assert_eq!(
            soft_or_hard_token_type_index(string, lexed.tokens(), string_index, &source),
            None
        );
    }

    #[test]
    fn keeps_plain_string_literals_as_semantic_strings() {
        let source = SourceFile::new(
            FileId::new(0),
            "test.aivi",
            r#"value label = "use \{literal\} braces""#,
        );
        let lexed = lex_module(&source);
        let string = lexed
            .tokens()
            .iter()
            .find(|token| token.kind() == TokenKind::StringLiteral)
            .copied()
            .expect("expected a string literal token");

        let string_index = lexed
            .tokens()
            .iter()
            .position(|token| *token == string)
            .expect("expected string token index");

        assert_eq!(
            soft_or_hard_token_type_index(string, lexed.tokens(), string_index, &source),
            Some(IDX_STRING)
        );
    }

    #[test]
    fn classifies_temporal_stage_heads_as_soft_keywords() {
        let source = SourceFile::new(
            FileId::new(0),
            "test.aivi",
            "signal later = click\n  |> #memo delay 80ms\n  |> burst 150ms 3times\n",
        );
        let lexed = lex_module(&source);
        let delay_index = lexed
            .tokens()
            .iter()
            .position(|token| {
                token.kind() == TokenKind::Identifier && token.text(&source) == "delay"
            })
            .expect("expected `delay` token");
        let burst_index = lexed
            .tokens()
            .iter()
            .position(|token| {
                token.kind() == TokenKind::Identifier && token.text(&source) == "burst"
            })
            .expect("expected `burst` token");

        assert!(temporal_stage_head(lexed.tokens(), delay_index, &source));
        assert!(temporal_stage_head(lexed.tokens(), burst_index, &source));
        assert_eq!(
            soft_or_hard_token_type_index(
                lexed.tokens()[delay_index],
                lexed.tokens(),
                delay_index,
                &source
            ),
            Some(IDX_KEYWORD)
        );
        assert_eq!(
            soft_or_hard_token_type_index(
                lexed.tokens()[burst_index],
                lexed.tokens(),
                burst_index,
                &source
            ),
            Some(IDX_KEYWORD)
        );
    }

    #[test]
    fn leaves_ordinary_delay_identifiers_unclassified() {
        let source = SourceFile::new(
            FileId::new(0),
            "test.aivi",
            "value delay = 1\nsignal later = click |> transform delay\n",
        );
        let lexed = lex_module(&source);
        let delay_index = lexed
            .tokens()
            .iter()
            .position(|token| {
                token.kind() == TokenKind::Identifier
                    && token.text(&source) == "delay"
                    && !temporal_stage_head(
                        lexed.tokens(),
                        lexed
                            .tokens()
                            .iter()
                            .position(|candidate| candidate == token)
                            .expect("expected delay index"),
                        &source,
                    )
            })
            .expect("expected ordinary `delay` identifier");

        assert_eq!(
            soft_or_hard_token_type_index(
                lexed.tokens()[delay_index],
                lexed.tokens(),
                delay_index,
                &source
            ),
            None
        );
    }
}
