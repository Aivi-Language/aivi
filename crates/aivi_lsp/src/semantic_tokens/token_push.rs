impl Backend {
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
}
