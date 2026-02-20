impl Backend {
    pub(super) const KEYWORDS: &'static [&'static str] = syntax::KEYWORDS_ALL;
    pub(super) const SIGILS: &'static [&'static str] = &[
        "~r//",
        "~u()",
        "~url()",
        "~d()",
        "~t()",
        "~dt()",
        "~tz()",
        "~zdt()",
        "~path[]",
        "~map{}",
        "~set[]",
        "~mat[]",
        "~<html></html>",
    ];

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
                SemanticTokenType::new("aiviArrow"),
                SemanticTokenType::new("aiviPipe"),
                SemanticTokenType::new("aiviBracket"),
                SemanticTokenType::new("aiviUnit"),
                SemanticTokenType::new("aiviSigil"),
                SemanticTokenType::PROPERTY,
                SemanticTokenType::new("aiviDot"),
                SemanticTokenType::new("aiviPathHead"),
                SemanticTokenType::new("aiviPathMid"),
                SemanticTokenType::new("aiviPathTail"),
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
                | "×"
                | "/"
                | "%"
                | "++"
                | "^"
                | "<<"
                | ">>"
                | "~"
                | "<-"
                | "->"
                | "=>"
                | "|>"
                | "<|"
                | "|"
                | "::"
                | ":="
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
            // Only treat the previous expression token as blocking when it is on
            // the same line.  Newlines separate statements in AIVI, so an
            // identifier at the start of a new line is a fresh application head
            // even if the previous line ended with an expression token.
            if Self::is_expression_token(prev) && prev.span.end.line == token.span.start.line {
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
                if token.text == "~" && next.is_some_and(|n| n.kind == "ident" && matches!(n.text.as_str(), "map" | "set" | "mat" | "path")) {
                    Some(Self::SEM_TOKEN_SIGIL)
                } else if token.text == "@" {
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
                if prev.is_some_and(|p| p.kind == "symbol" && p.text == "~") && matches!(token.text.as_str(), "map" | "set" | "mat" | "path") {
                    return Some(Self::SEM_TOKEN_SIGIL);
                }
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
}
