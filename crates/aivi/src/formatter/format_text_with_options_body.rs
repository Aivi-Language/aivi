{
    let indent_size = options.indent_size.clamp(1, 16);
    let max_blank_lines = options.max_blank_lines.min(10);
    let (tokens, _) = lex(content);

    let mut raw_lines: Vec<&str> = content.split('\n').collect();
    let mut tokens_by_line: Vec<Vec<&crate::cst::CstToken>> = vec![Vec::new(); raw_lines.len()];
    for token in &tokens {
        if token.kind == "whitespace" {
            continue;
        }
        if token.text == ";" {
            continue;
        }
        let line = token.span.start.line;
        if line == 0 {
            continue;
        }
        if let Some(bucket) = tokens_by_line.get_mut(line - 1) {
            bucket.push(token);
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ContextKind {
        Effect,
        Generate,
        Resource,
        MapSigil,
        SetSigil,
        Other,
    }

    #[derive(Debug, Clone)]
    struct OpenFrame {
        sym: char,
        kind: ContextKind,
    }

    #[derive(Debug, Clone)]
    struct LineState<'a> {
        tokens: Vec<&'a crate::cst::CstToken>,
        indent: String,
        indent_len: usize,
        top_delim: Option<char>,
        top_context: Option<ContextKind>,
        effect_align_lhs: Option<usize>,
        arm_align_pat: Option<usize>,
        map_align_key: Option<usize>,
        degraded: bool,
    }

    fn is_open_sym(text: &str) -> Option<char> {
        match text {
            "{" => Some('{'),
            "(" => Some('('),
            "[" => Some('['),
            _ => None,
        }
    }

    fn is_close_sym(text: &str) -> Option<char> {
        match text {
            "}" => Some('}'),
            ")" => Some(')'),
            "]" => Some(']'),
            _ => None,
        }
    }

    fn matches_pair(open: char, close: char) -> bool {
        matches!((open, close), ('{', '}') | ('(', ')') | ('[', ']'))
    }

    fn is_word_kind(kind: &str) -> bool {
        matches!(kind, "ident" | "number" | "string" | "sigil")
    }

    fn is_keyword(text: &str) -> bool {
        syntax::KEYWORDS_ALL.contains(&text)
    }

    fn first_code_index(tokens: &[&crate::cst::CstToken]) -> Option<usize> {
        tokens
            .iter()
            .position(|t| t.kind != "comment" && t.text != "\n")
    }

    fn last_code_token_is(tokens: &[&crate::cst::CstToken], expected: &[&str]) -> bool {
        let Some(last) = tokens
            .iter()
            .rev()
            .find(|t| t.kind != "comment" && t.text != "\n")
        else {
            return false;
        };
        expected.iter().any(|e| *e == last.text.as_str())
    }

    fn find_top_level_token(
        tokens: &[&crate::cst::CstToken],
        needle: &str,
        start: usize,
    ) -> Option<usize> {
        let mut depth = 0isize;
        for (i, t) in tokens.iter().enumerate().skip(start) {
            let text = t.text.as_str();
            if t.kind == "string" || t.kind == "comment" {
                continue;
            }
            if let Some(open) = is_open_sym(text) {
                let _ = open;
                depth += 1;
                continue;
            }
            if let Some(close) = is_close_sym(text) {
                let _ = close;
                depth -= 1;
                continue;
            }
            if depth == 0 && text == needle {
                return Some(i);
            }
        }
        None
    }

    fn wants_space_between(
        prevprev: Option<(&str, &str)>,
        prev: Option<(&str, &str)>,
        curr: (&str, &str),
        adjacent_in_input: bool,
    ) -> bool {
        let Some((prev_kind, prev_text)) = prev else {
            return false;
        };
        let (curr_kind, curr_text) = curr;

        if adjacent_in_input && (curr_text == "(" || curr_text == "[") {
            return false;
        }

        // Indexing/call brackets: never insert a space before `[` after a closed group.
        if curr_text == "[" && matches!(prev_text, ")" | "]" | "}") {
            return false;
        }

        if prev_text == "~" || prev_text == "@" || prev_text == "." || prev_text == "..." {
            return false;
        }
        if curr_text == "," || curr_text == ")" || curr_text == "]" {
            return false;
        }
        if prev_text == "," {
            return true;
        }

        if prev_text == "(" || prev_text == "[" {
            return false;
        }
        if prev_text == "{" {
            return curr_text != "}";
        }
        if curr_text == "}" {
            return prev_text != "{";
        }

        // Date/Time fragments: no space around '-' or ':' if surrounded by numbers.
        if prev_kind == "number" && curr_text == "-" {
            return false;
        }
        if prev_text == "-" && curr_kind == "number" {
            // Date fragments: `YYYY-MM` (no spaces) but keep binary minus spacing (`x - 1`).
            if prevprev.is_some_and(|(k, _)| k == "number") {
                return false;
            }
        }
        if prev_kind == "number" && curr_text == ":" {
            return false;
        }
        if prev_text == ":" && curr_kind == "number" {
            if let Some((pp_kind, pp_text)) = prevprev {
                let is_time_prefix = pp_text.starts_with('T')
                    && pp_text.len() > 1
                    && pp_text[1..].chars().all(|ch| ch.is_ascii_digit());
                if pp_kind == "number" || is_time_prefix {
                    return false;
                }
            }
        }

        // Ranges: no spaces around `..` when between numbers.
        if prev_kind == "number" && curr_text == ".." {
            return false;
        }
        if prev_text == ".." && curr_kind == "number" {
            return false;
        }

        if curr_text == ":" {
            return false;
        }
        if prev_text == ":" {
            return true;
        }
        if curr_text == "{" {
            if prev_text == "map" && prevprev.map(|(_, t)| t) == Some("~") {
                return false;
            }
            return prev_text != "@" && prev_text != ".";
        }
        if curr_text == "[" {
            if prev_text == "set" && prevprev.map(|(_, t)| t) == Some("~") {
                return false;
            }
            // Indexing is only when the bracket is adjacent: `arr[i]` / `(f x)[i]`.
            if adjacent_in_input && (is_word_kind(prev_kind) || matches!(prev_text, ")" | "]" | "}"))
            {
                return false;
            }
            return prev_text != "." && prev_text != "@";
        }

        // Dot access: no spaces around dot in `a.b`, but allow space before dot when starting `.name`.
        if prev_text == "." {
            return false;
        }
        if curr_text == "." {
            if is_word_kind(prev_kind) || matches!(prev_text, ")" | "]" | "}") {
                return false;
            }
            return true;
        }

        // Unit suffixes: no space between number and ident/percent (except if ident is keyword)
        if prev_kind == "number"
            && adjacent_in_input
            && (curr_text == "%" || (curr_kind == "ident" && !is_keyword(curr_text)))
        {
            return false;
        }

        // Postfix domain-literal application: no space between `)` and adjacent suffix.
        // This preserves forms like `(x)px` and `(n)%`.
        if prev_text == ")"
            && adjacent_in_input
            && (curr_text == "%" || (curr_kind == "ident" && !is_keyword(curr_text)))
        {
            return false;
        }

        // Unary +/-: no space between sign and number if it doesn't follow a binary precursor.
        if (prev_text == "-" || prev_text == "+") && curr_kind == "number" {
            let precursor = prevprev.map(|(_, t)| t).unwrap_or("");
            if precursor.is_empty()
                || matches!(
                    precursor,
                    "(" | "["
                        | "{"
                        | ","
                        | ":"
                        | "="
                        | "->"
                        | "=>"
                        | "<-"
                        | "|>"
                        | "<|"
                        | "?"
                        | "|"
                )
                || is_op(precursor)
            {
                return false;
            }
        }

        // Always space after keywords before words/symbol groups like `effect {`.
        if is_keyword(prev_text) {
            return true;
        }

        if prev_text == "="
            || prev_text == "=>"
            || prev_text == "<-"
            || prev_text == "->"
            || prev_text == "|>"
            || prev_text == "<|"
        {
            return true;
        }
        if curr_text == "="
            || curr_text == "=>"
            || curr_text == "<-"
            || curr_text == "->"
            || curr_text == "|>"
            || curr_text == "<|"
        {
            return true;
        }
        if is_op(prev_text) || is_op(curr_text) {
            return true;
        }

        if is_word_kind(prev_kind) && is_word_kind(curr_kind) {
            return true;
        }
        if is_word_kind(prev_kind) && curr_text == "(" {
            return true;
        }
        if prev_text == ")" && (is_word_kind(curr_kind) || curr_text == "(") {
            return true;
        }
        if prev_text == "}"
            && (is_word_kind(curr_kind) || is_keyword(curr_text) || curr_text == "(")
        {
            return true;
        }
        if prev_text == "]"
            && (is_word_kind(curr_kind) || is_keyword(curr_text) || curr_text == "(")
        {
            return true;
        }

        false
    }

    fn format_tokens_simple(tokens: &[&crate::cst::CstToken], top_delim: Option<char>) -> String {
        // Prefer newline-based separators for multiline forms by stripping trailing commas.
        // This is safe for record/list/map/set forms where `,` is an alternative `FieldSep`,
        // but *not* for multiline tuples, where commas are required separators.
        let strip_commas = top_delim != Some('(');
        let trailing_comma_idx = {
            let mut idx = None;
            for (i, t) in tokens.iter().enumerate() {
                if t.kind != "comment" && t.text != "\n" {
                    idx = Some(i);
                }
            }
            idx.filter(|&i| strip_commas && tokens[i].text == ",")
        };

        let mut out = String::new();
        let mut prevprev: Option<(&str, &str)> = None;
        let mut prev: Option<(&str, &str)> = None;
        let mut prev_token: Option<&crate::cst::CstToken> = None;
        let leading_comma_idx = first_code_index(tokens)
            .filter(|&i| strip_commas && tokens[i].text == ",");
        for (i, t) in tokens.iter().enumerate() {
            if leading_comma_idx == Some(i) {
                continue;
            }
            if trailing_comma_idx == Some(i) {
                continue;
            }
            if t.kind == "comment" {
                if !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str(&t.text);
                prevprev = prev;
                prev = Some((t.kind.as_str(), t.text.as_str()));
                continue;
            }

            let curr = (t.kind.as_str(), t.text.as_str());
            let adjacent_in_input = prev_token.is_some_and(|p| {
                p.span.start.line == t.span.start.line
                    && p.span.end.column + 1 == t.span.start.column
            });
            if wants_space_between(prevprev, prev, curr, adjacent_in_input) && !out.is_empty() {
                out.push(' ');
            }
            out.push_str(curr.1);
            prev_token = Some(t);
            prevprev = prev;
            prev = Some(curr);
        }
        out
    }

    fn leading_indent(line: &str) -> (String, usize) {
        let mut bytes = 0usize;
        for (i, ch) in line.char_indices() {
            if ch == ' ' || ch == '\t' {
                bytes = i + ch.len_utf8();
                continue;
            }
            break;
        }
        let indent = line[..bytes].to_string();
        let len = indent.chars().count();
        (indent, len)
    }

    // Pre-pass: merge "hanging" openers (`{`/`[`) that appear alone on the next line after
    // `=` / `=>` / `<-` / `->` back onto the previous line, then drop the opener-only line.
    //
    // This is intentionally conservative (no comments on the opener line) to avoid surprising
    // rewrites while still fixing the common formatter artifact in `integration-tests/complex`.
    {
        // Allman brace style: split trailing `{` onto its own line (best-effort).
        if matches!(options.brace_style, BraceStyle::Allman) {
            let mut split_raw_lines: Vec<&str> = Vec::with_capacity(raw_lines.len() + 16);
            let mut split_tokens_by_line: Vec<Vec<&crate::cst::CstToken>> =
                Vec::with_capacity(tokens_by_line.len() + 16);

            for (line_index, raw) in raw_lines.iter().enumerate() {
                let mut line_tokens = tokens_by_line[line_index].clone();
                let has_comment = line_tokens.iter().any(|t| t.kind == "comment");
                let last_is_open = last_code_token_is(&line_tokens, &["{"]);
                if !has_comment && last_is_open && line_tokens.len() >= 2 {
                    // Move the last `{` token to a new line.
                    let brace = line_tokens.pop().expect("brace token");
                    split_raw_lines.push(*raw);
                    split_tokens_by_line.push(line_tokens);
                    split_raw_lines.push("");
                    split_tokens_by_line.push(vec![brace]);
                    continue;
                }
                split_raw_lines.push(*raw);
                split_tokens_by_line.push(line_tokens);
            }

            raw_lines = split_raw_lines;
            tokens_by_line = split_tokens_by_line;
        }

        let mut merged_raw_lines: Vec<&str> = Vec::with_capacity(raw_lines.len());
        let mut merged_tokens_by_line: Vec<Vec<&crate::cst::CstToken>> =
            Vec::with_capacity(tokens_by_line.len());

        for (line_index, raw) in raw_lines.iter().enumerate() {
            let line_tokens = tokens_by_line[line_index].clone();
            let opener_tok = if line_tokens.iter().any(|t| t.kind == "comment") {
                None
            } else {
                if let Some(first_idx) = first_code_index(&line_tokens) {
                    if line_tokens.len() == 1
                        && matches!(line_tokens[first_idx].text.as_str(), "{" | "[")
                    {
                        Some(line_tokens[first_idx])
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if matches!(options.brace_style, BraceStyle::Kr) {
                if let Some(opener_tok) = opener_tok {
                    if let Some(prev_tokens) = merged_tokens_by_line.last_mut() {
                        if last_code_token_is(
                            prev_tokens,
                            &["=", "=>", "<-", "->", "then", "else", "?"],
                        ) {
                            prev_tokens.push(opener_tok);
                            continue;
                        }
                    }
                }
            }

            merged_raw_lines.push(*raw);
            merged_tokens_by_line.push(line_tokens);
        }

        raw_lines = merged_raw_lines;
        tokens_by_line = merged_tokens_by_line;
    }

    // Pre-pass: merge "hanging" match subjects onto the `=>` line:
    //
    //   name = args =>
    //     subject ?
    //       | ...
    //
    // becomes
    //
    //   name = args => subject ?
    //     | ...
    //
    // This is intentionally conservative (no comments on either merged line).
    {
        fn starts_with(tokens: &[&crate::cst::CstToken], text: &str) -> bool {
            first_code_index(tokens).is_some_and(|i| tokens[i].text == text)
        }

        let mut merged_raw_lines: Vec<&str> = Vec::with_capacity(raw_lines.len());
        let mut merged_tokens_by_line: Vec<Vec<&crate::cst::CstToken>> =
            Vec::with_capacity(tokens_by_line.len());

        let mut i = 0usize;
        while i < raw_lines.len() {
            let tokens = tokens_by_line[i].clone();
            if tokens.is_empty() {
                merged_raw_lines.push(raw_lines[i]);
                merged_tokens_by_line.push(tokens);
                i += 1;
                continue;
            }

            let can_merge = !tokens.iter().any(|t| t.kind == "comment")
                && last_code_token_is(&tokens, &["=>"]);
            if can_merge && i + 2 < raw_lines.len() {
                let next_tokens = tokens_by_line[i + 1].clone();
                let after_tokens = tokens_by_line[i + 2].clone();
                if !next_tokens.is_empty()
                    && !after_tokens.is_empty()
                    && !next_tokens.iter().any(|t| t.kind == "comment")
                    && last_code_token_is(&next_tokens, &["?"])
                    && !starts_with(&next_tokens, "|")
                    && starts_with(&after_tokens, "|")
                {
                    let mut combined = tokens.clone();
                    combined.extend(next_tokens);
                    merged_raw_lines.push(raw_lines[i]);
                    merged_tokens_by_line.push(combined);
                    i += 2;
                    continue;
                }
            }

            merged_raw_lines.push(raw_lines[i]);
            merged_tokens_by_line.push(tokens);
            i += 1;
        }

        raw_lines = merged_raw_lines;
        tokens_by_line = merged_tokens_by_line;
    }

    // First pass: compute context per line and indentation level.
    let mut stack: Vec<OpenFrame> = Vec::new();
    let mut degraded = false;
    let mut prev_non_comment_text: Option<String> = None;
    let mut prevprev_non_comment_text: Option<String> = None;

    let mut lines: Vec<LineState<'_>> = Vec::with_capacity(raw_lines.len());

    for line_index in 0..raw_lines.len() {
        let mut line_tokens = tokens_by_line[line_index].clone();
        // Sort by original (line, column) to stay correct even after we merge tokens across lines.
        line_tokens.sort_by_key(|t| (t.span.start.line, t.span.start.column, t.span.end.column));

        let (input_indent, _) = leading_indent(raw_lines[line_index]);

        let mut indent_level = stack
            .iter()
            .filter(|f| matches!(f.sym, '{' | '[' | '('))
            .count();
        if !degraded {
            if let Some(first_idx) = first_code_index(&line_tokens) {
                if is_close_sym(line_tokens[first_idx].text.as_str()).is_some() {
                    indent_level = indent_level.saturating_sub(1);
                }
            }
        }

        let indent = if degraded {
            input_indent
        } else {
            " ".repeat(indent_level * indent_size)
        };
        let indent_len = indent.chars().count();
        let top_context = stack.last().map(|f| f.kind);

        lines.push(LineState {
            tokens: line_tokens,
            indent,
            indent_len,
            top_delim: stack.last().map(|f| f.sym),
            top_context,
            effect_align_lhs: None,
            arm_align_pat: None,
            map_align_key: None,
            degraded,
        });

        if degraded {
            continue;
        }

        // Use the sorted line tokens so delimiter tracking stays stable even after we merge tokens
        // across lines in pre-passes.
        for t in lines
            .last()
            .expect("just pushed current line")
            .tokens
            .iter()
        {
            if t.kind == "comment" {
                continue;
            }
            let text = t.text.as_str();
            if let Some(open) = is_open_sym(text) {
                let kind = match (
                    open,
                    prev_non_comment_text.as_deref(),
                    prevprev_non_comment_text.as_deref(),
                ) {
                    ('{', Some("effect"), _) => ContextKind::Effect,
                    ('{', Some("generate"), _) => ContextKind::Generate,
                    ('{', Some("resource"), _) => ContextKind::Resource,
                    ('{', Some("map"), Some("~")) => ContextKind::MapSigil,
                    ('[', Some("set"), Some("~")) => ContextKind::SetSigil,
                    _ => ContextKind::Other,
                };
                stack.push(OpenFrame { sym: open, kind });
            } else if let Some(close) = is_close_sym(text) {
                let Some(frame) = stack.pop() else {
                    degraded = true;
                    break;
                };
                if !matches_pair(frame.sym, close) {
                    degraded = true;
                    break;
                }
            }

            prevprev_non_comment_text = prev_non_comment_text;
            prev_non_comment_text = Some(text.to_string());
        }
    }

    // Second pass: mark alignment groups.
    let mut i = 0usize;
    while i < lines.len() {
        if lines[i].tokens.is_empty() || lines[i].degraded {
            i += 1;
            continue;
        }

        let first = first_code_index(&lines[i].tokens);
        if let Some(first_idx) = first {
            if lines[i].top_context == Some(ContextKind::Effect) {
                // Effect bind alignment groups: consecutive `<-` lines, unbroken.
                if find_top_level_token(&lines[i].tokens, "<-", first_idx).is_some() {
                    let mut j = i;
                    let mut max_lhs = 0usize;
                    while j < lines.len() {
                        if lines[j].tokens.is_empty() || lines[j].degraded {
                            break;
                        }
                        if lines[j].top_context != Some(ContextKind::Effect) {
                            break;
                        }
                        let first_idx_j = match first_code_index(&lines[j].tokens) {
                            Some(v) => v,
                            None => break,
                        };
                        let Some(arrow_idx) =
                            find_top_level_token(&lines[j].tokens, "<-", first_idx_j)
                        else {
                            break;
                        };
                        let lhs_tokens = &lines[j].tokens[first_idx_j..arrow_idx];
                        let lhs_str =
                            format_tokens_simple(lhs_tokens, lines[j].top_delim).trim().to_string();
                        max_lhs = max_lhs.max(lhs_str.len());
                        j += 1;
                    }
                    if j - i >= 2 {
                        for line in lines.iter_mut().take(j).skip(i) {
                            line.effect_align_lhs = Some(max_lhs);
                        }
                    }
                    i = j;
                    continue;
                }
            }

            // Pattern match arm alignment groups.
            let is_arm = lines[i].tokens[first_idx].text == "|"
                && find_top_level_token(&lines[i].tokens, "=>", first_idx + 1).is_some();
            if is_arm {
                let this_indent = lines[i].indent_len;
                let mut j = i;
                let mut max_pat = 0usize;
                while j < lines.len() {
                    if lines[j].tokens.is_empty()
                        || lines[j].degraded
                        || lines[j].indent_len != this_indent
                    {
                        break;
                    }
                    let Some(first_idx_j) = first_code_index(&lines[j].tokens) else {
                        break;
                    };
                    if lines[j].tokens[first_idx_j].text != "|" {
                        break;
                    }
                    let Some(arrow_idx) =
                        find_top_level_token(&lines[j].tokens, "=>", first_idx_j + 1)
                    else {
                        break;
                    };
                    let pat_tokens = &lines[j].tokens[first_idx_j + 1..arrow_idx];
                    let pat_str =
                        format_tokens_simple(pat_tokens, lines[j].top_delim).trim().to_string();
                    max_pat = max_pat.max(pat_str.len());
                    j += 1;
                }
                if j - i >= 2 {
                    for line in lines.iter_mut().take(j).skip(i) {
                        line.arm_align_pat = Some(max_pat);
                    }
                }
                i = if j == i { i + 1 } else { j };
                continue;
            }

            // Structured map literal entry alignment groups (inside `~map{ ... }`).
            if lines[i].top_context == Some(ContextKind::MapSigil) {
                let Some(_) = find_top_level_token(&lines[i].tokens, "=>", first_idx) else {
                    i += 1;
                    continue;
                };
                let this_indent = lines[i].indent_len;
                let mut j = i;
                let mut max_key = 0usize;
                while j < lines.len() {
                    if lines[j].tokens.is_empty()
                        || lines[j].degraded
                        || lines[j].indent_len != this_indent
                        || lines[j].top_context != Some(ContextKind::MapSigil)
                    {
                        break;
                    }
                    let Some(first_idx_j) = first_code_index(&lines[j].tokens) else {
                        break;
                    };
                    let Some(arrow_idx_j) =
                        find_top_level_token(&lines[j].tokens, "=>", first_idx_j)
                    else {
                        break;
                    };
                    let key_tokens = &lines[j].tokens[first_idx_j..arrow_idx_j];
                    let key_str =
                        format_tokens_simple(key_tokens, lines[j].top_delim).trim().to_string();
                    max_key = max_key.max(key_str.len());
                    j += 1;
                }
                if j - i >= 2 {
                    for line in lines.iter_mut().take(j).skip(i) {
                        line.map_align_key = Some(max_key);
                    }
                }
                i = j;
                continue;
            }
        }

        i += 1;
    }

    // Third pass: render.
    //
    // NOTE: The lexer/parser is not indentation-sensitive per spec, but the current compiler
    // implementation uses newlines + indentation to disambiguate some constructs. To keep the
    // formatter deterministic and robust even when the input indentation is inconsistent, we
    // compute indentation from delimiter nesting (`{[(` / `}])`) plus a small set of newline
    // continuations (`|` arms, `then`/`else`, trailing `=`/`=>`).
    let mut rendered_lines: Vec<String> = Vec::new();
    let mut blank_run = 0usize;
    let mut pipe_block_stack: Vec<(usize, isize)> = Vec::new();
    let mut pipeop_block_base_indent: Option<usize> = None;
    let mut pipeop_block_base_depth: Option<isize> = None;
    let mut rhs_next_line_indent: Option<usize> = None;
    let mut rhs_next_line_depth: Option<isize> = None;
    let mut rhs_block_base_indent: Option<usize> = None;
    let mut rhs_block_base_depth: Option<isize> = None;
    let mut rhs_decorator_pending: bool = false;
    let mut arm_rhs_active = false;
    let mut pipeop_seed_indent: Option<usize> = None;
    let mut prev_non_blank_last_token: Option<String> = None;
    // Delimiter groups opened at end-of-line (`{`/`(`/`[`) that should cause a hanging indent
    // until the matching close delimiter starts a line. We also keep the opener line's effective
    // indentation to align the corresponding closer and contents.
    let mut hang_delim_stack: Vec<(char, usize)> = Vec::new();
    let mut open_depth: isize = 0;
    let mut prev_effective_indent_len: usize = 0;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum IfPhase {
        Then,
        Else,
    }

    #[derive(Debug, Clone, Copy)]
    struct IfFrame {
        if_indent: usize,
        phase: IfPhase,
        active_indent: bool,
    }

    // Tracks multiline `if ... then ... else ...` indentation so nested `if`s format correctly.
    let mut if_stack: Vec<IfFrame> = Vec::new();

    fn seeds_rhs_continuation(last: Option<&str>) -> bool {
        matches!(last, Some("=" | "=>" | "<-" | "->"))
    }

    fn last_continuation_token(tokens: &[&crate::cst::CstToken]) -> Option<String> {
        tokens
            .iter()
            .rev()
            .find(|t| {
                if t.kind == "comment" {
                    return false;
                }
                !matches!(t.text.as_str(), "{" | "[" | "(")
            })
            .map(|t| t.text.clone())
    }

    fn last_code_token(tokens: &[&crate::cst::CstToken]) -> Option<String> {
        tokens
            .iter()
            .rev()
            .find(|t| t.kind != "comment")
            .map(|t| t.text.clone())
    }

    fn matches_hang_close(opener: char, first_token_text: &str) -> bool {
        match (opener, first_token_text) {
            ('{', "}") => true,
            ('[', "]") => true,
            ('(', ")") => true,
            _ => false,
        }
    }

    fn net_open_depth(tokens: &[&crate::cst::CstToken]) -> isize {
        let mut depth = 0isize;
        for t in tokens {
            if matches!(t.kind.as_str(), "comment" | "string") {
                continue;
            }
            let text = t.text.as_str();
            if is_open_sym(text).is_some() {
                depth += 1;
            } else if is_close_sym(text).is_some() {
                depth -= 1;
            }
        }
        depth.max(0)
    }

    fn update_open_depth(open_depth: &mut isize, tokens: &[&crate::cst::CstToken]) {
        for t in tokens {
            if matches!(t.kind.as_str(), "comment" | "string") {
                continue;
            }
            let text = t.text.as_str();
            if is_open_sym(text).is_some() {
                *open_depth += 1;
            } else if is_close_sym(text).is_some() {
                *open_depth -= 1;
            }
        }
        if *open_depth < 0 {
            *open_depth = 0;
        }
    }

    fn looks_like_new_stmt(tokens: &[&crate::cst::CstToken], first_idx: usize) -> bool {
        let first = tokens[first_idx].text.as_str();
        if matches!(
            first,
            "module" | "use" | "export" | "type" | "class" | "instance" | "domain"
        ) {
            return true;
        }
        if tokens[first_idx].kind == "ident" {
            // A definition or type signature at the same indentation likely terminates a `|` block.
            if find_top_level_token(tokens, "=", first_idx + 1).is_some()
                || find_top_level_token(tokens, ":", first_idx + 1).is_some()
            {
                return true;
            }
        }
        false
    }

    fn find_top_level_token_clamped(
        tokens: &[&crate::cst::CstToken],
        needle: &str,
        start: usize,
    ) -> Option<usize> {
        let mut depth = 0isize;
        for (i, t) in tokens.iter().enumerate().skip(start) {
            let text = t.text.as_str();
            if t.kind == "string" || t.kind == "comment" {
                continue;
            }
            if is_open_sym(text).is_some() {
                depth += 1;
                continue;
            }
            if is_close_sym(text).is_some() {
                depth = (depth - 1).max(0);
                continue;
            }
            if depth == 0 && text == needle {
                return Some(i);
            }
        }
        None
    }

    for (line_index, state) in lines.iter().enumerate() {
        if state.tokens.is_empty() {
            // Keep `use` declarations grouped by removing blank lines between consecutive `use`s.
            let between_uses = {
                fn is_use_line(line: &LineState<'_>) -> bool {
                    first_code_index(&line.tokens).is_some_and(|i| line.tokens[i].text == "use")
                }

                let mut prev_use = None;
                for j in (0..line_index).rev() {
                    if lines[j].tokens.is_empty() {
                        continue;
                    }
                    prev_use = Some(is_use_line(&lines[j]));
                    break;
                }
                let mut next_use = None;
                for j in (line_index + 1)..lines.len() {
                    if lines[j].tokens.is_empty() {
                        continue;
                    }
                    next_use = Some(is_use_line(&lines[j]));
                    break;
                }
                prev_use == Some(true) && next_use == Some(true)
            };
            if between_uses {
                continue;
            }

            blank_run += 1;
            if blank_run > max_blank_lines {
                continue;
            }
            rendered_lines.push(String::new());
            // Keep continuation state across blank lines so indentation inside continuation blocks
            // and delimiter groups stays stable when the author uses spacing for readability.
            rhs_next_line_indent = None;
            rhs_next_line_depth = None;
            pipeop_seed_indent = None;
            continue;
        }

        blank_run = 0;

        let mut out = String::new();

        // One-shot seeds: only apply to the next non-blank line.
        let rhs_seed_indent = rhs_next_line_indent.take();
        let rhs_seed_depth = rhs_next_line_depth.take().unwrap_or(0);
        let pipeop_seed = pipeop_seed_indent.take();

        if state.degraded {
            out.push_str(state.indent.as_str());
            out.push_str(&format_tokens_simple(&state.tokens, state.top_delim));
            rendered_lines.push(out);
            pipe_block_stack.clear();
            pipeop_block_base_indent = None;
            pipeop_block_base_depth = None;
            rhs_next_line_indent = None;
            rhs_next_line_depth = None;
            rhs_block_base_indent = None;
            rhs_block_base_depth = None;
            pipeop_seed_indent = None;
            if_stack.clear();
            rhs_decorator_pending = false;
            prev_non_blank_last_token = last_continuation_token(&state.tokens);
            update_open_depth(&mut open_depth, &state.tokens);
            continue;
        }

        let Some(first_idx) = first_code_index(&state.tokens) else {
            out.push_str(state.indent.as_str());
            out.push_str(&format_tokens_simple(&state.tokens, state.top_delim));
            rendered_lines.push(out);
            pipe_block_stack.clear();
            pipeop_block_base_indent = None;
            pipeop_block_base_depth = None;
            rhs_next_line_indent = None;
            rhs_next_line_depth = None;
            rhs_block_base_indent = None;
            rhs_block_base_depth = None;
            pipeop_seed_indent = None;
            if_stack.clear();
            rhs_decorator_pending = false;
            prev_non_blank_last_token = last_continuation_token(&state.tokens);
            update_open_depth(&mut open_depth, &state.tokens);
            continue;
        };

        let rhs_decorator_pending_for_this_line = rhs_decorator_pending;
        rhs_decorator_pending = false;

        // Canonical base indentation from delimiter nesting (computed in pass 1).
        // This matches `stack`-based delimiter nesting, and avoids drift when heuristics for
        // continuation blocks are active.
        let line_indent_len = state.indent_len;
        let line_depth = (line_indent_len / indent_size) as isize;
        let pipeop_seed_match = pipeop_seed == Some(line_indent_len);
        let should_pop_hang = hang_delim_stack.last().is_some_and(|&(opener, _)| {
            matches_hang_close(opener, state.tokens[first_idx].text.as_str())
        });

        let line_has_top_level_eq =
            find_top_level_token(&state.tokens, "=", first_idx).is_some();

        if let (Some(base_indent), Some(base_depth)) = (rhs_block_base_indent, rhs_block_base_depth)
        {
            if line_depth <= base_depth
                && line_indent_len <= base_indent
                && looks_like_new_stmt(&state.tokens, first_idx)
                && !rhs_decorator_pending_for_this_line
            {
                rhs_block_base_indent = None;
                rhs_block_base_depth = None;
            }
        }
        // Continuation blocks:
        // - Multi-line `| ...` blocks (multi-clause functions and `?` matches).
        //   These blocks can contain continuation lines (e.g. multi-line patterns/bodies), so we
        //   keep the block active until we hit a same-indent non-`|` line (or a blank line).
        // - Multi-line `|> ...` pipeline blocks (common after `=`, even when RHS starts on same line).
        // - A single continuation line after a trailing `=` (e.g. `x =\n  expr`).
        let starts_with_pipe = state.tokens[first_idx].text == "|";
        let starts_with_pipeop = state.tokens[first_idx].text == "|>";
        let is_arm_line =
            starts_with_pipe && find_top_level_token(&state.tokens, "=>", first_idx + 1).is_some();
        let should_start_pipe_block =
            starts_with_pipe && matches!(prev_non_blank_last_token.as_deref(), Some("=") | Some("?"));
        let should_start_pipeop_block = starts_with_pipeop
            && (pipeop_seed_match
                || matches!(prev_non_blank_last_token.as_deref(), Some("=") | Some("?")));

        if should_start_pipe_block {
            // Anchor `|` blocks to the subject line's effective indentation so arms align even
            // when the subject is itself indented by other continuation rules.
            pipe_block_stack.push((prev_effective_indent_len, line_depth));
        }
        if should_start_pipeop_block {
            pipeop_block_base_indent = Some(prev_effective_indent_len);
            pipeop_block_base_depth = Some(line_depth);
        }

        // Close any nested `|` blocks we've left by delimiter nesting.
        while pipe_block_stack
            .last()
            .is_some_and(|&(_, base_depth)| line_depth < base_depth)
        {
            pipe_block_stack.pop();
        }

        // For `|`/`|>` lines, anchor indentation to the subject line's indent (not just delimiter nesting).
        let mut base_indent_len_for_line = line_indent_len;
        let hang_top = hang_delim_stack.last().copied();
        let hang_is_close = hang_top.is_some_and(|(opener, _)| {
            matches_hang_close(opener, state.tokens[first_idx].text.as_str())
        });
        // Suppress extra continuation indentation inside multi-line "hanging" delimiter groups.
        // The hang stack already aligns contents to the opener's *effective* indentation (which
        // includes any continuation indentation on the opener line), so adding continuation
        // levels again would double-indent.
        let inside_hang = hang_top.is_some_and(|(_, opener_indent)| {
            !hang_is_close && (opener_indent + indent_size) > line_indent_len
        });
        if starts_with_pipe {
            if let Some(&(base, _)) = pipe_block_stack.last() {
                // Indent arms one level relative to the match subject.
                base_indent_len_for_line = base + indent_size;
            }
        } else if starts_with_pipeop {
            if let Some(base) = pipeop_block_base_indent {
                base_indent_len_for_line = base + indent_size;
            }
        }
        if let Some((_, opener_indent)) = hang_top {
            if hang_is_close {
                // Closers align with their opener (not with any other continuation blocks).
                base_indent_len_for_line = opener_indent;
            } else {
                base_indent_len_for_line = base_indent_len_for_line.max(opener_indent + indent_size);
            }
        }
        // End a continuation block when we hit a line that clearly starts a new statement at or
        // above the block's base indentation. Avoid ending blocks just because a line starts with
        // a closing delimiter (`}`/`]`/`)`) which naturally decreases the computed indent.
        if let Some(&(_base_indent, base_depth)) = pipe_block_stack.last() {
            if !starts_with_pipe
                && !starts_with_pipeop
                && open_depth == 0
                && line_depth <= base_depth
                && looks_like_new_stmt(&state.tokens, first_idx)
            {
                pipe_block_stack.pop();
                if pipe_block_stack.is_empty() {
                    arm_rhs_active = false;
                }
            }
        }
        if let (Some(_base_indent), Some(base_depth)) = (pipeop_block_base_indent, pipeop_block_base_depth)
        {
            if !starts_with_pipeop
                && open_depth == 0
                && line_depth <= base_depth
                && looks_like_new_stmt(&state.tokens, first_idx)
            {
                pipeop_block_base_indent = None;
                pipeop_block_base_depth = None;
            }
        }

        let in_pipe_block = !pipe_block_stack.is_empty();
        let in_pipeop_block = pipeop_block_base_indent.is_some();
        let in_rhs_block = rhs_block_base_indent.is_some();

        let mut continuation_levels = 0usize;
        if !inside_hang && (in_pipe_block || in_pipeop_block) && !starts_with_pipe && !starts_with_pipeop {
            continuation_levels += 1;
        }
        if !inside_hang && in_rhs_block && !starts_with_pipe && !starts_with_pipeop {
            continuation_levels += 1;
        }
        // If a line ended with `=`/`=>` and did not open a delimiter group, indent the next line.
        // Avoid double-indenting `|`/`|>` continuation blocks after `=`/`?`.
        let rhs_seed_active = rhs_seed_indent.is_some()
            && !starts_with_pipe
            && !starts_with_pipeop
            && !in_rhs_block
            && (rhs_seed_depth == 0 || prev_non_blank_last_token.as_deref() == Some("=>"));
        if !inside_hang && rhs_seed_active {
            continuation_levels += 1;
        }
        if !inside_hang && arm_rhs_active && !starts_with_pipe {
            continuation_levels += 1;
        }
        if hang_is_close {
            // Standalone closers align with their opener and should not inherit continuation
            // indentation. Exceptions like `} } => ...` inside match arms should keep the arm
            // indentation so `=>` stays aligned.
            let has_arrow = find_top_level_token_clamped(&state.tokens, "=>", first_idx).is_some();
            let has_else = find_top_level_token_clamped(&state.tokens, "else", first_idx).is_some();
            if !(has_arrow && !has_else) {
                continuation_levels = 0;
            }
        }

        // Base indentation including continuation blocks, but excluding multiline `if` handling.
        let effective_indent_len_pre_if =
            base_indent_len_for_line + (continuation_levels * indent_size);

        // If we start a new statement at or above an `if`'s indentation, we left that `if`.
        // This is intentionally conservative to avoid popping while still inside branch bodies.
        if looks_like_new_stmt(&state.tokens, first_idx) {
            while if_stack
                .last()
                .is_some_and(|f| effective_indent_len_pre_if <= f.if_indent)
            {
                if_stack.pop();
            }
        }

        let mut effective_indent_len = effective_indent_len_pre_if;

        // Persistent `if`/`then`/`else` indentation (fixes nested ifs).
        //
        // - Body lines are indented one level relative to their `if` header.
        // - `else` header lines align with their matching `if`.
        // - `} else {` is handled by delimiter/hang indentation; we only update stack state.
        if !hang_is_close {
            let first_text = state.tokens[first_idx].text.as_str();
            let is_else_line = first_text == "else";

            if is_else_line {
                // We're starting an `else` header; any completed inner `else` branches end here.
                while if_stack.last().is_some_and(|f| f.phase == IfPhase::Else) {
                    if_stack.pop();
                }

                if let Some(idx) = if_stack.iter().rposition(|f| f.phase == IfPhase::Then) {
                    let outer_body_indent = if_stack
                        .iter()
                        .take(idx)
                        .filter(|f| f.active_indent)
                        .map(|f| f.if_indent + indent_size)
                        .max()
                        .unwrap_or(0);
                    effective_indent_len = outer_body_indent.max(if_stack[idx].if_indent);
                }
            } else if let Some(min_indent) = if_stack
                .iter()
                .filter(|f| f.active_indent)
                .map(|f| f.if_indent + indent_size)
                .max()
            {
                effective_indent_len = effective_indent_len.max(min_indent);
            }
        }

        let is_decorator_line = state.tokens[first_idx].text == "@";
        let is_decorator_only_line = is_decorator_line
            && find_top_level_token(&state.tokens, "=", first_idx).is_none()
            && find_top_level_token(&state.tokens, ":", first_idx).is_none();

        let effective_indent = " ".repeat(effective_indent_len);

        if let Some(max_lhs) = state.effect_align_lhs {
            if let Some(arrow_idx) = find_top_level_token(&state.tokens, "<-", first_idx) {
                // `<-` alignment across consecutive effect lines.
                let lhs_tokens = &state.tokens[first_idx..arrow_idx];
                let rhs_tokens = &state.tokens[arrow_idx + 1..];
                let lhs = format_tokens_simple(lhs_tokens, state.top_delim)
                    .trim()
                    .to_string();
                let rhs = format_tokens_simple(rhs_tokens, state.top_delim)
                    .trim()
                    .to_string();
                let spaces = (max_lhs.saturating_sub(lhs.len())) + 1;
                out.push_str(&effective_indent);
                out.push_str(&lhs);
                out.push_str(&" ".repeat(spaces));
                out.push_str("<-");
                if !rhs.is_empty() {
                    out.push(' ');
                    out.push_str(&rhs);
                }
                rendered_lines.push(out);
                prev_effective_indent_len = effective_indent_len;
                prev_non_blank_last_token = last_continuation_token(&state.tokens);
                if should_pop_hang {
                    hang_delim_stack.pop();
                }
                if let Some(last) = last_code_token(&state.tokens) {
                    if let Some(open) = is_open_sym(&last) {
                        hang_delim_stack.push((open, prev_effective_indent_len));
                    }
                }
                if let Some(else_idx) =
                    find_top_level_token_clamped(&state.tokens, "else", first_idx)
                {
                    if let Some(idx) = if_stack.iter().rposition(|f| f.phase == IfPhase::Then) {
                        let else_inline = state.tokens.iter().skip(else_idx + 1).any(|t| {
                            t.kind != "comment" && t.text != "\n"
                        });
                        if_stack[idx].phase = IfPhase::Else;
                        if_stack[idx].active_indent = !else_inline;
                    }
                }
                if prev_non_blank_last_token.as_deref() == Some("then") {
                    if_stack.push(IfFrame {
                        if_indent: prev_effective_indent_len,
                        phase: IfPhase::Then,
                        active_indent: true,
                    });
                }
                if line_has_top_level_eq {
                    pipeop_seed_indent = Some(line_indent_len);
                }
                if seeds_rhs_continuation(prev_non_blank_last_token.as_deref()) {
                    let depth = net_open_depth(&state.tokens);
                    // If the line already opened a delimiter group (e.g. `=> {`), delimiter-based
                    // indentation handles the continuation; avoid a one-shot RHS indent.
                    if depth == 0 {
                        rhs_next_line_indent = Some(line_indent_len);
                        rhs_next_line_depth = Some(depth);
                        rhs_block_base_indent = Some(line_indent_len);
                        rhs_block_base_depth = Some(line_depth);
                    }
                }
                update_open_depth(&mut open_depth, &state.tokens);
                continue;
            }
        }

        if let Some(max_pat) = state.arm_align_pat {
            let arrow_idx = find_top_level_token(&state.tokens, "=>", first_idx + 1);
            if state.tokens[first_idx].text == "|" {
                if let Some(arrow_idx) = arrow_idx {
                    let pat_tokens = &state.tokens[first_idx + 1..arrow_idx];
                    let rhs_tokens = &state.tokens[arrow_idx + 1..];
                    let pat = format_tokens_simple(pat_tokens, state.top_delim)
                        .trim()
                        .to_string();
                    let rhs = format_tokens_simple(rhs_tokens, state.top_delim)
                        .trim()
                        .to_string();
                    let spaces = (max_pat.saturating_sub(pat.len())) + 1;
                    out.push_str(&effective_indent);
                    out.push_str("| ");
                    out.push_str(&pat);
                    out.push_str(&" ".repeat(spaces));
                    out.push_str("=>");
                    if !rhs.is_empty() {
                        out.push(' ');
                        out.push_str(&rhs);
                    }
                    rendered_lines.push(out);
                    prev_effective_indent_len = effective_indent_len;
                    prev_non_blank_last_token = last_continuation_token(&state.tokens);
                    if should_pop_hang {
                        hang_delim_stack.pop();
                    }
                    if let Some(last) = last_code_token(&state.tokens) {
                        if let Some(open) = is_open_sym(&last) {
                            hang_delim_stack.push((open, prev_effective_indent_len));
                        }
                    }
                    if let Some(else_idx) =
                        find_top_level_token_clamped(&state.tokens, "else", first_idx)
                    {
                        if let Some(idx) = if_stack.iter().rposition(|f| f.phase == IfPhase::Then) {
                            let else_inline = state.tokens.iter().skip(else_idx + 1).any(|t| {
                                t.kind != "comment" && t.text != "\n"
                            });
                            if_stack[idx].phase = IfPhase::Else;
                            if_stack[idx].active_indent = !else_inline;
                        }
                    }
                    if prev_non_blank_last_token.as_deref() == Some("then") {
                        if_stack.push(IfFrame {
                            if_indent: prev_effective_indent_len,
                            phase: IfPhase::Then,
                            active_indent: true,
                        });
                    }
                    if line_has_top_level_eq {
                        pipeop_seed_indent = Some(line_indent_len);
                    }
                    if seeds_rhs_continuation(prev_non_blank_last_token.as_deref()) {
                        let depth = net_open_depth(&state.tokens);
                        if depth == 0 {
                            rhs_next_line_indent = Some(line_indent_len);
                            rhs_next_line_depth = Some(depth);
                            rhs_block_base_indent = Some(line_indent_len);
                            rhs_block_base_depth = Some(line_depth);
                        }
                    }
                    update_open_depth(&mut open_depth, &state.tokens);
                    continue;
                }
            }
        }

        if let Some(max_key) = state.map_align_key {
                let arrow_idx = find_top_level_token(&state.tokens, "=>", first_idx);
                if let Some(arrow_idx) = arrow_idx {
                    let key_tokens = &state.tokens[first_idx..arrow_idx];
                    let rhs_tokens = &state.tokens[arrow_idx + 1..];
                    let key = format_tokens_simple(key_tokens, state.top_delim)
                        .trim()
                        .to_string();
                    let rhs = format_tokens_simple(rhs_tokens, state.top_delim)
                        .trim()
                        .to_string();
                let spaces = (max_key.saturating_sub(key.len())) + 1;
                out.push_str(&effective_indent);
                out.push_str(&key);
                out.push_str(&" ".repeat(spaces));
                out.push_str("=>");
                if !rhs.is_empty() {
                    out.push(' ');
                    out.push_str(&rhs);
                }
                rendered_lines.push(out);
                prev_effective_indent_len = effective_indent_len;
                prev_non_blank_last_token = last_continuation_token(&state.tokens);
                if should_pop_hang {
                    hang_delim_stack.pop();
                }
                if let Some(last) = last_code_token(&state.tokens) {
                    if let Some(open) = is_open_sym(&last) {
                        hang_delim_stack.push((open, prev_effective_indent_len));
                    }
                }
                if let Some(else_idx) =
                    find_top_level_token_clamped(&state.tokens, "else", first_idx)
                {
                    if let Some(idx) = if_stack.iter().rposition(|f| f.phase == IfPhase::Then) {
                        let else_inline = state.tokens.iter().skip(else_idx + 1).any(|t| {
                            t.kind != "comment" && t.text != "\n"
                        });
                        if_stack[idx].phase = IfPhase::Else;
                        if_stack[idx].active_indent = !else_inline;
                    }
                }
                if prev_non_blank_last_token.as_deref() == Some("then") {
                    if_stack.push(IfFrame {
                        if_indent: prev_effective_indent_len,
                        phase: IfPhase::Then,
                        active_indent: true,
                    });
                }
                if line_has_top_level_eq {
                    pipeop_seed_indent = Some(line_indent_len);
                }
                if seeds_rhs_continuation(prev_non_blank_last_token.as_deref()) {
                    let depth = net_open_depth(&state.tokens);
                    if depth == 0 {
                        rhs_next_line_indent = Some(line_indent_len);
                        rhs_next_line_depth = Some(depth);
                        rhs_block_base_indent = Some(line_indent_len);
                        rhs_block_base_depth = Some(line_depth);
                    }
                }
                update_open_depth(&mut open_depth, &state.tokens);
                continue;
            }
        }

        // Type signatures: `name : Type` (only when followed by a matching `name ... =` definition).
        if let Some(colon_idx) = find_top_level_token(&state.tokens, ":", first_idx) {
            if colon_idx > first_idx {
                let name_tokens = &state.tokens[first_idx..colon_idx];
                let rest_tokens = &state.tokens[colon_idx + 1..];
                let name_len = name_tokens.len();

                let mut next_line = None;
                for (j, line) in lines.iter().enumerate().skip(line_index + 1) {
                    if line.degraded || line.tokens.is_empty() {
                        continue;
                    }
                    next_line = Some(j);
                    break;
                }

                if let Some(j) = next_line {
                    if let Some(next_first) = first_code_index(&lines[j].tokens) {
                        let mut name_matches = true;
                        for k in 0..name_len {
                            let a = name_tokens.get(k).map(|t| t.text.as_str());
                            let b = lines[j].tokens.get(next_first + k).map(|t| t.text.as_str());
                            if a != b {
                                name_matches = false;
                                break;
                            }
                        }

                        if name_matches
                            && find_top_level_token(&lines[j].tokens, "=", next_first + name_len)
                                .is_some()
                        {
                            out.push_str(&effective_indent);
                            out.push_str(format_tokens_simple(name_tokens, state.top_delim).trim());
                            out.push_str(" : ");
                            out.push_str(format_tokens_simple(rest_tokens, state.top_delim).trim());
                            rendered_lines.push(out);
                            prev_effective_indent_len = effective_indent_len;
                            prev_non_blank_last_token = last_continuation_token(&state.tokens);
                            if should_pop_hang {
                                hang_delim_stack.pop();
                            }
                            if let Some(last) = last_code_token(&state.tokens) {
                                if let Some(open) = is_open_sym(&last) {
                                    hang_delim_stack.push((open, prev_effective_indent_len));
                                }
                            }
                            if let Some(else_idx) =
                                find_top_level_token_clamped(&state.tokens, "else", first_idx)
                            {
                                if let Some(idx) =
                                    if_stack.iter().rposition(|f| f.phase == IfPhase::Then)
                                {
                                    let else_inline = state.tokens.iter().skip(else_idx + 1).any(|t| {
                                        t.kind != "comment" && t.text != "\n"
                                    });
                                    if_stack[idx].phase = IfPhase::Else;
                                    if_stack[idx].active_indent = !else_inline;
                                }
                            }
                            if prev_non_blank_last_token.as_deref() == Some("then") {
                                if_stack.push(IfFrame {
                                    if_indent: prev_effective_indent_len,
                                    phase: IfPhase::Then,
                                    active_indent: true,
                                });
                            }
                            if line_has_top_level_eq {
                                pipeop_seed_indent = Some(line_indent_len);
                            }
                    if seeds_rhs_continuation(prev_non_blank_last_token.as_deref()) {
                        let depth = net_open_depth(&state.tokens);
                        if depth == 0 {
                            rhs_next_line_indent = Some(line_indent_len);
                            rhs_next_line_depth = Some(depth);
                            rhs_block_base_indent = Some(line_indent_len);
                            rhs_block_base_depth = Some(line_depth);
                        }
                    }
                            update_open_depth(&mut open_depth, &state.tokens);
                            continue;
                        }
                    }
                }
            }
        }

        out.push_str(&effective_indent);
        out.push_str(&format_tokens_simple(&state.tokens, state.top_delim));
        rendered_lines.push(out);
        prev_effective_indent_len = effective_indent_len;

        prev_non_blank_last_token = last_continuation_token(&state.tokens);
        if should_pop_hang {
            hang_delim_stack.pop();
        }
        if let Some(last) = last_code_token(&state.tokens) {
            if let Some(open) = is_open_sym(&last) {
                hang_delim_stack.push((open, prev_effective_indent_len));
            }
        }
        if let Some(else_idx) = find_top_level_token_clamped(&state.tokens, "else", first_idx) {
            if let Some(idx) = if_stack.iter().rposition(|f| f.phase == IfPhase::Then) {
                let else_inline = state.tokens.iter().skip(else_idx + 1).any(|t| {
                    t.kind != "comment" && t.text != "\n"
                });
                if_stack[idx].phase = IfPhase::Else;
                if_stack[idx].active_indent = !else_inline;
            }
        }
        if prev_non_blank_last_token.as_deref() == Some("then") {
            if_stack.push(IfFrame {
                if_indent: prev_effective_indent_len,
                phase: IfPhase::Then,
                active_indent: true,
            });
        }
        if line_has_top_level_eq {
            pipeop_seed_indent = Some(line_indent_len);
        }
        if seeds_rhs_continuation(prev_non_blank_last_token.as_deref()) {
            let depth = net_open_depth(&state.tokens);
            if depth == 0 {
                rhs_next_line_indent = Some(line_indent_len);
                rhs_next_line_depth = Some(depth);
                rhs_block_base_indent = Some(line_indent_len);
                rhs_block_base_depth = Some(line_depth);
            }
        }
        update_open_depth(&mut open_depth, &state.tokens);

        // Decorators on their own line are part of the following definition/type-sig, even in
        // RHS continuation blocks (e.g. `x =\n  @test\n  foo = ...`). Keep the RHS block alive
        // for the next non-blank line so the binding doesn't accidentally dedent.
        if rhs_block_base_indent.is_some() && is_decorator_only_line {
            rhs_decorator_pending = true;
        }

        // After rendering an arm line, keep an extra indentation level for its body until the next
        // arm (or until we leave the surrounding `|` block).
        if is_arm_line {
            arm_rhs_active = true;
        } else if starts_with_pipe {
            // Starting a new arm resets the body indent for this line.
            arm_rhs_active = false;
        }
    }

    // Strip leading blank lines to keep output stable when inputs start with a newline.
    let first_non_blank = rendered_lines
        .iter()
        .position(|line| !line.is_empty())
        .unwrap_or(rendered_lines.len());
    if first_non_blank > 0 {
        rendered_lines.drain(0..first_non_blank);
    }

    // Final render via the `Doc` renderer. Today we mostly use hardlines, but this keeps the
    // formatter architecture ready for width-aware grouping in future rules.
    let mut doc_items = Vec::with_capacity(rendered_lines.len().saturating_mul(2));
    for line in rendered_lines.into_iter() {
        doc_items.push(super::doc::Doc::text(line));
        doc_items.push(super::doc::Doc::hardline());
    }
    let mut result = super::doc::render(super::doc::Doc::concat(doc_items), options.max_width);

    // Ensure exactly one trailing newline.
    while result.ends_with('\n') {
        result.pop();
    }
    result.push('\n');
    result
}
