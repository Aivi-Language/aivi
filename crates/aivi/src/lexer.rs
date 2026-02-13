use crate::cst::CstToken;
use crate::diagnostics::{Diagnostic, DiagnosticLabel, DiagnosticSeverity, Position, Span};
use crate::syntax;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Ident,
    Number,
    String,
    Sigil,
    Symbol,
    Newline,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub span: Span,
}

pub fn lex(content: &str) -> (Vec<CstToken>, Vec<Diagnostic>) {
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();

    // Scan across the full source so we can support multiline sigils like `~html{ ... }`.
    let chars: Vec<char> = content.chars().collect();
    let mut index = 0usize;
    let mut line = 1usize;
    let mut col = 1usize;

    while index < chars.len() {
        let ch = chars[index];

        if ch == '\n' {
            index += 1;
            line += 1;
            col = 1;
            continue;
        }

        if ch == ' ' || ch == '\t' {
            let start = index;
            let start_col = col;
            while index < chars.len() && (chars[index] == ' ' || chars[index] == '\t') {
                index += 1;
                col += 1;
            }
            let text: String = chars[start..index].iter().collect();
            tokens.push(CstToken {
                kind: "whitespace".to_string(),
                text,
                span: span_single(line, start_col, index - start),
            });
            continue;
        }

        // Line comments (`// ...` and `-- ...`) run to end-of-line.
        if ch == '/' && index + 1 < chars.len() && chars[index + 1] == '/' {
            let start = index;
            let start_col = col;
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
                col += 1;
            }
            let text: String = chars[start..index].iter().collect();
            tokens.push(CstToken {
                kind: "comment".to_string(),
                text,
                span: span_single(line, start_col, index - start),
            });
            continue;
        }
        if ch == '-' && index + 1 < chars.len() && chars[index + 1] == '-' {
            let start = index;
            let start_col = col;
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
                col += 1;
            }
            let text: String = chars[start..index].iter().collect();
            tokens.push(CstToken {
                kind: "comment".to_string(),
                text,
                span: span_single(line, start_col, index - start),
            });
            continue;
        }

        if ch == '"' {
            let start = index;
            let start_col = col;
            index += 1;
            col += 1;
            let mut closed = false;
            while index < chars.len() {
                if chars[index] == '\n' {
                    break;
                }
                if chars[index] == '\\' && index + 1 < chars.len() && chars[index + 1] != '\n' {
                    index += 2;
                    col += 2;
                    continue;
                }
                if chars[index] == '"' {
                    index += 1;
                    col += 1;
                    closed = true;
                    break;
                }
                index += 1;
                col += 1;
            }
            let text: String = chars[start..index.min(chars.len())].iter().collect();
            tokens.push(CstToken {
                kind: "string".to_string(),
                text: text.clone(),
                span: span_single(line, start_col, index - start),
            });
            if !closed {
                diagnostics.push(Diagnostic {
                    code: "E1001".to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: "unterminated string literal".to_string(),
                    span: span_single(line, start_col, index - start),
                    labels: vec![DiagnosticLabel {
                        message: "string literal started here".to_string(),
                        span: span_single(line, start_col, 1),
                    }],
                });
            }
            continue;
        }

        if ch == '~' {
            if let Some((text, end_line, end_col, closed)) =
                lex_sigil_multiline(&chars, index, line, col)
            {
                let len_chars = text.chars().count();
                tokens.push(CstToken {
                    kind: "sigil".to_string(),
                    text,
                    span: span_multiline(line, col, end_line, end_col),
                });
                if !closed {
                    let sigil_span = span_multiline(line, col, end_line, end_col);
                    diagnostics.push(Diagnostic {
                        code: "E1005".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: "unterminated sigil literal".to_string(),
                        span: sigil_span.clone(),
                        labels: vec![DiagnosticLabel {
                            message: "sigil literal started here".to_string(),
                            span: span_single(line, col, 1),
                        }],
                    });
                }
                index += len_chars;
                line = end_line;
                col = end_col + 1;
                continue;
            }
        }

        if is_ident_start(ch) {
            let start = index;
            let start_col = col;
            index += 1;
            col += 1;
            while index < chars.len() && is_ident_continue(chars[index]) {
                index += 1;
                col += 1;
            }
            let text: String = chars[start..index].iter().collect();
            tokens.push(CstToken {
                kind: "ident".to_string(),
                text,
                span: span_single(line, start_col, index - start),
            });
            continue;
        }

        if ch.is_ascii_digit() {
            let start = index;
            let start_col = col;
            index += 1;
            col += 1;
            while index < chars.len() && chars[index].is_ascii_digit() {
                index += 1;
                col += 1;
            }
            if index + 1 < chars.len() && chars[index] == '.' && chars[index + 1].is_ascii_digit() {
                index += 1;
                col += 1;
                while index < chars.len() && chars[index].is_ascii_digit() {
                    index += 1;
                    col += 1;
                }
            }
            let text: String = chars[start..index].iter().collect();
            tokens.push(CstToken {
                kind: "number".to_string(),
                text,
                span: span_single(line, start_col, index - start),
            });
            continue;
        }

        // Semicolons are not part of the surface syntax; keep them as recoverable tokens so
        // the parser/LSP can continue and the formatter can drop them.
        if ch == ';' {
            diagnostics.push(Diagnostic {
                code: "E1006".to_string(),
                severity: DiagnosticSeverity::Error,
                message: "semicolons are not part of AIVI syntax; use newlines".to_string(),
                span: span_single(line, col, 1),
                labels: vec![DiagnosticLabel {
                    message: "remove this ';'".to_string(),
                    span: span_single(line, col, 1),
                }],
            });
            tokens.push(CstToken {
                kind: "symbol".to_string(),
                text: ";".to_string(),
                span: span_single(line, col, 1),
            });
            index += 1;
            col += 1;
            continue;
        }

        if let Some((symbol, len)) = match_symbol(&chars, index) {
            tokens.push(CstToken {
                kind: "symbol".to_string(),
                text: symbol,
                span: span_single(line, col, len),
            });
            index += len;
            col += len;
            continue;
        }

        diagnostics.push(Diagnostic {
            code: "E1000".to_string(),
            severity: DiagnosticSeverity::Error,
            message: format!("unexpected character '{ch}'"),
            span: span_single(line, col, 1),
            labels: Vec::new(),
        });
        tokens.push(CstToken {
            kind: "unknown".to_string(),
            text: ch.to_string(),
            span: span_single(line, col, 1),
        });
        index += 1;
        col += 1;
    }

    diagnostics.extend(check_braces(&tokens));

    (tokens, diagnostics)
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit()
}

