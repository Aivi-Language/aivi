use std::collections::{HashMap, HashSet};

use aivi::{lex_cst, syntax, CstToken, Span};
use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
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
    pub(super) const SEM_TOKEN_ARROW: u32 = 9;
    pub(super) const SEM_TOKEN_PIPE: u32 = 10;
    pub(super) const SEM_TOKEN_BRACKET: u32 = 11;
    pub(super) const SEM_TOKEN_UNIT: u32 = 12;
    pub(super) const SEM_TOKEN_SIGIL: u32 = 13;
    pub(super) const SEM_TOKEN_PROPERTY: u32 = 14;
    pub(super) const SEM_TOKEN_DOT: u32 = 15;
    pub(super) const SEM_TOKEN_PATH_HEAD: u32 = 16;
    pub(super) const SEM_TOKEN_PATH_MID: u32 = 17;
    pub(super) const SEM_TOKEN_PATH_TAIL: u32 = 18;
    pub(super) const SEM_TOKEN_TYPE_PARAMETER: u32 = 19;

    pub(super) const SEM_MOD_SIGNATURE: u32 = 0;

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
                SemanticTokenType::new("aivi.arrow"),
                SemanticTokenType::new("aivi.pipe"),
                SemanticTokenType::new("aivi.bracket"),
                SemanticTokenType::new("aivi.unit"),
                SemanticTokenType::new("aivi.sigil"),
                SemanticTokenType::PROPERTY,
                SemanticTokenType::new("aivi.dot"),
                SemanticTokenType::new("aivi.path.head"),
                SemanticTokenType::new("aivi.path.mid"),
                SemanticTokenType::new("aivi.path.tail"),
                SemanticTokenType::TYPE_PARAMETER,
            ],
            token_modifiers: vec![SemanticTokenModifier::new("signature")],
        }
    }

    fn is_adjacent_span(left: &Span, right: &Span) -> bool {
        left.end.line == right.start.line && left.end.column.saturating_add(1) == right.start.column
    }

    fn is_arrow_symbol(symbol: &str) -> bool {
        matches!(symbol, "=>" | "<-" | "->")
    }

    fn is_pipe_symbol(symbol: &str) -> bool {
        matches!(symbol, "|>" | "<|" | "|")
    }

    fn is_bracket_symbol(symbol: &str) -> bool {
        matches!(symbol, "(" | ")" | "[" | "]" | "{" | "}")
    }

    fn is_lower_ident(token: &CstToken) -> bool {
        token.kind == "ident"
            && token
                .text
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_lowercase())
    }

    fn is_type_parameter_name(text: &str) -> bool {
        text.len() == 1 && text.chars().all(|ch| ch.is_ascii_uppercase())
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
                | "|"
                | "::"
                | ".."
                | "..."
                | ":"
        )
    }

    fn dotted_path_roles(tokens: &[CstToken]) -> HashMap<usize, u32> {
        let mut roles = HashMap::new();
        let mut index = 0;
        while index < tokens.len() {
            if tokens[index].kind != "ident" {
                index += 1;
                continue;
            }
            let mut ident_indices = vec![index];
            let mut current = index;
            loop {
                let dot_index = current + 1;
                let next_index = current + 2;
                if next_index >= tokens.len() {
                    break;
                }
                let dot = &tokens[dot_index];
                let next = &tokens[next_index];
                if dot.kind != "symbol" || dot.text != "." {
                    break;
                }
                if next.kind != "ident" {
                    break;
                }
                if !Self::is_adjacent_span(&tokens[current].span, &dot.span)
                    || !Self::is_adjacent_span(&dot.span, &next.span)
                {
                    break;
                }
                ident_indices.push(next_index);
                current = next_index;
            }
            if ident_indices.len() > 1 {
                let has_type_segment = ident_indices.iter().any(|idx| {
                    tokens[*idx]
                        .text
                        .chars()
                        .next()
                        .is_some_and(|ch| ch.is_ascii_uppercase())
                });
                if !has_type_segment {
                    let last = ident_indices.len().saturating_sub(1);
                    for (pos, idx) in ident_indices.iter().enumerate() {
                        let role = if pos == last {
                            Self::SEM_TOKEN_PATH_TAIL
                        } else if pos + 1 == last {
                            Self::SEM_TOKEN_PATH_MID
                        } else {
                            Self::SEM_TOKEN_PATH_HEAD
                        };
                        roles.insert(*idx, role);
                    }
                }
                index = ident_indices[ident_indices.len() - 1].saturating_add(1);
            } else {
                index += 1;
            }
        }
        roles
    }

    fn is_record_label(token: &CstToken, next: Option<&CstToken>) -> bool {
        let Some(next) = next else {
            return false;
        };
        if next.kind != "symbol" || next.text != ":" {
            return false;
        }
        Self::is_lower_ident(token)
    }

    fn is_expression_token(token: &CstToken) -> bool {
        match token.kind.as_str() {
            "ident" => !Self::KEYWORDS.contains(&token.text.as_str()),
            "number" | "string" | "sigil" => true,
            "symbol" => matches!(token.text.as_str(), ")" | "]" | "}"),
            _ => false,
        }
    }

    fn is_expression_start(current: &CstToken, next: &CstToken) -> bool {
        match next.kind.as_str() {
            "ident" | "number" | "string" | "sigil" => true,
            "symbol" => {
                if matches!(next.text.as_str(), "(" | "[" | "{") {
                    return true;
                }
                if next.text == "." && !Self::is_adjacent_span(&current.span, &next.span) {
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    fn is_application_head(
        prev: Option<&CstToken>,
        token: &CstToken,
        next: Option<&CstToken>,
    ) -> bool {
        if !Self::is_lower_ident(token) {
            return false;
        }
        let Some(next) = next else {
            return false;
        };
        if !Self::is_expression_start(token, next) {
            return false;
        }
        if let Some(prev) = prev {
            if Self::is_expression_token(prev) {
                return false;
            }
            if prev.kind == "symbol"
                && prev.text == "."
                && Self::is_adjacent_span(&prev.span, &token.span)
            {
                return false;
            }
        }
        true
    }

    fn classify_semantic_token(
        prev: Option<&CstToken>,
        token: &CstToken,
        next: Option<&CstToken>,
    ) -> Option<u32> {
        match token.kind.as_str() {
            "comment" => Some(Self::SEM_TOKEN_COMMENT),
            "string" => Some(Self::SEM_TOKEN_STRING),
            "sigil" => Some(Self::SEM_TOKEN_SIGIL),
            "number" => Some(Self::SEM_TOKEN_NUMBER),
            "symbol" => {
                if token.text == "@" {
                    Some(Self::SEM_TOKEN_DECORATOR)
                } else if token.text == "." {
                    Some(Self::SEM_TOKEN_DOT)
                } else if Self::is_arrow_symbol(&token.text) {
                    Some(Self::SEM_TOKEN_ARROW)
                } else if Self::is_pipe_symbol(&token.text) {
                    Some(Self::SEM_TOKEN_PIPE)
                } else if Self::is_bracket_symbol(&token.text) {
                    Some(Self::SEM_TOKEN_BRACKET)
                } else if Self::is_operator_symbol(&token.text) {
                    Some(Self::SEM_TOKEN_OPERATOR)
                } else {
                    None
                }
            }
            "ident" => {
                if prev.is_some_and(|prev| Self::is_unit_suffix(prev, token)) {
                    return Some(Self::SEM_TOKEN_UNIT);
                }
                if Self::is_type_parameter_name(&token.text) {
                    return Some(Self::SEM_TOKEN_TYPE_PARAMETER);
                }
                if token.text == "_" {
                    return Some(Self::SEM_TOKEN_KEYWORD);
                }
                if Self::KEYWORDS.contains(&token.text.as_str()) {
                    return Some(Self::SEM_TOKEN_KEYWORD);
                }
                if prev.is_some_and(|prev| prev.kind == "symbol" && prev.text == "@") {
                    return Some(Self::SEM_TOKEN_DECORATOR);
                }
                if Self::is_record_label(token, next) {
                    return Some(Self::SEM_TOKEN_PROPERTY);
                }
                if matches!(
                    next,
                    Some(next) if next.kind == "symbol" && (next.text == ":" || next.text == "=")
                ) {
                    return Some(Self::SEM_TOKEN_FUNCTION);
                }
                if Self::is_application_head(prev, token, next) {
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

    fn is_unit_suffix(prev: &CstToken, token: &CstToken) -> bool {
        if prev.kind != "number" || token.kind != "ident" {
            return false;
        }
        if prev.span.end.line != token.span.start.line {
            return false;
        }
        prev.span.end.column.saturating_add(1) == token.span.start.column
    }

    fn signature_lines(tokens: &[CstToken]) -> HashSet<u32> {
        let mut lines = HashSet::new();
        let mut index = 0;
        while index < tokens.len() {
            let line = tokens[index].span.start.line;
            let mut sig_tokens: Vec<usize> = Vec::new();
            while index < tokens.len() && tokens[index].span.start.line == line {
                if tokens[index].kind != "whitespace" {
                    sig_tokens.push(index);
                }
                index += 1;
            }
            if sig_tokens.len() < 2 {
                continue;
            }
            let first = &tokens[sig_tokens[0]];
            let second = &tokens[sig_tokens[1]];
            if first.kind != "ident" || second.kind != "symbol" || second.text != ":" {
                continue;
            }
            let Some(first_ch) = first.text.chars().next() else {
                continue;
            };
            if !first_ch.is_ascii_lowercase() {
                continue;
            }
            if first.span.end.column.saturating_add(1) == second.span.start.column {
                continue;
            }
            lines.insert(line.saturating_sub(1) as u32);
        }
        lines
    }

    fn push_semantic_token(
        data: &mut Vec<SemanticToken>,
        last_line: &mut u32,
        last_start: &mut u32,
        line: u32,
        start: u32,
        len: u32,
        token_type: u32,
        token_modifiers_bitset: u32,
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
            token_modifiers_bitset,
        });

        *last_line = line;
        *last_start = start;
    }

    fn emit_interpolated_string_tokens(
        token: &CstToken,
        data: &mut Vec<SemanticToken>,
        last_line: &mut u32,
        last_start: &mut u32,
        signature_lines: &std::collections::HashSet<u32>,
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
                let modifiers = if signature_lines.contains(&line0) {
                    1u32 << Self::SEM_MOD_SIGNATURE
                } else {
                    0
                };
                Self::push_semantic_token(
                    data,
                    last_line,
                    last_start,
                    line0,
                    col0 + last_text_start as u32,
                    (interp_open - last_text_start) as u32,
                    Self::SEM_TOKEN_STRING,
                    modifiers,
                );
            }

            let modifiers = if signature_lines.contains(&line0) {
                1u32 << Self::SEM_MOD_SIGNATURE
            } else {
                0
            };
            Self::push_semantic_token(
                data,
                last_line,
                last_start,
                line0,
                col0 + interp_open as u32,
                1,
                Self::SEM_TOKEN_OPERATOR,
                modifiers,
            );

            let expr_start = interp_open + 1;
            let expr_end = interp_close;
            if expr_end > expr_start {
                let expr_text_raw: String = chars[expr_start..expr_end].iter().collect();
                let (expr_text, expr_map) = Self::build_interpolated_expr_source(&expr_text_raw);
                let (expr_tokens, _) = lex_cst(&expr_text);
                let expr_dotted = Self::dotted_path_roles(&expr_tokens);
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

                    let token_type = expr_dotted
                        .get(&token_index)
                        .copied()
                        .or_else(|| Self::classify_semantic_token(prev, expr_token, next));
                    let Some(token_type) = token_type else {
                        continue;
                    };

                    let start_idx = expr_token.span.start.column.saturating_sub(1) as usize;
                    let end_idx = expr_token.span.end.column.saturating_sub(1) as usize;
                    if start_idx >= expr_map.len() || end_idx >= expr_map.len() {
                        continue;
                    }
                    let raw_start = expr_map[start_idx];
                    let raw_end = expr_map[end_idx];
                    let line = line0 + expr_token.span.start.line.saturating_sub(1) as u32;
                    let start = col0 + expr_start as u32 + raw_start as u32;
                    let len = raw_end.saturating_sub(raw_start).saturating_add(1) as u32;
                    let modifiers = if signature_lines.contains(&line) {
                        1u32 << Self::SEM_MOD_SIGNATURE
                    } else {
                        0
                    };

                    Self::push_semantic_token(
                        data, last_line, last_start, line, start, len, token_type, modifiers,
                    );
                }
            }

            let modifiers = if signature_lines.contains(&line0) {
                1u32 << Self::SEM_MOD_SIGNATURE
            } else {
                0
            };
            Self::push_semantic_token(
                data,
                last_line,
                last_start,
                line0,
                col0 + interp_close as u32,
                1,
                Self::SEM_TOKEN_OPERATOR,
                modifiers,
            );

            last_text_start = j;
            i = j;
        }

        if any && last_text_start < chars.len() {
            let modifiers = if signature_lines.contains(&line0) {
                1u32 << Self::SEM_MOD_SIGNATURE
            } else {
                0
            };
            Self::push_semantic_token(
                data,
                last_line,
                last_start,
                line0,
                col0 + last_text_start as u32,
                (chars.len() - last_text_start) as u32,
                Self::SEM_TOKEN_STRING,
                modifiers,
            );
        }

        any
    }

    fn build_interpolated_expr_source(text: &str) -> (String, Vec<usize>) {
        let chars: Vec<char> = text.chars().collect();
        let mut out = String::new();
        let mut map = Vec::new();
        let mut i = 0usize;
        while i < chars.len() {
            let ch = chars[i];
            if ch == '\\' && i + 1 < chars.len() {
                let next = chars[i + 1];
                if matches!(next, '"' | '\\' | '{' | '}') {
                    out.push(next);
                    map.push(i + 1);
                    i += 2;
                    continue;
                }
            }
            out.push(ch);
            map.push(i);
            i += 1;
        }
        (out, map)
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
        let signature_lines = Self::signature_lines(&tokens);
        let dotted_paths = Self::dotted_path_roles(&tokens);

        for (position, token_index) in significant.iter().copied().enumerate() {
            let token = &tokens[token_index];
            let prev = position
                .checked_sub(1)
                .and_then(|prev| significant.get(prev))
                .map(|idx| &tokens[*idx]);
            let next = significant.get(position + 1).map(|idx| &tokens[*idx]);

            if Self::emit_interpolated_string_tokens(
                token,
                &mut data,
                &mut last_line,
                &mut last_start,
                &signature_lines,
            ) {
                continue;
            }

            let token_type = dotted_paths
                .get(&token_index)
                .copied()
                .or_else(|| Self::classify_semantic_token(prev, token, next));
            let Some(token_type) = token_type else {
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
            let modifiers = if signature_lines.contains(&line) {
                1u32 << Self::SEM_MOD_SIGNATURE
            } else {
                0
            };
            Self::push_semantic_token(
                &mut data,
                &mut last_line,
                &mut last_start,
                line,
                start,
                len,
                token_type,
                modifiers,
            );
        }

        SemanticTokens {
            result_id: None,
            data,
        }
    }
}
