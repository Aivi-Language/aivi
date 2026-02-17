fn binary_prec(op: &str) -> u8 {
    match op {
        "<|" | "|>" => 1,
        "||" => 2,
        "&&" => 3,
        "==" | "!=" | "<" | ">" | "<=" | ">=" => 4,
        ".." => 5,
        "++" => 6,
        "+" | "-" => 6,
        "*" | "×" | "/" | "%" => 7,
        _ => 0,
    }
}

fn merge_span(start: Span, end: Span) -> Span {
    Span {
        start: start.start,
        end: end.end,
    }
}

fn shift_span(span: &Span, line_offset: usize, col_offset: usize) -> Span {
    Span {
        start: Position {
            line: span.start.line + line_offset,
            column: span.start.column + col_offset,
        },
        end: Position {
            line: span.end.line + line_offset,
            column: span.end.column + col_offset,
        },
    }
}

fn strip_text_literal_quotes(text: &str) -> Option<&str> {
    let inner = text.strip_prefix('"')?;
    Some(inner.strip_suffix('"').unwrap_or(inner))
}

fn decode_escape(ch: char) -> Option<char> {
    match ch {
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' => Some('\t'),
        '\\' => Some('\\'),
        '"' => Some('"'),
        '{' => Some('{'),
        '}' => Some('}'),
        _ => None,
    }
}

fn decode_text_literal(text: &str) -> Option<String> {
    let inner = strip_text_literal_quotes(text)?;
    let mut out = String::new();
    let chars: Vec<char> = inner.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\\' && i + 1 < chars.len() {
            let esc = chars[i + 1];
            out.push(decode_escape(esc).unwrap_or(esc));
            i += 2;
            continue;
        }
        out.push(ch);
        i += 1;
    }
    Some(out)
}

fn span_in_text_literal(token_span: &Span, start: usize, end: usize) -> Span {
    let line = token_span.start.line;
    let base_col = token_span.start.column + 1;
    let start_col = base_col + start;
    let end_col = if end > start {
        base_col + end - 1
    } else {
        start_col
    };
    Span {
        start: Position {
            line,
            column: start_col,
        },
        end: Position {
            line,
            column: end_col,
        },
    }
}

