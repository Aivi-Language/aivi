impl Backend {
    /// For any sigil using `(...)` delimiters (e.g. `~url(...)`, `~u(...)`, `~d(...)`, etc.),
    /// emit the opening `~name(` and closing `)` as `aiviSigil` (dark gray) and the inner
    /// content as a plain string token, so it gets the default string colour.
    pub(super) fn emit_paren_sigil_tokens(
        token: &CstToken,
        data: &mut Vec<SemanticToken>,
        last_line: &mut u32,
        last_start: &mut u32,
    ) -> bool {
        if token.kind != "sigil" {
            return false;
        }

        // Must start with `~` followed by a lowercase letter and contain a `(` delimiter.
        let text = &token.text;
        if !text.starts_with('~') {
            return false;
        }
        // Find the opening `(`.
        let open_paren = match text.find('(') {
            Some(pos) => pos,
            None => return false,
        };
        // The sigil name between `~` and `(` must be all alphanumeric / underscore.
        let name_part = &text[1..open_paren];
        if name_part.is_empty()
            || !name_part
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_lowercase())
            || !name_part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return false;
        }

        if token.span.start.line != token.span.end.line {
            // LSP semantic tokens cannot span multiple lines.
            return false;
        }

        let chars: Vec<char> = text.chars().collect();
        let prefix_len = open_paren + 1; // everything up to and including `(`

        // Find the closing `)` respecting backslash escapes.
        let mut i = prefix_len;
        let mut close_paren = None;
        while i < chars.len() {
            let ch = chars[i];
            if ch == '\\' {
                i = i.saturating_add(2);
                continue;
            }
            if ch == ')' {
                close_paren = Some(i);
                break;
            }
            i += 1;
        }
        let Some(close_paren) = close_paren else {
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

        // Prefix: `~name(`
        push(
            data,
            last_line,
            last_start,
            col0,
            prefix_len as u32,
            Self::SEM_TOKEN_SIGIL,
        );
        // Content between `(` and `)`.
        let content_len = close_paren.saturating_sub(prefix_len) as u32;
        push(
            data,
            last_line,
            last_start,
            col0.saturating_add(prefix_len as u32),
            content_len,
            Self::SEM_TOKEN_STRING,
        );
        // Suffix: `)` plus any trailing flags.
        push(
            data,
            last_line,
            last_start,
            col0.saturating_add(close_paren as u32),
            chars.len().saturating_sub(close_paren) as u32,
            Self::SEM_TOKEN_SIGIL,
        );

        true
    }
}
