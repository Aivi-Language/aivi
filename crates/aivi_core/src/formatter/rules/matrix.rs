#[derive(Debug, Clone)]
struct MatrixSigil {
    tag: String,
    rows: Vec<Vec<String>>,
}

fn parse_matrix_sigil(text: &str) -> Option<MatrixSigil> {
    let mut iter = text.chars();
    if iter.next()? != '~' {
        return None;
    }
    let mut tag = String::new();
    for ch in iter.by_ref() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            tag.push(ch);
            continue;
        }
        if ch != '[' {
            return None;
        }
        break;
    }
    if tag != "mat" {
        return None;
    }

    let mut body = String::new();
    let mut escaped = false;
    for ch in iter.by_ref() {
        if escaped {
            body.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == ']' {
            break;
        }
        body.push(ch);
    }

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut token = String::new();
    let push_token = |row: &mut Vec<String>, token: &mut String| {
        if !token.is_empty() {
            row.push(token.clone());
            token.clear();
        }
    };
    let push_row = |rows: &mut Vec<Vec<String>>, row: &mut Vec<String>| {
        if !row.is_empty() {
            rows.push(row.clone());
            row.clear();
        }
    };

    for ch in body.chars() {
        match ch {
            '\r' => {}
            '\n' | ';' => {
                push_token(&mut row, &mut token);
                push_row(&mut rows, &mut row);
            }
            ',' | ' ' | '\t' => {
                push_token(&mut row, &mut token);
            }
            _ => token.push(ch),
        }
    }
    push_token(&mut row, &mut token);
    push_row(&mut rows, &mut row);

    if rows.is_empty() {
        return None;
    }
    Some(MatrixSigil { tag, rows })
}

fn format_matrix_rows(rows: &[Vec<String>]) -> Vec<String> {
    let mut max_cols = 0usize;
    for row in rows {
        max_cols = max_cols.max(row.len());
    }
    if max_cols == 0 {
        return Vec::new();
    }
    let mut widths = vec![0usize; max_cols];
    for row in rows {
        for (i, value) in row.iter().enumerate() {
            widths[i] = widths[i].max(value.chars().count());
        }
    }
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let mut line = String::new();
        for (col, width) in widths.iter().enumerate().take(max_cols) {
            let value = row.get(col).map(String::as_str).unwrap_or("");
            let value_len = value.chars().count();
            let pad = width.saturating_sub(value_len);
            if pad > 0 {
                line.push_str(&" ".repeat(pad));
            }
            line.push_str(value);
            if col + 1 < max_cols {
                line.push(' ');
            }
        }
        out.push(line);
    }
    out
}

fn advance_column(col: &mut usize, text: &str) {
    for ch in text.chars() {
        if ch == '\n' {
            *col = 0;
        } else {
            *col += 1;
        }
    }
}

