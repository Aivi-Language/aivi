use crate::lexer::lex;
use crate::syntax;

pub fn format_text(content: &str) -> String {
    let (tokens, _) = lex(content);

    let raw_lines: Vec<&str> = content.split('\n').collect();
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

    fn find_top_level_token(
        tokens: &[&crate::cst::CstToken],
        needle: &str,
        start: usize,
    ) -> Option<usize> {
        let mut depth = 0isize;
        for (i, t) in tokens.iter().enumerate().skip(start) {
            let text = t.text.as_str();
            if t.kind == "string" || t.kind == "sigil" {
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

        if prev_text == "~" || prev_text == "@" || prev_text == "." || prev_text == "..." {
            return false;
        }
        if curr_text == "," || curr_text == ";" || curr_text == ")" || curr_text == "]" {
            return false;
        }
        if prev_text == "," || prev_text == ";" {
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
            return false;
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
            if is_word_kind(prev_kind) || matches!(prev_text, ")" | "]" | "}") {
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

        // Unary +/-: no space between sign and number if it doesn't follow a binary precursor.
        if (prev_text == "-" || prev_text == "+") && curr_kind == "number" {
            let precursor = prevprev.map(|(_, t)| t).unwrap_or("");
            if precursor.is_empty()
                || matches!(precursor, "(" | "[" | "{" | "," | ":" | "=" | "->" | "=>" | "<-" | "|>" | "<|" | "?" | "|")
                || is_op(precursor)
            {
                return false;
            }
        }

        // Always space after keywords before words/symbol groups like `effect {`.
        if is_keyword(prev_text) {
            return true;
        }

        if prev_text == "=" || prev_text == "=>" || prev_text == "<-" || prev_text == "->" || prev_text == "|>" || prev_text == "<|" {
            return true;
        }
        if curr_text == "=" || curr_text == "=>" || curr_text == "<-" || curr_text == "->" || curr_text == "|>" || curr_text == "<|" {
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
        if prev_text == "}" && (is_word_kind(curr_kind) || is_keyword(curr_text) || curr_text == "(") {
            return true;
        }
        if prev_text == "]" && (is_word_kind(curr_kind) || is_keyword(curr_text) || curr_text == "(") {
            return true;
        }

        false
    }

    fn format_tokens_simple(tokens: &[&crate::cst::CstToken]) -> String {
        let mut out = String::new();
        let mut prevprev: Option<(&str, &str)> = None;
        let mut prev: Option<(&str, &str)> = None;
        let mut prev_token: Option<&crate::cst::CstToken> = None;
        for t in tokens.iter() {
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
                p.span.start.line == t.span.start.line && p.span.end.column + 1 == t.span.start.column
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

    // First pass: compute context per line (indentation is preserved from input).
    let mut stack: Vec<OpenFrame> = Vec::new();
    let mut degraded = false;
    let mut prev_non_comment_text: Option<String> = None;
    let mut prevprev_non_comment_text: Option<String> = None;

    let mut lines: Vec<LineState<'_>> = Vec::with_capacity(raw_lines.len());

    for line_index in 0..raw_lines.len() {
        let mut line_tokens = tokens_by_line[line_index].clone();
        line_tokens.sort_by_key(|t| (t.span.start.column, t.span.end.column));
        let (indent, indent_len) = leading_indent(raw_lines[line_index]);
        let top_context = stack.last().map(|f| f.kind);

        lines.push(LineState {
            tokens: line_tokens,
            indent,
            indent_len,
            top_context,
            effect_align_lhs: None,
            arm_align_pat: None,
            map_align_key: None,
            degraded,
        });

        if degraded {
            continue;
        }

        for t in &tokens_by_line[line_index] {
            if t.kind == "comment" {
                continue;
            }
            let text = t.text.as_str();
            if let Some(open) = is_open_sym(text) {
                let kind = match (open, prev_non_comment_text.as_deref(), prevprev_non_comment_text.as_deref()) {
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
                // Effect bind alignment groups: consecutive `ident <- ...` lines, unbroken.
                let (is_bind, lhs_len) = {
                    let t0 = lines[i].tokens.get(first_idx);
                    let t1 = lines[i].tokens.get(first_idx + 1);
                    let ok = matches!((t0, t1), (Some(a), Some(b)) if a.kind == "ident" && (a.text == "_" || a.kind == "ident") && b.text == "<-");
                    if ok {
                        (true, lines[i].tokens[first_idx].text.len())
                    } else {
                        (false, 0)
                    }
                };
                if is_bind {
                    let mut j = i;
                    let mut max_lhs = lhs_len;
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
                        let t0 = lines[j].tokens.get(first_idx_j);
                        let t1 = lines[j].tokens.get(first_idx_j + 1);
                        let ok = matches!((t0, t1), (Some(a), Some(b)) if a.kind == "ident" && (a.text == "_" || a.kind == "ident") && b.text == "<-");
                        if !ok {
                            break;
                        }
                        max_lhs = max_lhs.max(lines[j].tokens[first_idx_j].text.len());
                        j += 1;
                    }
                    if j - i >= 2 {
                        for k in i..j {
                            lines[k].effect_align_lhs = Some(max_lhs);
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
                    let Some(arrow_idx) = find_top_level_token(&lines[j].tokens, "=>", first_idx_j + 1)
                    else {
                        break;
                    };
                    let pat_tokens = &lines[j].tokens[first_idx_j + 1..arrow_idx];
                    let pat_str = format_tokens_simple(pat_tokens).trim().to_string();
                    max_pat = max_pat.max(pat_str.len());
                    j += 1;
                }
                if j - i >= 2 {
                    for k in i..j {
                        lines[k].arm_align_pat = Some(max_pat);
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
                    let key_str = format_tokens_simple(key_tokens).trim().to_string();
                    max_key = max_key.max(key_str.len());
                    j += 1;
                }
                if j - i >= 2 {
                    for k in i..j {
                        lines[k].map_align_key = Some(max_key);
                    }
                }
                i = j;
                continue;
            }
        }

        i += 1;
    }

    // Third pass: render.
    let mut out = String::new();
    for (line_index, state) in lines.iter().enumerate() {
        if line_index > 0 {
            out.push('\n');
        }

        if state.tokens.is_empty() {
            continue;
        }

        let indent = state.indent.as_str();

        if state.degraded {
            out.push_str(&indent);
            out.push_str(&format_tokens_simple(&state.tokens));
            continue;
        }

        let Some(first_idx) = first_code_index(&state.tokens) else {
            out.push_str(&indent);
            out.push_str(&format_tokens_simple(&state.tokens));
            continue;
        };

        if let Some(max_lhs) = state.effect_align_lhs {
            // `x <- expr` alignment
            let lhs = state.tokens[first_idx].text.as_str();
            let rhs_tokens = &state.tokens[first_idx + 2..];
            let rhs = format_tokens_simple(rhs_tokens).trim().to_string();
            let spaces = (max_lhs.saturating_sub(lhs.len())) + 1;
            out.push_str(&indent);
            out.push_str(lhs);
            out.push_str(&" ".repeat(spaces));
            out.push_str("<-");
            if !rhs.is_empty() {
                out.push(' ');
                out.push_str(&rhs);
            }
            continue;
        }

        if let Some(max_pat) = state.arm_align_pat {
            let arrow_idx = find_top_level_token(&state.tokens, "=>", first_idx + 1);
            if state.tokens[first_idx].text == "|" && arrow_idx.is_some() {
                let arrow_idx = arrow_idx.unwrap();
                let pat_tokens = &state.tokens[first_idx + 1..arrow_idx];
                let rhs_tokens = &state.tokens[arrow_idx + 1..];
                let pat = format_tokens_simple(pat_tokens).trim().to_string();
                let rhs = format_tokens_simple(rhs_tokens).trim().to_string();
                let spaces = (max_pat.saturating_sub(pat.len())) + 1;
                out.push_str(&indent);
                out.push_str("| ");
                out.push_str(&pat);
                out.push_str(&" ".repeat(spaces));
                out.push_str("=>");
                if !rhs.is_empty() {
                    out.push(' ');
                    out.push_str(&rhs);
                }
                continue;
            }
        }

        if let Some(max_key) = state.map_align_key {
            let arrow_idx = find_top_level_token(&state.tokens, "=>", first_idx);
            if arrow_idx.is_some() {
                let arrow_idx = arrow_idx.unwrap();
                let key_tokens = &state.tokens[first_idx..arrow_idx];
                let rhs_tokens = &state.tokens[arrow_idx + 1..];
                let key = format_tokens_simple(key_tokens).trim().to_string();
                let rhs = format_tokens_simple(rhs_tokens).trim().to_string();
                let spaces = (max_key.saturating_sub(key.len())) + 1;
                out.push_str(&indent);
                out.push_str(&key);
                out.push_str(&" ".repeat(spaces));
                out.push_str("=>");
                if !rhs.is_empty() {
                    out.push(' ');
                    out.push_str(&rhs);
                }
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
                for j in (line_index + 1)..lines.len() {
                    if lines[j].degraded || lines[j].tokens.is_empty() {
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
                            let b = lines[j]
                                .tokens
                                .get(next_first + k)
                                .map(|t| t.text.as_str());
                            if a != b {
                                name_matches = false;
                                break;
                            }
                        }

                        if name_matches
                            && find_top_level_token(&lines[j].tokens, "=", next_first + name_len)
                                .is_some()
                        {
                            out.push_str(&indent);
                            out.push_str(&format_tokens_simple(name_tokens).trim().to_string());
                            out.push_str(" : ");
                            out.push_str(&format_tokens_simple(rest_tokens).trim().to_string());
                            continue;
                        }
                    }
                }
            }
        }

        out.push_str(&indent);
        out.push_str(&format_tokens_simple(&state.tokens));
    }

    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn is_op(text: &str) -> bool {
    matches!(
        text,
        "=" | "+"
            | "-"
            | "*"
            | "/"
            | "%"
            | "->"
            | "=>"
            | "<-"
            | "<|"
            | "|>"
            | "?"
            | "|"
            | "++"
            | "::"
            | ".."
            | ":="
            | "??"
            | "^"
            | "=="
            | "!="
            | "<"
            | ">"
            | "<="
            | ">="
            | "&&"
            | "||"
    )
}
