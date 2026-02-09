use crate::cst::CstToken;
use crate::diagnostics::{Diagnostic, DiagnosticLabel, Position, Span};

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

    for (line_index, line) in content.lines().enumerate() {
        let line_no = line_index + 1;
        let chars: Vec<char> = line.chars().collect();
        let mut col = 1;
        let mut index = 0;
        while index < chars.len() {
            let ch = chars[index];
            if ch == ' ' || ch == '\t' {
                let start = index;
                while index < chars.len() && (chars[index] == ' ' || chars[index] == '\t') {
                    index += 1;
                }
                let text: String = chars[start..index].iter().collect();
                tokens.push(CstToken {
                    kind: "whitespace".to_string(),
                    text,
                    span: span(line_no, col, index - start),
                });
                col += index - start;
                continue;
            }

            if ch == '/' && index + 1 < chars.len() && chars[index + 1] == '/' {
                let text: String = chars[index..].iter().collect();
                tokens.push(CstToken {
                    kind: "comment".to_string(),
                    text,
                    span: span(line_no, col, chars.len() - index),
                });
                break;
            }

            if ch == '-' && index + 1 < chars.len() && chars[index + 1] == '-' {
                let text: String = chars[index..].iter().collect();
                tokens.push(CstToken {
                    kind: "comment".to_string(),
                    text,
                    span: span(line_no, col, chars.len() - index),
                });
                break;
            }

            if ch == '"' {
                let start = index;
                index += 1;
                let mut closed = false;
                while index < chars.len() {
                    if chars[index] == '\\' && index + 1 < chars.len() {
                        index += 2;
                        continue;
                    }
                    if chars[index] == '"' {
                        index += 1;
                        closed = true;
                        break;
                    }
                    index += 1;
                }
                let text: String = chars[start..index.min(chars.len())].iter().collect();
                tokens.push(CstToken {
                    kind: "string".to_string(),
                    text: text.clone(),
                    span: span(line_no, col, index - start),
                });
                if !closed {
                    diagnostics.push(Diagnostic {
                        code: "E1001".to_string(),
                        message: "unterminated string literal".to_string(),
                        span: span(line_no, col, index - start),
                        labels: vec![DiagnosticLabel {
                            message: "string literal started here".to_string(),
                            span: span(line_no, col, 1),
                        }],
                    });
                }
                col += index - start;
                continue;
            }

            if ch == '~' {
                if let Some((text, len, closed)) = lex_sigil(&chars, index) {
                    tokens.push(CstToken {
                        kind: "sigil".to_string(),
                        text,
                        span: span(line_no, col, len),
                    });
                    if !closed {
                        let sigil_span = span(line_no, col, len);
                        diagnostics.push(Diagnostic {
                            code: "E1005".to_string(),
                            message: "unterminated sigil literal".to_string(),
                            span: sigil_span.clone(),
                            labels: vec![DiagnosticLabel {
                                message: "sigil literal started here".to_string(),
                                span: span(line_no, col, 1),
                            }],
                        });
                    }
                    index += len;
                    col += len;
                    continue;
                }
            }

            if is_ident_start(ch) {
                let start = index;
                index += 1;
                while index < chars.len() && is_ident_continue(chars[index]) {
                    index += 1;
                }
                let text: String = chars[start..index].iter().collect();
                tokens.push(CstToken {
                    kind: "ident".to_string(),
                    text,
                    span: span(line_no, col, index - start),
                });
                col += index - start;
                continue;
            }

            if ch.is_ascii_digit() {
                let start = index;
                index += 1;
                while index < chars.len() && chars[index].is_ascii_digit() {
                    index += 1;
                }
                if index + 1 < chars.len()
                    && chars[index] == '.'
                    && chars[index + 1].is_ascii_digit()
                {
                    index += 1;
                    while index < chars.len() && chars[index].is_ascii_digit() {
                        index += 1;
                    }
                }
                let text: String = chars[start..index].iter().collect();
                tokens.push(CstToken {
                    kind: "number".to_string(),
                    text,
                    span: span(line_no, col, index - start),
                });
                col += index - start;
                continue;
            }

            if let Some((symbol, len)) = match_symbol(&chars, index) {
                tokens.push(CstToken {
                    kind: "symbol".to_string(),
                    text: symbol,
                    span: span(line_no, col, len),
                });
                index += len;
                col += len;
                continue;
            }

            diagnostics.push(Diagnostic {
                code: "E1000".to_string(),
                message: format!("unexpected character '{ch}'"),
                span: span(line_no, col, 1),
                labels: Vec::new(),
            });
            tokens.push(CstToken {
                kind: "unknown".to_string(),
                text: ch.to_string(),
                span: span(line_no, col, 1),
            });
            index += 1;
            col += 1;
        }
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
        let triple = [chars[index], chars[index + 1], chars[index + 2]];
        let symbol = match triple {
            ['.', '.', '.'] => "...",
            _ => "",
        };
        if !symbol.is_empty() {
            return Some((symbol.to_string(), 3));
        }
    }

    let two = if index + 1 < chars.len() {
        Some([chars[index], chars[index + 1]])
    } else {
        None
    };

    if let Some(pair) = two {
        let symbol = match pair {
            ['=', '>'] => "=>",
            ['-', '>'] => "->",
            ['<', '-'] => "<-",
            ['<', '|'] => "<|",
            ['|', '>'] => "|>",
            ['=', '='] => "==",
            ['!', '='] => "!=",
            ['<', '='] => "<=",
            ['>', '='] => ">=",
            ['&', '&'] => "&&",
            ['|', '|'] => "||",
            [':', ':'] => "::",
            ['?', '?'] => "??",
            ['.', '.'] => "..",
            _ => "",
        };
        if !symbol.is_empty() {
            return Some((symbol.to_string(), 2));
        }
    }

    let ch = chars[index];
    let symbol = match ch {
        '{' | '}' | '(' | ')' | '[' | ']' | ',' | '.' | ':' | ';' | '=' | '+' | '-' | '*' | '/'
        | '|' | '&' | '!' | '<' | '>' | '?' | '@' | '%' | '~' => Some(ch.to_string()),
        _ => None,
    }?;
    Some((symbol, 1))
}

