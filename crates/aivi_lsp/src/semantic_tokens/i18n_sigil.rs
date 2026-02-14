impl Backend {
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
}