fn match_symbol(chars: &[char], index: usize) -> Option<(String, usize)> {
    if index + 2 < chars.len() {
        for (needle, symbol) in syntax::SYMBOLS_3 {
            if chars[index] == needle[0]
                && chars[index + 1] == needle[1]
                && chars[index + 2] == needle[2]
            {
                return Some(((*symbol).to_string(), 3));
            }
        }
    }

    if index + 1 < chars.len() {
        for (needle, symbol) in syntax::SYMBOLS_2 {
            if chars[index] == needle[0] && chars[index + 1] == needle[1] {
                return Some(((*symbol).to_string(), 2));
            }
        }
    }

    let ch = chars[index];
    if syntax::SYMBOLS_1.contains(&ch) {
        return Some((ch.to_string(), 1));
    }

    None
}

fn lex_sigil_multiline(
    chars: &[char],
    start: usize,
    start_line: usize,
    start_col: usize,
) -> Option<(String, usize, usize, bool)> {
    if chars.get(start) != Some(&'~') {
        return None;
    }
    let mut index = start + 1;
    let tag_start = *chars.get(index)?;
    if !is_ident_start(tag_start) {
        return None;
    }
    index += 1;
    while index < chars.len() && is_ident_continue(chars[index]) {
        index += 1;
    }
    let open = *chars.get(index)?;
    let tag: String = chars[start + 1..index].iter().collect();
    if (tag == "map" && open == '{') || (tag == "set" && open == '[') {
        return None;
    }
    let close = match open {
        '/' => '/',
        '"' => '"',
        '(' => ')',
        '[' => ']',
        '{' => '}',
        _ => return None,
    };

    // `~html{ ... }` is allowed to span multiple lines and contain nested `{ ... }` splices.
    if tag == "html" && open == '{' {
        index += 1; // consume '{'

        let mut line = start_line;
        let mut col = start_col + (index - start);
        let mut depth = 1usize;
        let mut in_quote: Option<char> = None;
        let mut escaped = false;
        let mut closed = false;

        while index < chars.len() {
            let ch = chars[index];
            if ch == '\n' {
                line += 1;
                col = 1;
                index += 1;
                escaped = false;
                continue;
            }

            if escaped {
                escaped = false;
                index += 1;
                col += 1;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                index += 1;
                col += 1;
                continue;
            }

            if let Some(q) = in_quote {
                if ch == q {
                    in_quote = None;
                }
                index += 1;
                col += 1;
                continue;
            }

            if ch == '"' || ch == '\'' {
                in_quote = Some(ch);
                index += 1;
                col += 1;
                continue;
            }

            if ch == '{' {
                depth += 1;
                index += 1;
                col += 1;
                continue;
            }
            if ch == '}' {
                depth = depth.saturating_sub(1);
                index += 1;
                col += 1;
                if depth == 0 {
                    closed = true;
                    break;
                }
                continue;
            }

            index += 1;
            col += 1;
        }

        if closed {
            while index < chars.len() && chars[index].is_ascii_alphabetic() {
                if chars[index] == '\n' {
                    break;
                }
                index += 1;
                col += 1;
            }
        }

        let text: String = chars[start..index.min(chars.len())].iter().collect();
        let end_col = col.saturating_sub(1).max(1);
        return Some((text, line, end_col, closed));
    }

    // Default: single-line sigils (to avoid swallowing the rest of the file on a missing close).
    index += 1; // consume opener
    let mut line = start_line;
    let mut col = start_col + (index - start);
    let mut closed = false;
    while index < chars.len() {
        if chars[index] == '\n' {
            break;
        }
        if chars[index] == '\\' && index + 1 < chars.len() && chars[index + 1] != '\n' {
            index += 2;
            col += 2;
            continue;
        }
        if chars[index] == close {
            index += 1;
            col += 1;
            closed = true;
            break;
        }
        index += 1;
        col += 1;
    }

    if closed {
        while index < chars.len() && chars[index].is_ascii_alphabetic() {
            if chars[index] == '\n' {
                break;
            }
            index += 1;
            col += 1;
        }
    }

    let text: String = chars[start..index.min(chars.len())].iter().collect();
    let end_col = col.saturating_sub(1).max(1);
    Some((text, line, end_col, closed))
}