fn lex_sigil(chars: &[char], start: usize) -> Option<(String, usize, bool)> {
    if chars.get(start) != Some(&'~') {
        return None;
    }
    let mut index = start + 1;
    let Some(&tag_start) = chars.get(index) else {
        return None;
    };
    if !is_ident_start(tag_start) {
        return None;
    }
    index += 1;
    while index < chars.len() && is_ident_continue(chars[index]) {
        index += 1;
    }
    let Some(&open) = chars.get(index) else {
        return None;
    };
    let close = match open {
        '/' => '/',
        '"' => '"',
        '(' => ')',
        '[' => ']',
        '{' => '}',
        _ => return None,
    };
    index += 1;
    let mut closed = false;
    while index < chars.len() {
        if chars[index] == '\\' && index + 1 < chars.len() {
            index += 2;
            continue;
        }
        if chars[index] == close {
            index += 1;
            closed = true;
            break;
        }
        index += 1;
    }

    if closed {
        while index < chars.len() && chars[index].is_ascii_alphabetic() {
            index += 1;
        }
    }

    let len = index - start;
    let text: String = chars[start..index.min(chars.len())].iter().collect();
    Some((text, len, closed))
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
                        message: format!("unmatched closing '{}'", token.text),
                        span: token.span.clone(),
                        labels: Vec::new(),
                    });
                    continue;
                };
                if !matches_pair(&open, &token.text) {
                    diagnostics.push(Diagnostic {
                        code: "E1003".to_string(),
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
            message: format!("unclosed '{}'", open),
            span,
            labels: Vec::new(),
        });
    }

    diagnostics
}

fn matches_pair(open: &str, close: &str) -> bool {
    matches!(
        (open, close),
        ("{", "}") | ("(", ")") | ("[", "]")
    )
}

fn span(line: usize, column: usize, len: usize) -> Span {
    Span {
        start: Position { line, column },
        end: Position {
            line,
            column: if len == 0 { column } else { column + len - 1 },
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
            last_line = token.span.start.line;
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
