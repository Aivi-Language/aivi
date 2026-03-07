impl Backend {
    fn emit_raw_text_sigil_tokens(
        token: &CstToken,
        data: &mut Vec<SemanticToken>,
        last_line: &mut u32,
        last_start: &mut u32,
    ) -> bool {
        if token.kind != "sigil" || !token.text.starts_with("~`") {
            return false;
        }

        let lines: Vec<&str> = token.text.split('\n').collect();
        if lines.is_empty() {
            return false;
        }

        let start_line = token.span.start.line.saturating_sub(1) as u32;
        let start_col = token.span.start.column.saturating_sub(1) as u32;

        let push = |data: &mut Vec<SemanticToken>,
                    last_line: &mut u32,
                    last_start: &mut u32,
                    line: u32,
                    col: u32,
                    len: u32,
                    token_type: u32| {
            if len == 0 {
                return;
            }
            Self::push_semantic_token(data, last_line, last_start, line, col, len, token_type, 0);
        };

        let first_line = lines[0];
        let first_line_chars = first_line.chars().count() as u32;
        let embedded_language = if lines.len() > 1 && first_line.len() >= 2 {
            aivi::raw_text_sigil::raw_text_language_line(&first_line[2..])
        } else {
            None
        };

        if lines.len() == 1 {
            push(
                data,
                last_line,
                last_start,
                start_line,
                start_col,
                2,
                Self::SEM_TOKEN_SIGIL,
            );
            push(
                data,
                last_line,
                last_start,
                start_line,
                start_col.saturating_add(2),
                first_line_chars.saturating_sub(3),
                Self::SEM_TOKEN_STRING,
            );
            push(
                data,
                last_line,
                last_start,
                start_line,
                start_col.saturating_add(first_line_chars.saturating_sub(1)),
                1,
                Self::SEM_TOKEN_SIGIL,
            );
            return true;
        }

        push(
            data,
            last_line,
            last_start,
            start_line,
            start_col,
            2,
            Self::SEM_TOKEN_SIGIL,
        );
        push(
            data,
            last_line,
            last_start,
            start_line,
            start_col.saturating_add(2),
            first_line_chars.saturating_sub(2),
            if embedded_language.is_some() {
                Self::SEM_TOKEN_SIGIL
            } else {
                Self::SEM_TOKEN_STRING
            },
        );

        if embedded_language.is_none() {
            for (line_offset, line_text) in lines
                .iter()
                .enumerate()
                .skip(1)
                .take(lines.len().saturating_sub(2))
            {
                push(
                    data,
                    last_line,
                    last_start,
                    start_line.saturating_add(line_offset as u32),
                    0,
                    line_text.chars().count() as u32,
                    Self::SEM_TOKEN_STRING,
                );
            }

            let last_line_text = lines[lines.len() - 1];
            let last_line_chars = last_line_text.chars().count() as u32;
            push(
                data,
                last_line,
                last_start,
                start_line.saturating_add((lines.len() - 1) as u32),
                0,
                last_line_chars.saturating_sub(1),
                Self::SEM_TOKEN_STRING,
            );
            push(
                data,
                last_line,
                last_start,
                start_line.saturating_add((lines.len() - 1) as u32),
                last_line_chars.saturating_sub(1),
                1,
                Self::SEM_TOKEN_SIGIL,
            );
            return true;
        }

        let last_line_text = lines[lines.len() - 1];
        let last_line_chars = last_line_text.chars().count() as u32;
        push(
            data,
            last_line,
            last_start,
            start_line.saturating_add((lines.len() - 1) as u32),
            last_line_chars.saturating_sub(1),
            1,
            Self::SEM_TOKEN_SIGIL,
        );

        true
    }
}
