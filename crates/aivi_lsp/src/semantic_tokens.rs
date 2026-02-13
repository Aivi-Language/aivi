use std::collections::{HashMap, HashSet};

use aivi::{lex_cst, syntax, CstToken, Span};
use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
};

use crate::backend::Backend;

impl Backend {
    pub(super) const KEYWORDS: &'static [&'static str] = syntax::KEYWORDS_ALL;
    pub(super) const SIGILS: [&'static str; 5] = ["~r//", "~u()", "~d()", "~dt()", "~html~><~html"];

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

    fn is_record_label(prev: Option<&CstToken>, token: &CstToken, next: Option<&CstToken>) -> bool {
        let Some(next) = next else {
            return false;
        };
        if next.kind != "symbol" || next.text != ":" {
            return false;
        }
        // Disambiguate record labels from type signatures. A record label must appear directly
        // after `{` or `,` in a record field list; type signatures are top-level `name : Type`.
        let is_field_context = prev
            .is_some_and(|prev| prev.kind == "symbol" && matches!(prev.text.as_str(), "{" | ","));
        Self::is_lower_ident(token) && is_field_context
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
                if Self::is_record_label(prev, token, next) {
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

    #[allow(clippy::too_many_arguments)]
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

                    let start_idx = expr_token.span.start.column.saturating_sub(1);
                    let end_idx = expr_token.span.end.column.saturating_sub(1);
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

    fn emit_html_sigil_tokens(
        token: &CstToken,
        data: &mut Vec<SemanticToken>,
        last_line: &mut u32,
        last_start: &mut u32,
    ) -> bool {
        if token.kind != "sigil" {
            return false;
        }
        if !token.text.starts_with("~html~>") || !token.text.ends_with("<~html") {
            return false;
        }
        let chars: Vec<char> = token.text.chars().collect();
        if chars.len() < "~html~><~html".chars().count() {
            return false;
        }

        let line0 = token.span.start.line.saturating_sub(1) as u32;
        let col0 = token.span.start.column.saturating_sub(1) as u32;

        // Map each char index to an absolute (line, col) position in the document.
        let mut pos: Vec<(u32, u32)> = Vec::with_capacity(chars.len() + 1);
        let mut line = line0;
        let mut col = col0;
        for ch in &chars {
            pos.push((line, col));
            if *ch == '\n' {
                line = line.saturating_add(1);
                col = 0;
            } else {
                col = col.saturating_add(1);
            }
        }
        pos.push((line, col));

        let push = |data: &mut Vec<SemanticToken>,
                    last_line: &mut u32,
                    last_start: &mut u32,
                    pos: &[(u32, u32)],
                    start: usize,
                    end: usize,
                    token_type: u32,
                    modifiers: u32| {
            if end <= start || start >= pos.len() {
                return;
            }
            let end = end.min(pos.len().saturating_sub(1));
            let (start_line, start_col) = pos[start];
            let mut i = start;
            while i < end {
                let (line, _) = pos[i];
                if line != start_line {
                    break;
                }
                i += 1;
                if i < chars.len() && chars[i - 1] == '\n' {
                    break;
                }
            }
            if start_line == pos[end].0 {
                let len = (end - start) as u32;
                Self::push_semantic_token(
                    data,
                    last_line,
                    last_start,
                    start_line,
                    start_col,
                    len,
                    token_type,
                    modifiers,
                );
            } else {
                // Should not happen for our tag/attr tokens; keep safe.
                let len = (i - start) as u32;
                Self::push_semantic_token(
                    data,
                    last_line,
                    last_start,
                    start_line,
                    start_col,
                    len,
                    token_type,
                    modifiers,
                );
            }
        };

        // Emit the sigil delimiters as `aivi.sigil`, then highlight HTML-ish tokens inside.
        let prefix_len = "~html~>".chars().count();
        let suffix_len = "<~html".chars().count();
        push(
            data,
            last_line,
            last_start,
            &pos,
            0,
            prefix_len,
            Self::SEM_TOKEN_SIGIL,
            0,
        );

        let emit_embedded_aivi_tokens = |data: &mut Vec<SemanticToken>,
                                         last_line: &mut u32,
                                         last_start: &mut u32,
                                         expr_start: usize,
                                         expr_end: usize| {
            if expr_end <= expr_start || expr_start >= chars.len() {
                return;
            }
            let expr_end = expr_end.min(chars.len());
            let expr_text: String = chars[expr_start..expr_end].iter().collect();
            let expr_chars: Vec<char> = expr_text.chars().collect();
            let mut expr_line_starts: Vec<usize> = Vec::new();
            expr_line_starts.push(0);
            for (idx, ch) in expr_chars.iter().enumerate() {
                if *ch == '\n' {
                    expr_line_starts.push(idx + 1);
                }
            }
            let expr_char_index_from_line_col = |line: usize, col: usize| -> Option<usize> {
                let line_index = line.saturating_sub(1);
                let col_index = col.saturating_sub(1);
                let start = *expr_line_starts.get(line_index)?;
                Some(start + col_index)
            };

            let (tokens, _) = lex_cst(&expr_text);
            let significant: Vec<usize> = tokens
                .iter()
                .enumerate()
                .filter(|(_, token)| token.kind != "whitespace")
                .map(|(idx, _)| idx)
                .collect();
            let signature_lines = Self::signature_lines(&tokens);
            let dotted_paths = Self::dotted_path_roles(&tokens);

            for (position, token_index) in significant.iter().copied().enumerate() {
                let t = &tokens[token_index];
                let prev = position
                    .checked_sub(1)
                    .and_then(|prev| significant.get(prev))
                    .map(|idx| &tokens[*idx]);
                let next = significant.get(position + 1).map(|idx| &tokens[*idx]);
                let token_type = dotted_paths
                    .get(&token_index)
                    .copied()
                    .or_else(|| Self::classify_semantic_token(prev, t, next));
                let Some(token_type) = token_type else {
                    continue;
                };

                let Some(local_start) =
                    expr_char_index_from_line_col(t.span.start.line, t.span.start.column)
                else {
                    continue;
                };
                let global_start = expr_start.saturating_add(local_start);
                let token_chars: Vec<char> = t.text.chars().collect();
                if token_chars.is_empty() {
                    continue;
                }

                let line0 = t.span.start.line.saturating_sub(1) as u32;
                let modifiers = if signature_lines.contains(&line0) {
                    1u32 << Self::SEM_MOD_SIGNATURE
                } else {
                    0
                };

                let mut seg_start = 0usize;
                for (idx, ch) in token_chars.iter().enumerate() {
                    if *ch != '\n' {
                        continue;
                    }
                    if idx > seg_start {
                        let start = global_start.saturating_add(seg_start);
                        let end = global_start.saturating_add(idx);
                        push(
                            data, last_line, last_start, &pos, start, end, token_type, modifiers,
                        );
                    }
                    seg_start = idx + 1;
                }
                if seg_start < token_chars.len() {
                    let start = global_start.saturating_add(seg_start);
                    let end = global_start.saturating_add(token_chars.len());
                    push(
                        data, last_line, last_start, &pos, start, end, token_type, modifiers,
                    );
                }
            }
        };

        let mut i = prefix_len;
        let end_limit = chars.len().saturating_sub(1);

        while i < end_limit {
            // Skip AIVI interpolation blocks in HTML attributes/content: `{ ... }`.
            if chars[i] == '{' {
                let mut depth: i32 = 1;
                let mut j = i + 1;
                let mut in_quote: Option<char> = None;
                while j < end_limit && depth > 0 {
                    let c = chars[j];
                    if let Some(q) = in_quote {
                        if q != '`' && c == '\\' {
                            j = j.saturating_add(2);
                            continue;
                        }
                        if c == q {
                            in_quote = None;
                        }
                        j += 1;
                        continue;
                    }
                    match c {
                        '"' | '\'' | '`' => {
                            in_quote = Some(c);
                        }
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        _ => {}
                    }
                    j += 1;
                }
                // Treat `{ ... }` like JSX: tokenize the embedded AIVI expression.
                if j > i + 1 {
                    emit_embedded_aivi_tokens(
                        data,
                        last_line,
                        last_start,
                        i + 1,
                        j.saturating_sub(1),
                    );
                }
                i = j;
                continue;
            }

            if chars[i] != '<' {
                i += 1;
                continue;
            }

            // Skip comments.
            if i + 3 < end_limit
                && chars[i + 1] == '!'
                && chars[i + 2] == '-'
                && chars[i + 3] == '-'
            {
                let mut j = i + 4;
                while j + 2 < end_limit {
                    if chars[j] == '-' && chars[j + 1] == '-' && chars[j + 2] == '>' {
                        j += 3;
                        break;
                    }
                    j += 1;
                }
                i = j;
                continue;
            }

            let mut j = i + 1;
            if j < end_limit && chars[j] == '/' {
                j += 1;
            }

            // Parse tag name.
            let tag_start = j;
            if tag_start >= end_limit {
                break;
            }
            if !chars[tag_start].is_ascii_alphabetic() {
                i += 1;
                continue;
            }
            j += 1;
            while j < end_limit
                && (chars[j].is_ascii_alphanumeric() || matches!(chars[j], '-' | ':' | '_'))
            {
                j += 1;
            }
            let tag_end = j;
            push(
                data,
                last_line,
                last_start,
                &pos,
                tag_start,
                tag_end,
                Self::SEM_TOKEN_TYPE,
                0,
            );

            // Parse attributes until tag closes.
            while j < end_limit {
                // stop at tag close
                if chars[j] == '>' {
                    j += 1;
                    break;
                }
                if chars[j] == '/' && j + 1 < end_limit && chars[j + 1] == '>' {
                    j += 2;
                    break;
                }
                if chars[j].is_whitespace() {
                    j += 1;
                    continue;
                }
                if chars[j] == '{' {
                    // Something like `<div { ... }>`; skip balanced.
                    break;
                }

                let attr_start = j;
                if !chars[j].is_ascii_alphabetic() && chars[j] != '_' && chars[j] != ':' {
                    j += 1;
                    continue;
                }
                j += 1;
                while j < end_limit
                    && (chars[j].is_ascii_alphanumeric()
                        || matches!(chars[j], '-' | ':' | '_' | '.'))
                {
                    j += 1;
                }
                let attr_end = j;
                push(
                    data,
                    last_line,
                    last_start,
                    &pos,
                    attr_start,
                    attr_end,
                    Self::SEM_TOKEN_PROPERTY,
                    0,
                );

                while j < end_limit && chars[j].is_whitespace() {
                    j += 1;
                }
                if j < end_limit && chars[j] == '=' {
                    j += 1;
                    while j < end_limit && chars[j].is_whitespace() {
                        j += 1;
                    }
                    if j < end_limit && matches!(chars[j], '"' | '\'') {
                        let quote = chars[j];
                        let value_start = j;
                        j += 1;
                        while j < end_limit {
                            if chars[j] == '\\' {
                                j = j.saturating_add(2);
                                continue;
                            }
                            if chars[j] == quote {
                                j += 1;
                                break;
                            }
                            j += 1;
                        }
                        let value_end = j.min(end_limit);
                        push(
                            data,
                            last_line,
                            last_start,
                            &pos,
                            value_start,
                            value_end,
                            Self::SEM_TOKEN_STRING,
                            0,
                        );
                    } else if j < end_limit && chars[j] == '{' {
                        // AIVI expression; skip balanced.
                        break;
                    } else {
                        // Unquoted literal.
                        let value_start = j;
                        while j < end_limit && !chars[j].is_whitespace() && chars[j] != '>' {
                            j += 1;
                        }
                        push(
                            data,
                            last_line,
                            last_start,
                            &pos,
                            value_start,
                            j,
                            Self::SEM_TOKEN_STRING,
                            0,
                        );
                    }
                }
            }

            i = j;
        }

        // Emit the closing delimiter last so semantic tokens stay in document order.
        push(
            data,
            last_line,
            last_start,
            &pos,
            chars.len().saturating_sub(suffix_len),
            chars.len(),
            Self::SEM_TOKEN_SIGIL,
            0,
        );

        true
    }

    fn emit_i18n_sigil_tokens(
        token: &CstToken,
        data: &mut Vec<SemanticToken>,
        last_line: &mut u32,
        last_start: &mut u32,
    ) -> bool {
        if token.kind != "sigil" {
            return false;
        }

        // `~k"..."` and `~m"..."` should look like normal strings, with only the sigil
        // delimiters highlighted as `aivi.sigil`.
        if !(token.text.starts_with("~k\"") || token.text.starts_with("~m\"")) {
            return false;
        }
        if token.text.contains('\n') || token.span.start.line != token.span.end.line {
            // Keep it simple; LSP semantic tokens cannot span lines anyway.
            return false;
        }

        let chars: Vec<char> = token.text.chars().collect();
        let prefix_len = 3usize; // "~k\"" or "~m\""
        if chars.len() <= prefix_len {
            return false;
        }

        // Find the closing quote, respecting backslash escapes.
        let mut i = prefix_len;
        let mut end_quote = None;
        while i < chars.len() {
            let ch = chars[i];
            if ch == '\\' {
                i = i.saturating_add(2);
                continue;
            }
            if ch == '"' {
                end_quote = Some(i);
                break;
            }
            i += 1;
        }
        let Some(end_quote) = end_quote else {
            return false;
        };

        let start_line = token.span.start.line.saturating_sub(1) as u32;
        let col0 = token.span.start.column.saturating_sub(1) as u32;

        let push = |data: &mut Vec<SemanticToken>,
                    last_line: &mut u32,
                    last_start: &mut u32,
                    start_col: u32,
                    len: u32,
                    token_type: u32| {
            if len == 0 {
                return;
            }
            Self::push_semantic_token(
                data,
                last_line,
                last_start,
                start_line,
                start_col,
                len,
                token_type,
                0,
            );
        };

        // Prefix: `~k"` / `~m"`
        push(
            data,
            last_line,
            last_start,
            col0,
            prefix_len as u32,
            Self::SEM_TOKEN_SIGIL,
        );
        // Content: `...`
        push(
            data,
            last_line,
            last_start,
            col0.saturating_add(prefix_len as u32),
            end_quote.saturating_sub(prefix_len) as u32,
            Self::SEM_TOKEN_STRING,
        );
        // Suffix: `"`, plus any trailing sigil suffix flags if present.
        push(
            data,
            last_line,
            last_start,
            col0.saturating_add(end_quote as u32),
            chars.len().saturating_sub(end_quote) as u32,
            Self::SEM_TOKEN_SIGIL,
        );

        true
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

            if Self::emit_html_sigil_tokens(token, &mut data, &mut last_line, &mut last_start) {
                continue;
            }

            if Self::emit_i18n_sigil_tokens(token, &mut data, &mut last_line, &mut last_start) {
                continue;
            }

            let token_type = dotted_paths
                .get(&token_index)
                .copied()
                .or_else(|| Self::classify_semantic_token(prev, token, next));
            let Some(token_type) = token_type else {
                continue;
            };

            let start_line = token.span.start.line.saturating_sub(1) as u32;
            let start_col = token.span.start.column.saturating_sub(1) as u32;

            if token.span.start.line != token.span.end.line {
                // LSP semantic tokens cannot span multiple lines.
                for (idx, part) in token.text.split('\n').enumerate() {
                    let line = start_line.saturating_add(idx as u32);
                    let start = if idx == 0 { start_col } else { 0 };
                    let len = part.chars().count() as u32;
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
                continue;
            }

            let line = start_line;
            let start = start_col;
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
