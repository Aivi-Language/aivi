impl Backend {
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
        let lambda_heads = Self::lambda_head_positions(&significant, &tokens);

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

            if Self::emit_gtk_sigil_tokens(token, &mut data, &mut last_line, &mut last_start) {
                continue;
            }

            if Self::emit_i18n_sigil_tokens(token, &mut data, &mut last_line, &mut last_start) {
                continue;
            }

            if Self::emit_paren_sigil_tokens(token, &mut data, &mut last_line, &mut last_start) {
                continue;
            }

            let token_type = dotted_paths
                .get(&token_index)
                .copied()
                .or_else(|| {
                    if lambda_heads.contains(&token_index) {
                        return Some(Self::SEM_TOKEN_VARIABLE);
                    }
                    Self::classify_semantic_token(prev, token, next)
                });
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
