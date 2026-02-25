impl Backend {
    fn emit_gtk_sigil_tokens(
        token: &CstToken,
        data: &mut Vec<SemanticToken>,
        last_line: &mut u32,
        last_start: &mut u32,
    ) -> bool {
        if token.kind != "sigil" {
            return false;
        }
        if !token.text.starts_with("~<gtk>") || !token.text.ends_with("</gtk>") {
            return false;
        }
        let chars: Vec<char> = token.text.chars().collect();
        if chars.len() < "~<gtk></gtk>".chars().count() {
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

        // Emit the sigil delimiters as `aivi.sigil`, then highlight XML-ish tokens inside.
        let prefix_len = "~<gtk>".chars().count();
        let suffix_len = "</gtk>".chars().count();
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
        let end_limit = chars.len().saturating_sub(suffix_len);
        let mut in_style_text_content = false;

        while i < end_limit {
            // Skip AIVI interpolation blocks in GTK/XML attributes/content: `{ ... }`.
            if chars[i] == '{' && !in_style_text_content {
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
            let mut is_closing_tag = false;
            if j < end_limit && chars[j] == '/' {
                is_closing_tag = true;
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
            while j < end_limit && (chars[j].is_ascii_alphanumeric() || matches!(chars[j], '-' | ':' | '_')) {
                j += 1;
            }
            let tag_end = j;
            let tag_name: String = chars[tag_start..tag_end].iter().collect();
            let is_style_tag = tag_name.eq_ignore_ascii_case("style");
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
            let mut self_closing_tag = false;
            while j < end_limit {
                // stop at tag close
                if chars[j] == '>' {
                    j += 1;
                    break;
                }
                if chars[j] == '/' && j + 1 < end_limit && chars[j + 1] == '>' {
                    self_closing_tag = true;
                    j += 2;
                    break;
                }
                if chars[j].is_whitespace() {
                    j += 1;
                    continue;
                }
                if chars[j] == '{' {
                    // Something like `<object { ... }>`; skip balanced.
                    break;
                }

                let attr_start = j;
                if !chars[j].is_ascii_alphabetic() && chars[j] != '_' && chars[j] != ':' {
                    j += 1;
                    continue;
                }
                j += 1;
                while j < end_limit
                    && (chars[j].is_ascii_alphanumeric() || matches!(chars[j], '-' | ':' | '_' | '.'))
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

            if is_style_tag {
                if is_closing_tag {
                    in_style_text_content = false;
                } else {
                    in_style_text_content = !self_closing_tag;
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
}
