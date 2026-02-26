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
        MatSigil,
        Machine,
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
        machine_align: Option<(usize, usize, usize)>,
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
            .position(|t| t.kind != "comment" && t.text != "\n" && t.text != ";")
    }

    fn last_code_token_is(tokens: &[&crate::cst::CstToken], expected: &[&str]) -> bool {
        let Some(last) = tokens
            .iter()
            .rev()
            .find(|t| t.kind != "comment" && t.text != "\n" && t.text != ";")
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

        if adjacent_in_input
            && (curr_text == "(" || curr_text == "[")
            && !is_keyword(prev_text)
        {
            return false;
        }

        // Preserve negative numeric literals (`-1`) when authored adjacent.
        if adjacent_in_input && prev_text == "-" && curr_kind == "number" {
            return false;
        }

        // Keep indexing tight after a closed group when it was adjacent in source:
        // `(f x)[i]` stays `(f x)[i]`, but `(f x) [a, b]` (list arg) keeps its space.
        if curr_text == "[" && prev_text == ")" && adjacent_in_input {
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
            // Keywords (e.g. `then`, `else`) always need a space before `[`.
            if adjacent_in_input
                && !is_keyword(prev_text)
                && (is_word_kind(prev_kind) || matches!(prev_text, ")" | "]" | "}"))
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

        let mut out = String::new();
        let mut prevprev: Option<(&str, &str)> = None;
        let mut prev: Option<(&str, &str)> = None;
        let mut prev_token: Option<&crate::cst::CstToken> = None;
        let mut in_leading_commas = strip_commas;
        for (i, t) in tokens.iter().enumerate() {
            if in_leading_commas {
                if t.text == "," || t.text == ";" || t.kind == "comment" || t.text == "\n" {
                    if t.text == "," {
                        continue;
                    }
                } else {
                    in_leading_commas = false;
                }
            }
            if let Some(start) = trailing_commas_start {
                if i >= start && t.text == "," {
                    continue;
                }
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

    #[derive(Debug, Clone)]
    struct MatrixSigil {
        tag: String,
        rows: Vec<Vec<String>>,
    }

    #[derive(Debug, Clone)]
    struct MarkupSigil {
        open: &'static str,
        close: &'static str,
        body: String,
    }

    fn parse_markup_sigil(text: &str) -> Option<MarkupSigil> {
        for (open, close) in [("~<html>", "</html>"), ("~<gtk>", "</gtk>")] {
            if text.starts_with(open) && text.ends_with(close) {
                let body_start = open.chars().count();
                let body_end = text.chars().count().saturating_sub(close.chars().count());
                let body: String = text
                    .chars()
                    .skip(body_start)
                    .take(body_end.saturating_sub(body_start))
                    .collect();
                return Some(MarkupSigil { open, close, body });
            }
        }
        None
    }

    fn parse_open_markup_tag(tag_text: &str) -> Option<(String, Vec<String>, bool)> {
        let raw = tag_text.trim();
        if !raw.starts_with('<') || raw.starts_with("</") || !raw.ends_with('>') {
            return None;
        }
        let mut inner = raw
            .trim_start_matches('<')
            .trim_end_matches('>')
            .trim()
            .to_string();
        let self_close = inner.ends_with('/');
        if self_close {
            inner = inner.trim_end_matches('/').trim_end().to_string();
        }
        if inner.is_empty() {
            return None;
        }

        let chars: Vec<char> = inner.chars().collect();
        let mut i = 0usize;
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        let tag_name: String = chars[0..i].iter().collect();
        if tag_name.is_empty() {
            return None;
        }

        let mut attrs = Vec::new();
        while i < chars.len() {
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            if i >= chars.len() {
                break;
            }

            let name_start = i;
            while i < chars.len()
                && !chars[i].is_whitespace()
                && chars[i] != '='
                && chars[i] != '>'
                && chars[i] != '/'
            {
                i += 1;
            }
            let name: String = chars[name_start..i].iter().collect();
            if name.is_empty() {
                i += 1;
                continue;
            }

            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }

            if i < chars.len() && chars[i] == '=' {
                i += 1;
                while i < chars.len() && chars[i].is_whitespace() {
                    i += 1;
                }
                if i >= chars.len() {
                    attrs.push(name);
                    break;
                }
                let value_start = i;
                if chars[i] == '"' || chars[i] == '\'' {
                    let quote = chars[i];
                    i += 1;
                    while i < chars.len() {
                        if chars[i] == '\\' && i + 1 < chars.len() {
                            i += 2;
                            continue;
                        }
                        if chars[i] == quote {
                            i += 1;
                            break;
                        }
                        i += 1;
                    }
                } else if chars[i] == '{' {
                    let mut depth = 1isize;
                    i += 1;
                    let mut in_quote: Option<char> = None;
                    while i < chars.len() && depth > 0 {
                        let ch = chars[i];
                        if let Some(q) = in_quote {
                            if q != '`' && ch == '\\' && i + 1 < chars.len() {
                                i += 2;
                                continue;
                            }
                            if ch == q {
                                in_quote = None;
                            }
                            i += 1;
                            continue;
                        }
                        match ch {
                            '"' | '\'' | '`' => in_quote = Some(ch),
                            '{' => depth += 1,
                            '}' => depth -= 1,
                            _ => {}
                        }
                        i += 1;
                    }
                } else {
                    while i < chars.len() && !chars[i].is_whitespace() {
                        i += 1;
                    }
                }
                let value: String = chars[value_start..i.min(chars.len())].iter().collect();
                attrs.push(format!("{name}={value}"));
            } else {
                attrs.push(name);
            }
        }

        Some((tag_name, attrs, self_close))
    }

    fn normalize_top_level_colon_spacing(field: &str) -> String {
        let mut brace_depth = 0isize;
        let mut paren_depth = 0isize;
        let mut bracket_depth = 0isize;
        let mut in_quote: Option<char> = None;
        let mut escaped = false;
        let mut colon_idx: Option<usize> = None;

        for (idx, ch) in field.char_indices() {
            if let Some(q) = in_quote {
                if q != '`' && !escaped && ch == '\\' {
                    escaped = true;
                    continue;
                }
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == q {
                    in_quote = None;
                }
                continue;
            }

            match ch {
                '"' | '\'' | '`' => in_quote = Some(ch),
                '{' => brace_depth += 1,
                '}' => {
                    if brace_depth > 0 {
                        brace_depth -= 1;
                    }
                }
                '(' => paren_depth += 1,
                ')' => {
                    if paren_depth > 0 {
                        paren_depth -= 1;
                    }
                }
                '[' => bracket_depth += 1,
                ']' => {
                    if bracket_depth > 0 {
                        bracket_depth -= 1;
                    }
                }
                ':' if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 => {
                    colon_idx = Some(idx);
                    break;
                }
                _ => {}
            }
        }

        if let Some(idx) = colon_idx {
            let lhs = field[..idx].trim();
            let rhs = field[idx + 1..].trim();
            format!("{lhs}: {rhs}")
        } else {
            field.trim().to_string()
        }
    }

    fn split_top_level_commas(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut start = 0usize;
        let mut brace_depth = 0isize;
        let mut paren_depth = 0isize;
        let mut bracket_depth = 0isize;
        let mut in_quote: Option<char> = None;
        let mut escaped = false;

        for (idx, ch) in text.char_indices() {
            if let Some(q) = in_quote {
                if q != '`' && !escaped && ch == '\\' {
                    escaped = true;
                    continue;
                }
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == q {
                    in_quote = None;
                }
                continue;
            }

            match ch {
                '"' | '\'' | '`' => in_quote = Some(ch),
                '{' => brace_depth += 1,
                '}' => {
                    if brace_depth > 0 {
                        brace_depth -= 1;
                    }
                }
                '(' => paren_depth += 1,
                ')' => {
                    if paren_depth > 0 {
                        paren_depth -= 1;
                    }
                }
                '[' => bracket_depth += 1,
                ']' => {
                    if bracket_depth > 0 {
                        bracket_depth -= 1;
                    }
                }
                ',' if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 => {
                    let item = text[start..idx].trim();
                    if !item.is_empty() {
                        out.push(normalize_top_level_colon_spacing(item));
                    }
                    start = idx + ch.len_utf8();
                }
                _ => {}
            }
        }

        let tail = text[start..].trim();
        if !tail.is_empty() {
            out.push(normalize_top_level_colon_spacing(tail));
        }
        out
    }

    fn parse_wrapped_record_attr(attr: &str) -> Option<(String, Vec<String>)> {
        let (name, value) = attr.split_once('=')?;
        let value = value.trim();
        let outer = value.strip_prefix('{')?.strip_suffix('}')?.trim();
        let inner = outer.strip_prefix('{')?.strip_suffix('}')?.trim();
        if inner.is_empty() || !inner.contains(':') {
            return None;
        }
        let fields = split_top_level_commas(inner);
        if fields.is_empty() {
            return None;
        }
        Some((name.trim().to_string(), fields))
    }

    fn format_wrapped_record_attr_inline(name: &str, fields: &[String]) -> String {
        format!("{name}={{{{ {} }}}}", fields.join(", "))
    }

    fn format_markup_sigil(text: &str) -> Option<Vec<String>> {
        let MarkupSigil { open, close, body } = parse_markup_sigil(text)?;
        let body_chars: Vec<char> = body.chars().collect();
        let mut lines = vec![open.to_string()];
        let mut depth = 1usize;
        let mut i = 0usize;

        while i < body_chars.len() {
            let ch = body_chars[i];
            if ch.is_whitespace() {
                i += 1;
                continue;
            }

            if ch == '<' {
                if i + 3 < body_chars.len()
                    && body_chars[i + 1] == '!'
                    && body_chars[i + 2] == '-'
                    && body_chars[i + 3] == '-'
                {
                    let start = i;
                    i += 4;
                    while i + 2 < body_chars.len() {
                        if body_chars[i] == '-'
                            && body_chars[i + 1] == '-'
                            && body_chars[i + 2] == '>'
                        {
                            i += 3;
                            break;
                        }
                        i += 1;
                    }
                    let comment: String = body_chars[start..i.min(body_chars.len())].iter().collect();
                    lines.push(format!("{}{}", " ".repeat(depth * 2), comment.trim()));
                    continue;
                }

                let start = i;
                i += 1;
                let mut in_quote: Option<char> = None;
                let mut brace_depth = 0isize;
                while i < body_chars.len() {
                    let c = body_chars[i];
                    if let Some(q) = in_quote {
                        if q != '`' && c == '\\' && i + 1 < body_chars.len() {
                            i += 2;
                            continue;
                        }
                        if c == q {
                            in_quote = None;
                        }
                        i += 1;
                        continue;
                    }
                    match c {
                        '"' | '\'' | '`' => in_quote = Some(c),
                        '{' => brace_depth += 1,
                        '}' => {
                            if brace_depth > 0 {
                                brace_depth -= 1;
                            }
                        }
                        '>' if brace_depth == 0 => {
                            i += 1;
                            break;
                        }
                        _ => {}
                    }
                    i += 1;
                }
                if i > body_chars.len() {
                    return None;
                }
                let tag_text: String = body_chars[start..i.min(body_chars.len())].iter().collect();
                let trimmed = tag_text.trim();
                if trimmed.starts_with("</") {
                    let name = trimmed
                        .trim_start_matches("</")
                        .trim_end_matches('>')
                        .trim();
                    depth = depth.saturating_sub(1);
                    lines.push(format!("{}</{}>", " ".repeat(depth * 2), name));
                    continue;
                }
                let (tag, attrs, self_close) = parse_open_markup_tag(trimmed)?;
                let indent = " ".repeat(depth * 2);
                if attrs.len() < 5 {
                    let wrapped = attrs
                        .iter()
                        .enumerate()
                        .find_map(|(idx, attr)| parse_wrapped_record_attr(attr).map(|v| (idx, v)));

                    if let Some((wrapped_idx, (wrapped_name, wrapped_fields))) = wrapped {
                        if wrapped_fields.len() <= 3 {
                            let mut line = format!("{indent}<{tag}");
                            for attr in attrs.iter().take(wrapped_idx) {
                                line.push(' ');
                                line.push_str(attr);
                            }
                            line.push(' ');
                            line.push_str(&format_wrapped_record_attr_inline(
                                &wrapped_name,
                                &wrapped_fields,
                            ));
                            for attr in attrs.iter().skip(wrapped_idx + 1) {
                                line.push(' ');
                                line.push_str(attr);
                            }
                            if self_close {
                                line.push_str(" />");
                            } else {
                                line.push('>');
                            }
                            lines.push(line);
                        } else {
                            let mut open_line = format!("{indent}<{tag}");
                            for attr in attrs.iter().take(wrapped_idx) {
                                open_line.push(' ');
                                open_line.push_str(attr);
                            }
                            open_line.push(' ');
                            open_line.push_str(&format!("{wrapped_name}={{{{"));
                            lines.push(open_line);

                            let field_indent = format!("{indent}  ");
                            for (idx, field) in wrapped_fields.iter().enumerate() {
                                if idx + 1 == wrapped_fields.len() {
                                    lines.push(format!("{field_indent}{field}"));
                                } else {
                                    lines.push(format!("{field_indent}{field},"));
                                }
                            }

                            let mut close_line = format!("{indent}}}}}");
                            for attr in attrs.iter().skip(wrapped_idx + 1) {
                                close_line.push(' ');
                                close_line.push_str(attr);
                            }
                            if self_close {
                                close_line.push_str(" />");
                            } else {
                                close_line.push('>');
                            }
                            lines.push(close_line);
                        }
                    } else {
                        let mut line = format!("{indent}<{tag}");
                        for attr in attrs {
                            line.push(' ');
                            line.push_str(&attr);
                        }
                        if self_close {
                            line.push_str(" />");
                        } else {
                            line.push('>');
                        }
                        lines.push(line);
                    }
                } else {
                    lines.push(format!("{indent}<{tag}"));
                    let attr_indent = format!("{indent}  ");
                    for (idx, attr) in attrs.iter().enumerate() {
                        if idx + 1 == attrs.len() {
                            if self_close {
                                lines.push(format!("{attr_indent}{attr} />"));
                            } else {
                                lines.push(format!("{attr_indent}{attr}>"));
                            }
                        } else {
                            lines.push(format!("{attr_indent}{attr}"));
                        }
                    }
                }
                if !self_close {
                    depth += 1;
                }
                continue;
            }

            if ch == '{' {
                let start = i;
                i += 1;
                let mut brace_depth = 1isize;
                let mut in_quote: Option<char> = None;
                while i < body_chars.len() && brace_depth > 0 {
                    let c = body_chars[i];
                    if let Some(q) = in_quote {
                        if q != '`' && c == '\\' && i + 1 < body_chars.len() {
                            i += 2;
                            continue;
                        }
                        if c == q {
                            in_quote = None;
                        }
                        i += 1;
                        continue;
                    }
                    match c {
                        '"' | '\'' | '`' => in_quote = Some(c),
                        '{' => brace_depth += 1,
                        '}' => brace_depth -= 1,
                        _ => {}
                    }
                    i += 1;
                }
                let end = i.min(body_chars.len());
                let inner: String = body_chars[start + 1..end.saturating_sub(1)].iter().collect();
                let inner = inner.trim();
                if !inner.is_empty() {
                    lines.push(format!("{}{{ {inner} }}", " ".repeat(depth * 2)));
                }
                continue;
            }

            let start = i;
            while i < body_chars.len() && body_chars[i] != '<' && body_chars[i] != '{' {
                i += 1;
            }
            let text: String = body_chars[start..i].iter().collect();
            let text = text.trim();
            if !text.is_empty() {
                lines.push(format!("{}{}", " ".repeat(depth * 2), text));
            }
        }

        lines.push(close.to_string());
        Some(lines)
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
            for col in 0..max_cols {
                let value = row.get(col).map(String::as_str).unwrap_or("");
                let value_len = value.chars().count();
                let pad = widths[col].saturating_sub(value_len);
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
            // Must be checked before spacing logic to avoid inserting a phantom space.
            // Emit a space separator to prevent adjacent tokens from merging into a
            // different token (e.g. two separate `&` tokens becoming `&&`).
            if t.text == ";" {
                if !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                    current_col += 1;
                }
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

    // Pre-pass: split trailing closers (`}`/`]`/`)`) off a line onto their own
    // line when the line also contains other tokens and the matching opener lives
    // on a different source line.  This prevents the common pattern:
    //
    //   rgb = {
    //     r: 255, g: 0, b: 0 }      ← `}` should be on its own line
    //
    // from causing downstream indentation drift.
    {
        let mut split_raw_lines: Vec<&str> = Vec::with_capacity(raw_lines.len() + 16);
        let mut split_tokens_by_line: Vec<Vec<&crate::cst::CstToken>> =
            Vec::with_capacity(tokens_by_line.len() + 16);

        for (line_index, raw) in raw_lines.iter().enumerate() {
            let line_tokens = tokens_by_line[line_index].clone();
            let has_comment = line_tokens.iter().any(|t| t.kind == "comment");
            if has_comment || line_tokens.len() < 2 {
                split_raw_lines.push(*raw);
                split_tokens_by_line.push(line_tokens);
                continue;
            }
            // Check if the last code token is a closer.
            let last_close = last_code_token_is(&line_tokens, &["}", "]", ")"]);
            if !last_close {
                split_raw_lines.push(*raw);
                split_tokens_by_line.push(line_tokens);
                continue;
            }
            // Check that there are other code tokens besides the closer.
            let first_code = first_code_index(&line_tokens);
            let last_code_idx = line_tokens
                .iter()
                .rposition(|t| t.kind != "comment" && t.text != "\n" && t.text != ";");
            if first_code == last_code_idx {
                // Only token on the line is the closer — nothing to split.
                split_raw_lines.push(*raw);
                split_tokens_by_line.push(line_tokens);
                continue;
            }
            // Find the matching open delimiter for the trailing closer.
            // Walk backwards through all tokens up to this point to find the
            // opener.  If the opener is on the same source line, keep the line
            // intact (e.g. `{ r: 255, g: 0, b: 0 }` all on one source line).
            let closer_tok = line_tokens[last_code_idx.expect("infallible")];
            let _opener_char = match closer_tok.text.as_str() {
                "}" => "{",
                "]" => "[",
                ")" => "(",
                _ => {
                    split_raw_lines.push(*raw);
                    split_tokens_by_line.push(line_tokens);
                    continue;
                }
            };
            // Find matching opener by scanning backward through all token buckets.
            // On the current line, exclude the closer token itself from the scan so we
            // don't count it in the nesting depth.
            let mut depth = 0isize;
            let mut opener_line: Option<usize> = None;
            'outer: for scan_line in (0..=line_index).rev() {
                let scan_tokens = &tokens_by_line[scan_line];
                let end = if scan_line == line_index {
                    last_code_idx.expect("infallible")
                } else {
                    scan_tokens.len()
                };
                for t in scan_tokens[..end].iter().rev() {
                    if t.kind == "comment" || t.kind == "string" {
                        continue;
                    }
                    if is_close_sym(t.text.as_str()).is_some() {
                        depth += 1;
                    } else if is_open_sym(t.text.as_str()).is_some() {
                        if depth == 0 {
                            opener_line = Some(scan_line);
                            break 'outer;
                        }
                        depth -= 1;
                    }
                }
            }
            let same_line = opener_line == Some(line_index);
            if same_line || opener_line.is_none() {
                // Don't split if opener is on the same line, or if there is no
                // matching opener (unmatched closer — splitting would be
                // non-idempotent because the closer wasn't originally alone).
                split_raw_lines.push(*raw);
                split_tokens_by_line.push(line_tokens);
                continue;
            }
            // Split: everything except the closer goes on one line, the closer
            // goes on a new line.
            let mut before = line_tokens.clone();
            let closer = before.pop().expect("closer token");
            split_raw_lines.push(*raw);
            split_tokens_by_line.push(before);
            split_raw_lines.push("");
            split_tokens_by_line.push(vec![closer]);
        }

        raw_lines = split_raw_lines;
        tokens_by_line = split_tokens_by_line;
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
                            &["=", "=>", "<-", "->", "then", "else", "?", "match"],
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
    //     subject match
    //       | ...
    //
    // becomes
    //
    //   name = args => subject match
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
                    && last_code_token_is(&next_tokens, &["?", "match"])
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
    let mut machine_pending = false;

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
            machine_align: None,
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
            if text == "machine" {
                machine_pending = true;
            }
            if let Some(open) = is_open_sym(text) {
                let kind = match (
                    open,
                    prev_non_comment_text.as_deref(),
                    prevprev_non_comment_text.as_deref(),
                ) {
                    ('{', Some(monad), Some("do")) if !is_keyword(monad) => ContextKind::Effect,
                    ('{', Some("effect"), _) => ContextKind::Effect,
                    ('{', Some("generate"), _) => ContextKind::Generate,
                    ('{', Some("resource"), _) => ContextKind::Resource,
                    ('{', Some("map"), Some("~")) => ContextKind::MapSigil,
                    ('[', Some("set"), Some("~")) => ContextKind::SetSigil,
                    ('[', Some("mat"), Some("~")) => ContextKind::MatSigil,
                    _ => {
                        if machine_pending && open == '{' {
                            machine_pending = false;
                            ContextKind::Machine
                        } else {
                            ContextKind::Other
                        }
                    }
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

            // Machine transition alignment groups (inside `machine ... = { ... }`).
            if lines[i].top_context == Some(ContextKind::Machine) {
                if find_top_level_token(&lines[i].tokens, "->", first_idx).is_some() {
                    let mut j = i;
                    let mut max_source = 0usize;
                    let mut max_target = 0usize;
                    let mut max_event = 0usize;
                    while j < lines.len() {
                        if lines[j].tokens.is_empty() || lines[j].degraded {
                            break;
                        }
                        if lines[j].top_context != Some(ContextKind::Machine) {
                            break;
                        }
                        let first_idx_j = match first_code_index(&lines[j].tokens) {
                            Some(v) => v,
                            None => break,
                        };
                        let Some(arrow_idx) =
                            find_top_level_token(&lines[j].tokens, "->", first_idx_j)
                        else {
                            break;
                        };
                        // source = tokens before `->` (may be empty for initial transitions)
                        let source_tokens = &lines[j].tokens[first_idx_j..arrow_idx];
                        let source_str =
                            format_tokens_simple(source_tokens, lines[j].top_delim).trim().to_string();
                        max_source = max_source.max(source_str.len());
                        // target = tokens between `->` and first top-level `:`
                        let colon_idx = find_top_level_token(&lines[j].tokens, ":", arrow_idx + 1);
                        let target_end = colon_idx.unwrap_or(lines[j].tokens.len());
                        let target_tokens = &lines[j].tokens[arrow_idx + 1..target_end];
                        let target_str =
                            format_tokens_simple(target_tokens, lines[j].top_delim).trim().to_string();
                        max_target = max_target.max(target_str.len());
                        // event = tokens between `:` and first top-level `{` (or end)
                        if let Some(colon_idx) = colon_idx {
                            let brace_idx = find_top_level_token(&lines[j].tokens, "{", colon_idx + 1);
                            let event_end = brace_idx.unwrap_or(lines[j].tokens.len());
                            let event_tokens = &lines[j].tokens[colon_idx + 1..event_end];
                            let event_str =
                                format_tokens_simple(event_tokens, lines[j].top_delim).trim().to_string();
                            max_event = max_event.max(event_str.len());
                        }
                        j += 1;
                    }
                    if j > i {
                        for line in lines.iter_mut().take(j).skip(i) {
                            line.machine_align = Some((max_source, max_target, max_event));
                        }
                    }
                    i = j;
                    continue;
                }
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

        let preceded_by_blank = blank_run > 0;
        blank_run = 0;

        let mut out = String::new();

        // One-shot seeds: only apply to the next non-blank line.
        let rhs_seed_indent = rhs_next_line_indent.take();
        let rhs_seed_depth = rhs_next_line_depth.take().unwrap_or(0);
        let pipeop_seed = pipeop_seed_indent.take();

        if state.degraded {
            out.push_str(state.indent.as_str());
            out.push_str(&format_tokens_with_matrix(
                &state.tokens,
                state.top_delim,
                state.indent.as_str(),
            ));
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
            out.push_str(&format_tokens_with_matrix(
                &state.tokens,
                state.top_delim,
                state.indent.as_str(),
            ));
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
            let is_decorator_start = state.tokens[first_idx].text == "@";
            if line_depth <= base_depth
                && line_indent_len <= base_indent
                && (looks_like_new_stmt(&state.tokens, first_idx)
                    || (is_decorator_start && preceded_by_blank))
                && !rhs_decorator_pending_for_this_line
            {
                rhs_block_base_indent = None;
                rhs_block_base_depth = None;
            }
        }
        let is_decorator_line = state.tokens[first_idx].text == "@";
        let is_decorator_only_line = is_decorator_line
            && find_top_level_token(&state.tokens, "=", first_idx).is_none()
            && find_top_level_token(&state.tokens, ":", first_idx).is_none();

        // Continuation blocks:
        // - Multi-line `| ...` blocks (multi-clause functions and `match` expressions).
        //   These blocks can contain continuation lines (e.g. multi-line patterns/bodies), so we
        //   keep the block active until we hit a same-indent non-`|` line (or a blank line).
        // - Multi-line `|> ...` pipeline blocks (common after `=`, even when RHS starts on same line).
        // - A single continuation line after a trailing `=` (e.g. `x =\n  expr`).
        let starts_with_pipe = state.tokens[first_idx].text == "|";
        let starts_with_pipeop = state.tokens[first_idx].text == "|>";
        let is_arm_line =
            starts_with_pipe && find_top_level_token(&state.tokens, "=>", first_idx + 1).is_some();
        let should_start_pipe_block =
            starts_with_pipe && matches!(prev_non_blank_last_token.as_deref(), Some("=") | Some("?") | Some("match"));
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
        // A decorator on its own line conceptually belongs to the following item; it should align
        // with that item's indentation boundary instead of inheriting indentation from a preceding
        // `|`/`|>` continuation block.
        if !inside_hang
            && (in_pipe_block || in_pipeop_block)
            && !starts_with_pipe
            && !starts_with_pipeop
            && !is_decorator_only_line
        {
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
        if !inside_hang && arm_rhs_active && !starts_with_pipe && !is_decorator_only_line {
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

        if let Some((max_source, max_target, max_event)) = state.machine_align {
            if let Some(arrow_idx) = find_top_level_token(&state.tokens, "->", first_idx) {
                // source = tokens before `->`
                let source_tokens = &state.tokens[first_idx..arrow_idx];
                let source = format_tokens_simple(source_tokens, state.top_delim)
                    .trim()
                    .to_string();
                // target = tokens between `->` and first top-level `:`
                let colon_idx = find_top_level_token(&state.tokens, ":", arrow_idx + 1);
                let target_end = colon_idx.unwrap_or(state.tokens.len());
                let target_tokens = &state.tokens[arrow_idx + 1..target_end];
                let target = format_tokens_simple(target_tokens, state.top_delim)
                    .trim()
                    .to_string();
                // event = tokens between `:` and first top-level `{`
                let (event, payload) = if let Some(colon_idx) = colon_idx {
                    let brace_idx =
                        find_top_level_token(&state.tokens, "{", colon_idx + 1);
                    let event_end = brace_idx.unwrap_or(state.tokens.len());
                    let event_tokens = &state.tokens[colon_idx + 1..event_end];
                    let event_str = format_tokens_simple(event_tokens, state.top_delim)
                        .trim()
                        .to_string();
                    let payload_str = if let Some(brace_idx) = brace_idx {
                        let payload_tokens = &state.tokens[brace_idx..];
                        format_tokens_simple(payload_tokens, state.top_delim)
                            .trim()
                            .to_string()
                    } else {
                        String::new()
                    };
                    (event_str, payload_str)
                } else {
                    (String::new(), String::new())
                };

                out.push_str(&effective_indent);
                // Source column (left-aligned, padded to max_source width)
                if !source.is_empty() {
                    out.push_str(&source);
                    out.push_str(&" ".repeat(max_source.saturating_sub(source.len())));
                } else {
                    out.push_str(&" ".repeat(max_source));
                }
                if max_source > 0 {
                    out.push(' ');
                }
                out.push_str("->");
                out.push(' ');
                out.push_str(&target);
                // Pad target to max_target width
                let target_pad = max_target.saturating_sub(target.len());
                out.push_str(&" ".repeat(target_pad));
                if !event.is_empty() || !payload.is_empty() {
                    out.push_str(" : ");
                    out.push_str(&event);
                    if !payload.is_empty() {
                        let event_pad = max_event.saturating_sub(event.len());
                        out.push_str(&" ".repeat(event_pad));
                        out.push(' ');
                        out.push_str(&payload);
                    }
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
        out.push_str(&format_tokens_with_matrix(
            &state.tokens,
            state.top_delim,
            &effective_indent,
        ));
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

    // Align consecutive single-line records with identical field structure inside list literals.
    // E.g. a list of `{ key: "n", modifiers: "ctrl", action: "compose", label: "New email" }`
    // lines gets their corresponding field values aligned to the same column.
    {
        /// Split `s` by top-level commas (respecting `{}`, `()`, `[]`, and `"…"` strings).
        fn split_top_level_commas(s: &str) -> Vec<String> {
            let mut result = Vec::new();
            let mut depth: i32 = 0;
            let mut in_string = false;
            let mut escape = false;
            let mut start = 0usize;
            let bytes = s.as_bytes();
            let mut i = 0usize;
            while i < bytes.len() {
                let b = bytes[i];
                if escape {
                    escape = false;
                    i += 1;
                    continue;
                }
                if in_string {
                    if b == b'\\' {
                        escape = true;
                    } else if b == b'"' {
                        in_string = false;
                    }
                    i += 1;
                    continue;
                }
                match b {
                    b'"' => in_string = true,
                    b'{' | b'(' | b'[' => depth += 1,
                    b'}' | b')' | b']' => depth -= 1,
                    b',' if depth == 0 => {
                        result.push(s[start..i].trim().to_string());
                        start = i + 1;
                    }
                    _ => {}
                }
                i += 1;
            }
            let tail = s[start..].trim();
            if !tail.is_empty() {
                result.push(tail.to_string());
            }
            result
        }

        /// Parse a rendered line as an inline record: returns `(indent, fields)` where each
        /// field is the trimmed `key: value` string (without surrounding comma).
        /// Returns `None` if the line is not a single-line record.
        fn parse_inline_record(line: &str) -> Option<(String, Vec<String>)> {
            let trimmed_end = line.trim_end_matches(|c: char| c == ' ' || c == '\t');
            let indent_len = trimmed_end.len() - trimmed_end.trim_start().len();
            let indent = trimmed_end[..indent_len].to_string();
            let inner = trimmed_end.trim_start();
            // Must look like `{ ... }`; no nested `{` allowed at depth 0 in the content.
            if !inner.starts_with('{') || !inner.ends_with('}') {
                return None;
            }
            let content = inner[1..inner.len() - 1].trim();
            if content.is_empty() {
                return None;
            }
            let fields = split_top_level_commas(content);
            // Every field must look like `ident: value` (at least one `:` after an identifier).
            for f in &fields {
                let f = f.trim();
                let colon = f.find(':')?;
                let key = f[..colon].trim();
                if key.is_empty() || key.contains(' ') {
                    return None;
                }
            }
            if fields.is_empty() {
                return None;
            }
            Some((indent, fields))
        }

        /// Extract the field key (before the first `:`).
        fn field_key(field: &str) -> &str {
            field.split(':').next().map(str::trim).unwrap_or("")
        }

        let mut i = 0usize;
        while i < rendered_lines.len() {
            let Some((indent0, fields0)) = parse_inline_record(&rendered_lines[i]) else {
                i += 1;
                continue;
            };
            // Collect the run of consecutive same-structure records at the same indent.
            let mut j = i + 1;
            while j < rendered_lines.len() {
                if let Some((ind, flds)) = parse_inline_record(&rendered_lines[j]) {
                    // Same indent and same field keys in same order.
                    if ind == indent0
                        && flds.len() == fields0.len()
                        && flds
                            .iter()
                            .zip(fields0.iter())
                            .all(|(a, b)| field_key(a) == field_key(b))
                    {
                        j += 1;
                        continue;
                    }
                }
                break;
            }
            if j - i >= 2 {
                // Compute max width per field position (for all but the last field).
                let n_fields = fields0.len();
                let mut max_widths = vec![0usize; n_fields.saturating_sub(1)];
                for line in &rendered_lines[i..j] {
                    if let Some((_, flds)) = parse_inline_record(line) {
                        for (k, w) in max_widths.iter_mut().enumerate() {
                            *w = (*w).max(flds[k].len());
                        }
                    }
                }
                // Re-render each record with padding.
                for line in rendered_lines[i..j].iter_mut() {
                    if let Some((ind, flds)) = parse_inline_record(line) {
                        let mut s = ind;
                        s.push('{');
                        for (k, f) in flds.iter().enumerate() {
                            if k == 0 {
                                s.push(' ');
                            }
                            s.push_str(f);
                            if k + 1 < flds.len() {
                                s.push(',');
                                // Pad after the comma so the next field starts at a fixed column.
                                let pad = max_widths[k].saturating_sub(f.len()) + 1;
                                s.push_str(&" ".repeat(pad));
                            }
                        }
                        s.push_str(" }");
                        *line = s;
                    }
                }
            }
            i = j;
        }
    }

    // Final render via the `Doc` renderer. Today we mostly use hardlines, but this keeps the
    // formatter architecture ready for width-aware grouping in future rules.
    let mut doc_items = Vec::with_capacity(rendered_lines.len().saturating_mul(2));
    for line in rendered_lines.into_iter() {
        // Strip trailing whitespace so formatting is idempotent.
        // Only strip ASCII whitespace (space/tab) — not all Unicode whitespace —
        // to avoid removing unknown tokens (e.g. \x0c form-feed) that the lexer
        // emits as content tokens, which would change the token structure between
        // formatting passes.
        let trimmed = line.trim_end_matches(|c: char| c == ' ' || c == '\t').to_string();
        doc_items.push(super::doc::Doc::text(trimmed));
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