fn find_interpolation_close(remainder: &str) -> Option<usize> {
    let (decoded, raw_map) = decode_interpolation_source_with_map(remainder);
    let (tokens, _) = lex(&decoded);
    let mut depth = 0usize;
    for token in tokens {
        if token.kind != "symbol" {
            continue;
        }
        match token.text.as_str() {
            "{" => depth += 1,
            "}" => {
                if depth == 0 {
                    let decoded_index = decoded_char_index(
                        &decoded,
                        token.span.start.line,
                        token.span.start.column,
                    )?;
                    return raw_map.get(decoded_index).copied();
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

fn decode_interpolation_source_with_map(raw: &str) -> (String, Vec<usize>) {
    let raw_chars: Vec<char> = raw.chars().collect();
    let mut decoded = String::new();
    let mut raw_map = Vec::new();
    let mut i = 0usize;
    while i < raw_chars.len() {
        let ch = raw_chars[i];
        if ch == '\\' && i + 1 < raw_chars.len() {
            let esc = raw_chars[i + 1];
            if matches!(esc, '\\' | '"' | '{' | '}') {
                decoded.push(esc);
                raw_map.push(i + 1);
                i += 2;
                continue;
            }
        }
        decoded.push(ch);
        raw_map.push(i);
        i += 1;
    }
    (decoded, raw_map)
}

fn decoded_char_index(text: &str, line: usize, column: usize) -> Option<usize> {
    if line == 0 || column == 0 {
        return None;
    }
    let mut line_offsets = vec![0usize];
    let mut idx = 0usize;
    for ch in text.chars() {
        idx += 1;
        if ch == '\n' {
            line_offsets.push(idx);
        }
    }
    let line_start = *line_offsets.get(line - 1)?;
    Some(line_start + (column - 1))
}

fn map_span_columns(span: &Span, raw_map: &[usize]) -> Span {
    let start_idx = span.start.column.saturating_sub(1);
    let end_idx = span.end.column.saturating_sub(1);
    let start_raw = raw_map.get(start_idx).copied().unwrap_or(start_idx);
    let end_raw = raw_map.get(end_idx).copied().unwrap_or(end_idx);
    Span {
        start: Position {
            line: span.start.line,
            column: start_raw + 1,
        },
        end: Position {
            line: span.end.line,
            column: end_raw + 1,
        },
    }
}

fn expr_span(expr: &Expr) -> Span {
    match expr {
        Expr::Ident(name) => name.span.clone(),
        Expr::Literal(literal) => literal_span(literal),
        Expr::UnaryNeg { span, .. } => span.clone(),
        Expr::Suffixed { span, .. } => span.clone(),
        Expr::TextInterpolate { span, .. } => span.clone(),
        Expr::List { span, .. }
        | Expr::Tuple { span, .. }
        | Expr::Record { span, .. }
        | Expr::PatchLit { span, .. }
        | Expr::FieldAccess { span, .. }
        | Expr::FieldSection { span, .. }
        | Expr::Index { span, .. }
        | Expr::Call { span, .. }
        | Expr::Lambda { span, .. }
        | Expr::Match { span, .. }
        | Expr::If { span, .. }
        | Expr::Binary { span, .. }
        | Expr::Block { span, .. } => span.clone(),
        Expr::Raw { span, .. } => span.clone(),
    }
}

fn pattern_span(pattern: &Pattern) -> Span {
    match pattern {
        Pattern::Wildcard(span) => span.clone(),
        Pattern::Ident(name) => name.span.clone(),
        Pattern::SubjectIdent(name) => name.span.clone(),
        Pattern::Literal(literal) => literal_span(literal),
        Pattern::At { span, .. } => span.clone(),
        Pattern::Constructor { span, .. }
        | Pattern::Tuple { span, .. }
        | Pattern::List { span, .. }
        | Pattern::Record { span, .. } => span.clone(),
    }
}

fn type_span(ty: &TypeExpr) -> Span {
    match ty {
        TypeExpr::Name(name) => name.span.clone(),
        TypeExpr::And { span, .. }
        | TypeExpr::Apply { span, .. }
        | TypeExpr::Func { span, .. }
        | TypeExpr::Record { span, .. }
        | TypeExpr::Tuple { span, .. }
        | TypeExpr::Star { span }
        | TypeExpr::Unknown { span } => span.clone(),
    }
}

fn literal_span(literal: &Literal) -> Span {
    match literal {
        Literal::Number { span, .. }
        | Literal::String { span, .. }
        | Literal::Sigil { span, .. }
        | Literal::Bool { span, .. }
        | Literal::DateTime { span, .. } => span.clone(),
    }
}

fn parse_sigil_text(text: &str) -> Option<(String, String, String)> {
    let mut iter = text.chars();
    if iter.next()? != '~' {
        return None;
    }
    let mut tag = String::new();
    let mut open = None;
    for ch in iter.by_ref() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            tag.push(ch);
            continue;
        }
        open = Some(ch);
        break;
    }
    let open = open?;
    
    // Special handling for ~html~> ... <~html delimiter
    if open == '~' {
        // Expect '>' after '~'
        let next_ch = iter.next()?;
        if next_ch != '>' {
            return None;
        }
        // The body is everything until we find the closing <~TAGNAME sequence
        let closing_marker = format!("<~{}", tag);
        let remainder: String = iter.collect();
        if let Some(pos) = remainder.find(&closing_marker) {
            let body = remainder[..pos].to_string();
            // No flags for this delimiter type
            return Some((tag, body, String::new()));
        } else {
            // If no closing marker found, take everything
            return Some((tag, remainder, String::new()));
        }
    }
    
    let close = match open {
        '/' => '/',
        '"' => '"',
        '(' => ')',
        '[' => ']',
        '{' => '}',
        _ => return None,
    };
    let mut body = String::new();
    // Some sigils may contain nested `{ ... }` splices (notably `~html~> ... <~html`).
    // For other delimiters, we stop at the first close delimiter.
    if open == '{' {
        let mut escaped = false;
        let mut in_quote: Option<char> = None;
        let mut depth = 1usize;
        for ch in iter.by_ref() {
            if escaped {
                body.push(ch);
                escaped = false;
                continue;
            }
            if ch == '\\' {
                body.push(ch);
                escaped = true;
                continue;
            }
            if let Some(q) = in_quote {
                if ch == q {
                    in_quote = None;
                }
                body.push(ch);
                continue;
            }
            if ch == '"' || ch == '\'' {
                in_quote = Some(ch);
                body.push(ch);
                continue;
            }
            if ch == '{' {
                depth += 1;
                body.push(ch);
                continue;
            }
            if ch == '}' {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
                body.push(ch);
                continue;
            }
            body.push(ch);
        }
    } else {
        let mut escaped = false;
        for ch in iter.by_ref() {
            if escaped {
                body.push(ch);
                escaped = false;
                continue;
            }
            if ch == '\\' {
                body.push(ch);
                escaped = true;
                continue;
            }
            if ch == close {
                break;
            }
            body.push(ch);
        }
    }
    let flags: String = iter.take_while(|c| c.is_ascii_alphabetic()).collect();
    Some((tag, body, flags))
}

fn parse_html_angle_sigil_text(text: &str) -> Option<String> {
    // HTML sigil syntax: `~<html> ... </html>`
    let open = "~<html>";
    let close = "</html>";
    if !text.starts_with(open) || !text.ends_with(close) {
        return None;
    }
    let body = &text[open.len()..text.len().saturating_sub(close.len())];
    Some(body.to_string())
}

fn is_probably_url(text: &str) -> bool {
    let text = text.trim();
    let Some((scheme, rest)) = text.split_once("://") else {
        return false;
    };
    if scheme.is_empty()
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
    {
        return false;
    }
    !rest.is_empty() && !rest.starts_with('/')
}

fn is_probably_date(text: &str) -> bool {
    let text = text.trim();
    let parts: Vec<&str> = text.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    parts[0].len() == 4
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}

fn is_probably_datetime(text: &str) -> bool {
    let text = text.trim();
    let Some((date, time)) = text.split_once('T') else {
        return false;
    };
    if !is_probably_date(date) {
        return false;
    }
    let Some(time) = time.strip_suffix('Z') else {
        return false;
    };
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() != 3 {
        return false;
    }
    parts[0].len() == 2
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}

fn path_span(path: &[PathSegment]) -> Span {
    match (path.first(), path.last()) {
        (Some(PathSegment::Field(first)), Some(PathSegment::Field(last))) => {
            merge_span(first.span.clone(), last.span.clone())
        }
        (Some(PathSegment::Field(first)), Some(PathSegment::Index(_, span))) => {
            merge_span(first.span.clone(), span.clone())
        }
        (Some(PathSegment::Field(first)), Some(PathSegment::All(span))) => {
            merge_span(first.span.clone(), span.clone())
        }
        (Some(PathSegment::Index(_, span)), _) => span.clone(),
        (Some(PathSegment::All(span)), _) => span.clone(),
        _ => Span {
            start: Position { line: 1, column: 1 },
            end: Position { line: 1, column: 1 },
        },
    }
}

fn is_adjacent(left: &Span, right: &Span) -> bool {
    left.end.line == right.start.line && left.end.column + 1 == right.start.column
}
