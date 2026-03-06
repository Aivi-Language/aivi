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
    bind_align_lhs: Option<usize>,
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
    expected.contains(&last.text.as_str())
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

    // Date/Time fragments: no space around '-' or ':' if surrounded by numbers
    // and adjacent in the source (e.g. `2023-01-01`). When there is a space in
    // the source (e.g. `3.0 - 8.0`), treat `-` as a binary operator.
    if adjacent_in_input && prev_kind == "number" && curr_text == "-" {
        return false;
    }
    if prev_text == "-" && curr_kind == "number" {
        // Date fragments: `YYYY-MM` (no spaces) but keep binary minus spacing (`x - 1`).
        if adjacent_in_input && prevprev.is_some_and(|(k, _)| k == "number") {
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

        // Skip stray `;` tokens consistently with the main formatting loop.
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
