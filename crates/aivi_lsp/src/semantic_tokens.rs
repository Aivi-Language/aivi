use aivi::{lex_cst, syntax, CstToken};
use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
};

use crate::backend::Backend;

impl Backend {
    pub(super) const KEYWORDS: &'static [&'static str] = syntax::KEYWORDS_ALL;
    pub(super) const SIGILS: [&'static str; 4] = ["~r//", "~u()", "~d()", "~dt()"];

    pub(super) const SEM_TOKEN_KEYWORD: u32 = 0;
    pub(super) const SEM_TOKEN_TYPE: u32 = 1;
    pub(super) const SEM_TOKEN_FUNCTION: u32 = 2;
    pub(super) const SEM_TOKEN_VARIABLE: u32 = 3;
    pub(super) const SEM_TOKEN_NUMBER: u32 = 4;
    pub(super) const SEM_TOKEN_STRING: u32 = 5;
    pub(super) const SEM_TOKEN_COMMENT: u32 = 6;
    pub(super) const SEM_TOKEN_OPERATOR: u32 = 7;
    pub(super) const SEM_TOKEN_DECORATOR: u32 = 8;

    pub(super) fn semantic_tokens_legend() -> SemanticTokensLegend {
        SemanticTokensLegend {
            token_types: vec![
                SemanticTokenType::KEYWORD,
                SemanticTokenType::TYPE,
                SemanticTokenType::FUNCTION,
                SemanticTokenType::VARIABLE,
                SemanticTokenType::NUMBER,
                SemanticTokenType::STRING,
                SemanticTokenType::COMMENT,
                SemanticTokenType::OPERATOR,
                SemanticTokenType::DECORATOR,
            ],
            token_modifiers: Vec::new(),
        }
    }

    fn is_operator_symbol(symbol: &str) -> bool {
        matches!(
            symbol,
            "=" | "=="
                | "!="
                | "<"
                | ">"
                | "<="
                | ">="
                | "&&"
                | "||"
                | "!"
                | "?"
                | "??"
                | "+"
                | "-"
                | "*"
                | "/"
                | "%"
                | "<-"
                | "->"
                | "=>"
                | "|>"
                | "<|"
                | "::"
                | ".."
                | "..."
                | ":"
                | "."
        )
    }

    fn classify_semantic_token(
        prev: Option<&CstToken>,
        token: &CstToken,
        next: Option<&CstToken>,
    ) -> Option<u32> {
        match token.kind.as_str() {
            "comment" => Some(Self::SEM_TOKEN_COMMENT),
            "string" => Some(Self::SEM_TOKEN_STRING),
            "sigil" => Some(Self::SEM_TOKEN_STRING),
            "number" => Some(Self::SEM_TOKEN_NUMBER),
            "symbol" => {
                if token.text == "@" {
                    Some(Self::SEM_TOKEN_DECORATOR)
                } else if Self::is_operator_symbol(&token.text) {
                    Some(Self::SEM_TOKEN_OPERATOR)
                } else {
                    None
                }
            }
            "ident" => {
                if token.text == "_" {
                    return Some(Self::SEM_TOKEN_KEYWORD);
                }
                if Self::KEYWORDS.contains(&token.text.as_str()) {
                    return Some(Self::SEM_TOKEN_KEYWORD);
                }
                if prev.is_some_and(|prev| prev.kind == "symbol" && prev.text == "@") {
                    return Some(Self::SEM_TOKEN_DECORATOR);
                }
                if matches!(
                    next,
                    Some(next) if next.kind == "symbol" && (next.text == ":" || next.text == "=")
                ) {
                    return Some(Self::SEM_TOKEN_FUNCTION);
                }
                if token
                    .text
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_uppercase())
                {
                    Some(Self::SEM_TOKEN_TYPE)
                } else {
                    Some(Self::SEM_TOKEN_VARIABLE)
                }
            }
            _ => None,
        }
    }

    fn push_semantic_token(
        data: &mut Vec<SemanticToken>,
        last_line: &mut u32,
        last_start: &mut u32,
        line: u32,
        start: u32,
        len: u32,
        token_type: u32,
    ) {
        if len == 0 {
            return;
        }

        let delta_line = line.saturating_sub(*last_line);
        let delta_start = if delta_line == 0 {
            start.saturating_sub(*last_start)
        } else {
            start
        };

        data.push(SemanticToken {
            delta_line,
            delta_start,
            length: len,
            token_type,
            token_modifiers_bitset: 0,
        });

        *last_line = line;
        *last_start = start;
    }

    fn emit_interpolated_string_tokens(
        token: &CstToken,
        data: &mut Vec<SemanticToken>,
        last_line: &mut u32,
        last_start: &mut u32,
    ) -> bool {
        if token.kind != "string" {
            return false;
        }
        if !token.text.starts_with('\"') || !token.text.contains('{') {
            return false;
        }

        let chars: Vec<char> = token.text.chars().collect();
        if chars.len() < 2 {
            return false;
        }

        let line0 = token.span.start.line.saturating_sub(1) as u32;
        let col0 = token.span.start.column.saturating_sub(1) as u32;

        let mut last_text_start: usize = 0;
        let mut i: usize = 1;
        let end_limit = chars.len().saturating_sub(1);
        let mut any = false;

        while i < end_limit {
            let ch = chars[i];
            if ch == '\\' {
                i = i.saturating_add(2);
                continue;
            }
            if ch != '{' {
                i += 1;
                continue;
            }

            let interp_open = i;
            let mut j = i + 1;
            let mut depth: i32 = 1;
            let mut in_quote: Option<char> = None;
            while j < end_limit {
                let c = chars[j];
                if let Some(q) = in_quote {
                    if q != '`' && c == '\\' {
                        j = j.saturating_add(2);
                        continue;
                    }
                    if c == q {
                        in_quote = None;
                        j += 1;
                        continue;
                    }
                    j += 1;
                    continue;
                }

                match c {
                    '"' | '\'' | '`' => {
                        in_quote = Some(c);
                        j += 1;
                    }
                    '{' => {
                        depth += 1;
                        j += 1;
                    }
                    '}' => {
                        depth -= 1;
                        j += 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => j += 1,
                }
            }

            if depth != 0 {
                break;
            }

            let interp_close = j - 1;
            any = true;

            if interp_open > last_text_start {
                Self::push_semantic_token(
                    data,
                    last_line,
                    last_start,
                    line0,
                    col0 + last_text_start as u32,
                    (interp_open - last_text_start) as u32,
                    Self::SEM_TOKEN_STRING,
                );
            }

            Self::push_semantic_token(
                data,
                last_line,
                last_start,
                line0,
                col0 + interp_open as u32,
                1,
                Self::SEM_TOKEN_OPERATOR,
            );

            let expr_start = interp_open + 1;
            let expr_end = interp_close;
            if expr_end > expr_start {
                let expr_text: String = chars[expr_start..expr_end].iter().collect();
                let (expr_tokens, _) = lex_cst(&expr_text);
                let significant: Vec<usize> = expr_tokens
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.kind != "whitespace")
                    .map(|(idx, _)| idx)
                    .collect();

                for (position, token_index) in significant.iter().copied().enumerate() {
                    let expr_token = &expr_tokens[token_index];
                    let prev = position
                        .checked_sub(1)
                        .and_then(|prev| significant.get(prev))
                        .map(|idx| &expr_tokens[*idx]);
                    let next = significant.get(position + 1).map(|idx| &expr_tokens[*idx]);

                    let Some(token_type) =
                        Self::classify_semantic_token(prev, expr_token, next)
                    else {
                        continue;
                    };

                    let line = line0 + expr_token.span.start.line.saturating_sub(1) as u32;
                    let start = col0
                        + expr_start as u32
                        + expr_token.span.start.column.saturating_sub(1) as u32;
                    let len = expr_token
                        .span
                        .end
                        .column
                        .saturating_sub(expr_token.span.start.column)
                        .saturating_add(1) as u32;

                    Self::push_semantic_token(
                        data,
                        last_line,
                        last_start,
                        line,
                        start,
                        len,
                        token_type,
                    );
                }
            }

            Self::push_semantic_token(
                data,
                last_line,
                last_start,
                line0,
                col0 + interp_close as u32,
                1,
                Self::SEM_TOKEN_OPERATOR,
            );

            last_text_start = j;
            i = j;
        }

        if any && last_text_start < chars.len() {
            Self::push_semantic_token(
                data,
                last_line,
                last_start,
                line0,
                col0 + last_text_start as u32,
                (chars.len() - last_text_start) as u32,
                Self::SEM_TOKEN_STRING,
            );
        }

        any
    }

    pub(super) fn build_semantic_tokens(text: &str) -> SemanticTokens {
        let (tokens, _) = lex_cst(text);
        let significant: Vec<usize> = tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| token.kind != "whitespace")
            .map(|(idx, _)| idx)
            .collect();

        let mut data = Vec::new();
        let mut last_line = 0u32;
        let mut last_start = 0u32;

        for (position, token_index) in significant.iter().copied().enumerate() {
            let token = &tokens[token_index];
            let prev = position
                .checked_sub(1)
                .and_then(|prev| significant.get(prev))
                .map(|idx| &tokens[*idx]);
            let next = significant.get(position + 1).map(|idx| &tokens[*idx]);

            if Self::emit_interpolated_string_tokens(token, &mut data, &mut last_line, &mut last_start) {
                continue;
            }

            let Some(token_type) = Self::classify_semantic_token(prev, token, next) else {
                continue;
            };

            let line = token.span.start.line.saturating_sub(1) as u32;
            let start = token.span.start.column.saturating_sub(1) as u32;
            let len = token
                .span
                .end
                .column
                .saturating_sub(token.span.start.column)
                .saturating_add(1) as u32;
            Self::push_semantic_token(&mut data, &mut last_line, &mut last_start, line, start, len, token_type);
        }

        SemanticTokens {
            result_id: None,
            data,
        }
    }
}