fn format_tokens_with_matrix(
    tokens: &[&crate::cst::CstToken],
    top_delim: Option<char>,
    base_indent: &str,
) -> String {
    let strip_commas = top_delim != Some('(');
    // Find the index from which all remaining code tokens are commas.
    // This strips ALL trailing commas in one pass (not just the last one)
    // to ensure idempotency when consecutive commas appear.
    let trailing_commas_start: Option<usize> = if strip_commas {
        let mut first_trailing = None;
        for (i, t) in tokens.iter().enumerate().rev() {
            if t.kind == "comment" || t.text == "\n" || t.text == ";" {
                continue;
            }
            if t.text == "," {
                first_trailing = Some(i);
            } else {
                break;
            }
        }
        first_trailing
    } else {
        None
    };

    // Detect inline matrix sigil pattern: `~` `mat` `[` ... `;` ... `]`
    // Returns (start_of_~_index, end_of_]_index) if found.
    let mat_range = {
        let mut found = None;
        let mut i = 0;
        while i + 2 < tokens.len() {
            if tokens[i].text == "~"
                && tokens[i + 1].text == "mat"
                && tokens[i + 2].text == "["
            {
                // Find matching `]` at the same bracket depth.
                let mut depth = 1usize;
                let mut j = i + 3;
                let mut has_semi = false;
                while j < tokens.len() && depth > 0 {
                    if tokens[j].text == "[" {
                        depth += 1;
                    } else if tokens[j].text == "]" {
                        depth -= 1;
                    } else if tokens[j].text == ";" {
                        has_semi = true;
                    }
                    if depth > 0 {
                        j += 1;
                    }
                }
                if depth == 0 && has_semi {
                    found = Some((i, j));
                }
                break;
            }
            i += 1;
        }
        found
    };

    let mut out = String::new();
    let mut prevprev: Option<(&str, &str)> = None;
    let mut prev: Option<(&str, &str)> = None;
    let mut prev_token: Option<&crate::cst::CstToken> = None;
    let mut current_col = 0usize;
    // Track whether we're still in the leading-comma region (all commas before
    // the first non-comma code token).
    let mut in_leading_commas = strip_commas;

    let mut skip_until: Option<usize> = None;

    for (i, t) in tokens.iter().enumerate() {
        if let Some(skip) = skip_until {
            if i <= skip {
                continue;
            }
            skip_until = None;
        }
        // Skip all leading commas (not just the first) so formatting is
        // idempotent when the input has multiple consecutive commas.
        if in_leading_commas {
            if t.text == "," || t.text == ";" || t.kind == "comment" || t.text == "\n" {
                if t.text == "," {
                    continue;
                }
            } else {
                in_leading_commas = false;
            }
        }
        // Skip all trailing commas (not just the last one) for idempotency.
        if let Some(start) = trailing_commas_start {
            if i >= start && t.text == "," {
                continue;
            }
        }
        if t.kind == "comment" {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
                current_col += 1;
            }
            out.push_str(&t.text);
            advance_column(&mut current_col, &t.text);
            prevprev = prev;
            prev = Some((t.kind.as_str(), t.text.as_str()));
            continue;
        }

        // Handle inline matrix sigil from separate tokens.
        if let Some((mat_start, mat_end)) = mat_range {
            if i == mat_start {
                // Collect cell values between `[` and `]`.
                let content_start = mat_start + 3; // after `~`, `mat`, `[`
                let mut rows: Vec<Vec<String>> = Vec::new();
                let mut row: Vec<String> = Vec::new();
                let content_tokens = &tokens[content_start..mat_end];
                let mut ci = 0;
                while ci < content_tokens.len() {
                    let ct = &content_tokens[ci];
                    if ct.text == ";" {
                        if !row.is_empty() {
                            rows.push(row);
                            row = Vec::new();
                        }
                    } else if ct.text == "," {
                        // cell separator, skip
                    } else if ct.kind == "comment" {
                        // skip comments
                    } else if ct.text == "-" && ci + 1 < content_tokens.len() && content_tokens[ci + 1].kind == "number" {
                        // Merge unary minus with following number
                        row.push(format!("-{}", content_tokens[ci + 1].text));
                        ci += 1; // skip the number token
                    } else {
                        row.push(ct.text.clone());
                    }
                    ci += 1;
                }
                if !row.is_empty() {
                    rows.push(row);
                }

                let formatted_rows = format_matrix_rows(&rows);
                if !formatted_rows.is_empty() {
                    // Add space before `~` if needed.
                    let curr = (t.kind.as_str(), t.text.as_str());
                    let adjacent_in_input = prev_token.is_some_and(|p| {
                        p.span.start.line == t.span.start.line
                            && p.span.end.column + 1 == t.span.start.column
                    });
                    if wants_space_between(prevprev, prev, curr, adjacent_in_input)
                        && !out.is_empty()
                    {
                        out.push(' ');
                        current_col += 1;
                    }

                    let prefix = "~mat[";
                    let row_start_col = current_col + prefix.len();
                    out.push_str(prefix);
                    advance_column(&mut current_col, prefix);
                    out.push_str(&formatted_rows[0]);
                    advance_column(&mut current_col, &formatted_rows[0]);
                    if formatted_rows.len() == 1 {
                        out.push(']');
                        current_col += 1;
                    } else {
                        for frow in formatted_rows.iter().skip(1) {
                            out.push('\n');
                            advance_column(&mut current_col, "\n");
                            out.push_str(base_indent);
                            advance_column(&mut current_col, base_indent);
                            let pad = " ".repeat(row_start_col);
                            out.push_str(&pad);
                            advance_column(&mut current_col, &pad);
                            out.push_str(frow);
                            advance_column(&mut current_col, frow);
                        }
                        out.push(']');
                        current_col += 1;
                    }
                    skip_until = Some(mat_end);
                    prev_token = Some(tokens[mat_end]);
                    prevprev = prev;
                    prev = Some(("symbol", "]"));
                    continue;
                }
            }
        }

        // Skip stray `;` tokens (they're not part of AIVI syntax outside matrix literals).
        // Do NOT emit a space here: `wants_space_between` for the next real token
        // handles operator separation correctly. Because the skipped `;` leaves the two
        // neighbours non-adjacent (`adjacent_in_input = false`), the `is_op` / word-kind
        // checks in `wants_space_between` add a space wherever one is genuinely needed
        // (e.g. `<;-` → `< -`). Adding a space unconditionally caused non-idempotency:
        // `unknown;unknown` → `unknown  unknown` on pass 1 but `unknownunknown` on pass 2.
        if t.text == ";" {
            continue;
        }

        let curr = (t.kind.as_str(), t.text.as_str());
        let adjacent_in_input = prev_token.is_some_and(|p| {
            p.span.start.line == t.span.start.line
                && p.span.end.column + 1 == t.span.start.column
        });
        if wants_space_between(prevprev, prev, curr, adjacent_in_input) && !out.is_empty() {
            out.push(' ');
            current_col += 1;
        }

        if t.kind == "sigil" {
            if let Some(markup_lines) = format_markup_sigil(&t.text) {
                if !markup_lines.is_empty() {
                    let row_start_col = current_col;
                    out.push_str(&markup_lines[0]);
                    advance_column(&mut current_col, &markup_lines[0]);
                    for line in markup_lines.iter().skip(1) {
                        out.push('\n');
                        advance_column(&mut current_col, "\n");
                        out.push_str(base_indent);
                        advance_column(&mut current_col, base_indent);
                        let pad = " ".repeat(row_start_col);
                        out.push_str(&pad);
                        advance_column(&mut current_col, &pad);
                        out.push_str(line);
                        advance_column(&mut current_col, line);
                    }
                    prev_token = Some(t);
                    prevprev = prev;
                    prev = Some(curr);
                    continue;
                }
            }
            if let Some(matrix) = parse_matrix_sigil(&t.text) {
                let rows = format_matrix_rows(&matrix.rows);
                if !rows.is_empty() {
                    let prefix = format!("~{}[", matrix.tag);
                    let row_start_col = current_col + prefix.chars().count();
                    out.push_str(&prefix);
                    advance_column(&mut current_col, &prefix);
                    out.push_str(&rows[0]);
                    advance_column(&mut current_col, &rows[0]);
                    if rows.len() == 1 {
                        out.push(']');
                        current_col += 1;
                    } else {
                        for row in rows.iter().skip(1) {
                            out.push('\n');
                            advance_column(&mut current_col, "\n");
                            out.push_str(base_indent);
                            advance_column(&mut current_col, base_indent);
                            let pad = " ".repeat(row_start_col);
                            out.push_str(&pad);
                            advance_column(&mut current_col, &pad);
                            out.push_str(row);
                            advance_column(&mut current_col, row);
                        }
                        out.push(']');
                        current_col += 1;
                    }
                    prev_token = Some(t);
                    prevprev = prev;
                    prev = Some(curr);
                    continue;
                }
            }
        }

        out.push_str(curr.1);
        advance_column(&mut current_col, curr.1);
        prev_token = Some(t);
        prevprev = prev;
        prev = Some(curr);
    }
    out
}