fn check_braces(tokens: &[CstToken]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut stack: Vec<(String, Span)> = Vec::new();

    for token in tokens {
        if token.kind != "symbol" {
            continue;
        }
        match token.text.as_str() {
            "{" | "(" | "[" => stack.push((token.text.clone(), token.span.clone())),
            "}" | ")" | "]" => {
                let Some((open, open_span)) = stack.pop() else {
                    diagnostics.push(Diagnostic {
                        code: "E1002".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: format!("unmatched closing '{}'", token.text),
                        span: token.span.clone(),
                        labels: Vec::new(),
                    });
                    continue;
                };
                if !matches_pair(&open, &token.text) {
                    diagnostics.push(Diagnostic {
                        code: "E1003".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: format!("mismatched '{}' and '{}'", open, token.text),
                        span: token.span.clone(),
                        labels: vec![DiagnosticLabel {
                            message: "opening here".to_string(),
                            span: open_span,
                        }],
                    });
                }
            }
            _ => {}
        }
    }

    for (open, span) in stack {
        diagnostics.push(Diagnostic {
            code: "E1004".to_string(),
            severity: DiagnosticSeverity::Error,
            message: format!("unclosed '{}'", open),
            span,
            labels: Vec::new(),
        });
    }

    diagnostics
}

fn matches_pair(open: &str, close: &str) -> bool {
    matches!((open, close), ("{", "}") | ("(", ")") | ("[", "]"))
}

fn span_single(line: usize, column: usize, len: usize) -> Span {
    Span {
        start: Position { line, column },
        end: Position {
            line,
            column: if len == 0 { column } else { column + len - 1 },
        },
    }
}

fn span_multiline(start_line: usize, start_col: usize, end_line: usize, end_col: usize) -> Span {
    Span {
        start: Position {
            line: start_line,
            column: start_col,
        },
        end: Position {
            line: end_line,
            column: end_col,
        },
    }
}

pub fn filter_tokens(tokens: &[CstToken]) -> Vec<Token> {
    let mut filtered = Vec::new();
    let mut last_line = 0;
    for token in tokens {
        if token.span.start.line > last_line {
            if last_line != 0 {
                filtered.push(Token {
                    kind: TokenKind::Newline,
                    text: "\n".to_string(),
                    span: token.span.clone(),
                });
            }
            last_line = token.span.end.line;
        } else {
            // Multiline tokens (e.g. `~html{ ... }`) should advance the logical "current line"
            // so we don't synthesize a newline when the next token starts on the closing line.
            last_line = last_line.max(token.span.end.line);
        }
        if token.kind == "symbol" && token.text == ";" {
            // Treat legacy `;` as a line separator for recovery.
            filtered.push(Token {
                kind: TokenKind::Newline,
                text: "\n".to_string(),
                span: token.span.clone(),
            });
            continue;
        }
        let kind = match token.kind.as_str() {
            "ident" => TokenKind::Ident,
            "number" => TokenKind::Number,
            "string" => TokenKind::String,
            "sigil" => TokenKind::Sigil,
            "symbol" => TokenKind::Symbol,
            _ => continue,
        };
        filtered.push(Token {
            kind,
            text: token.text.clone(),
            span: token.span.clone(),
        });
    }
    filtered
}
