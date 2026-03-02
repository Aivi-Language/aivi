impl Backend {
    pub(super) fn build_formatting_edits(
        text: &str,
        options: aivi::FormatOptions,
    ) -> Vec<TextEdit> {
        let range = Self::full_document_range(text);
        let formatted = aivi::format_text_with_options(text, options);
        vec![TextEdit::new(range, formatted)]
    }

    pub(super) fn full_document_range(text: &str) -> Range {
        let lines: Vec<&str> = text.split('\n').collect();
        let last_line = lines.len().saturating_sub(1) as u32;
        let last_col = lines
            .last()
            .map(|line| line.chars().count() as u32)
            .unwrap_or(0);
        Range::new(Position::new(0, 0), Position::new(last_line, last_col))
    }

    pub(super) fn span_to_range(span: Span) -> Range {
        let start_line = span.start.line.saturating_sub(1) as u32;
        let start_char = span.start.column.saturating_sub(1) as u32;
        let end_line = span.end.line.saturating_sub(1) as u32;
        let end_char = span.end.column as u32;
        Range::new(
            Position::new(start_line, start_char),
            Position::new(end_line, end_char),
        )
    }

    pub(super) fn offset_at(text: &str, position: Position) -> usize {
        let mut offset = 0usize;
        for (line, chunk) in text.split_inclusive('\n').enumerate() {
            if line as u32 == position.line {
                let char_offset = position.character as usize;
                return offset
                    + chunk
                        .chars()
                        .take(char_offset)
                        .map(|c| c.len_utf8())
                        .sum::<usize>();
            }
            offset += chunk.len();
        }
        offset
    }

    pub(super) fn extract_identifier(text: &str, position: Position) -> Option<String> {
        let offset = Self::offset_at(text, position).min(text.len());
        if text.is_empty() {
            return None;
        }

        // Check if we are on a symbol/operator character
        // Note: '.' is excluded because it's part of dotted identifiers (e.g. MyHeap.push)
        // Brackets and delimiters are excluded so hover is not triggered on them.
        fn is_symbol_char(c: char) -> bool {
            !c.is_alphanumeric()
                && c != '_'
                && c != '.'
                && c != ' '
                && c != '\t'
                && c != '\n'
                && c != '\r'
                && !matches!(c, '{' | '}' | '(' | ')' | '[' | ']' | ',' | ';')
        }

        // Helper to check if a char is part of a standard identifier
        fn is_ident_char(c: char) -> bool {
            c.is_alphanumeric() || c == '_' || c == '.'
        }

        // Determine if we are on a symbol or an identifier
        // We look at the character *before* the cursor (if any) and *at* the cursor.
        // If the cursor is at offset, we might be right after the last char of interest.
        //
        // NOTE: `offset` is a byte offset (computed on UTF-8 boundaries). Never slice at
        // `offset - 1`, which may not be a valid UTF-8 boundary for non-ASCII.
        let ch_at = (offset < text.len()).then(|| text[offset..].chars().next()).flatten();
        let ch_before = (offset > 0).then(|| text[..offset].chars().last()).flatten();
        let on_symbol = ch_at.is_some_and(is_symbol_char) || ch_before.is_some_and(is_symbol_char);

        // If we are on a symbol, scan for continuous symbol characters.
        // Note: Aivi might have multi-char operators like <|, |>, ++, etc.
        if on_symbol {
            let mut start = offset;
            // Scan backwards for symbol chars
            while start > 0 {
                let ch = text[..start].chars().last().unwrap();
                if is_symbol_char(ch) {
                    start -= ch.len_utf8();
                } else {
                    break;
                }
            }
            let mut end = offset;
            // Scan forwards for symbol chars
            while end < text.len() {
                let ch = text[end..].chars().next().unwrap();
                if is_symbol_char(ch) {
                    end += ch.len_utf8();
                } else {
                    break;
                }
            }

            let ident = text[start..end].trim();
            if ident.is_empty() {
                None
            } else {
                Some(ident.to_string())
            }
        } else {
            // Existing logic for alphanumeric identifiers
            let mut start = offset;
            while start > 0 {
                let ch = text[..start].chars().last().unwrap();
                if is_ident_char(ch) {
                    start -= ch.len_utf8();
                } else {
                    break;
                }
            }
            let mut end = offset;
            while end < text.len() {
                let ch = text[end..].chars().next().unwrap();
                if is_ident_char(ch) {
                    end += ch.len_utf8();
                } else {
                    break;
                }
            }
            let ident = text[start..end].trim();
            if ident.is_empty() {
                None
            } else {
                Some(ident.to_string())
            }
        }
    }
}
