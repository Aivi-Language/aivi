use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, TextEdit};

use super::{
    diag_with_fix, is_invisible_unicode, keywords_v01, push_simple, StrictCategory, StrictFix,
};
use crate::backend::Backend;

pub(super) fn strict_lexical_and_structural(
    text: &str,
    cst_tokens: &[aivi::CstToken],
    out: &mut Vec<Diagnostic>,
) {
    let keywords = keywords_v01();

    // 1) Invisible Unicode (whole-file scan; spans approximate per line/column).
    for (line_index, line) in text.lines().enumerate() {
        for (col_index, ch) in line.chars().enumerate() {
            if !is_invisible_unicode(ch) {
                continue;
            }
            let span = aivi::Span {
                start: aivi::Position {
                    line: line_index + 1,
                    column: col_index + 1,
                },
                end: aivi::Position {
                    line: line_index + 1,
                    column: col_index + 1,
                },
            };
            push_simple(
                out,
                "AIVI-S001",
                StrictCategory::Syntax,
                DiagnosticSeverity::ERROR,
                format!(
                    "AIVI-S001 [{}]\nInvisible Unicode character.\nFix: Remove the invisible character.",
                    StrictCategory::Syntax.as_str(),
                ),
                span,
            );
        }
    }

    // 2) Split arrow / split pipe tokens (`= >`, `| >`).
    // These are intentionally lexical: they should fire even if recovery continues.
    let mut i = 0usize;
    while i + 2 < cst_tokens.len() {
        let a = &cst_tokens[i];
        if a.kind != "symbol" || (a.text != "=" && a.text != "|") {
            i += 1;
            continue;
        }
        let ws = &cst_tokens[i + 1];
        let b = &cst_tokens[i + 2];
        if ws.kind != "whitespace" || b.kind != "symbol" || b.text != ">" {
            i += 1;
            continue;
        }
        let combined = if a.text == "=" { "=>" } else { "|>" };
        let code = if a.text == "=" {
            "AIVI-S014"
        } else {
            "AIVI-S015"
        };
        let category = StrictCategory::Syntax;
        let severity = DiagnosticSeverity::ERROR;
        let message = format!(
            "{code} [{}]\nMisplaced token.\nFound: \"{}{}{}\"\nExpected: \"{combined}\"\nFix: Replace with \"{combined}\".",
            category.as_str(),
            a.text,
            ws.text,
            b.text
        );
        let span = aivi::Span {
            start: a.span.start.clone(),
            end: b.span.end.clone(),
        };
        let edit = TextEdit {
            range: Backend::span_to_range(span.clone()),
            new_text: combined.to_string(),
        };
        out.push(diag_with_fix(
            code,
            category,
            severity,
            message,
            Backend::span_to_range(span),
            Some(StrictFix {
                title: format!("Replace with \"{combined}\""),
                edits: vec![edit],
                is_preferred: true,
            }),
        ));
        i += 3;
    }

    // 3) Identifier hygiene from lexical tokens (best-effort).
    for tok in cst_tokens {
        if tok.kind != "ident" {
            continue;
        }
        let name = tok.text.as_str();
        if keywords.contains(name) {
            push_simple(
                out,
                "AIVI-S002",
                StrictCategory::Syntax,
                DiagnosticSeverity::ERROR,
                format!(
                    "AIVI-S002 [{}]\nKeyword used as identifier.\nFound: \"{name}\"\nFix: Rename to a non-keyword identifier.",
                    StrictCategory::Syntax.as_str()
                ),
                tok.span.clone(),
            );
        }
        if name.contains("__") {
            push_simple(
                out,
                "AIVI-S003",
                StrictCategory::Style,
                DiagnosticSeverity::WARNING,
                format!(
                    "AIVI-S003 [{}]\nIdentifier contains \"__\".\nFound: \"{name}\"\nFix: Use a single '_' separator.",
                    StrictCategory::Style.as_str()
                ),
                tok.span.clone(),
            );
        }
        if name.starts_with('_') {
            push_simple(
                out,
                "AIVI-S004",
                StrictCategory::Style,
                DiagnosticSeverity::WARNING,
                format!(
                    "AIVI-S004 [{}]\nIdentifier starts with '_'.\nFound: \"{name}\"\nFix: Rename to start with a letter (values: a-z, types/modules: A-Z).",
                    StrictCategory::Style.as_str()
                ),
                tok.span.clone(),
            );
        }
        if name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            push_simple(
                out,
                "AIVI-S005",
                StrictCategory::Syntax,
                DiagnosticSeverity::ERROR,
                format!(
                    "AIVI-S005 [{}]\nIdentifier starts with a digit.\nFound: \"{name}\"\nFix: Rename to start with a letter.",
                    StrictCategory::Syntax.as_str()
                ),
                tok.span.clone(),
            );
        }
    }

    // 4) Tuple whitespace policy: no whitespace directly before ',' or ')' inside tuple parens.
    // This is a style restriction (not syntax) and only applies once we know the parens are a tuple.
    let mut paren_stack: Vec<(usize, bool)> = Vec::new(); // (index of '(', saw_comma_at_depth1)
    let mut depth = 0usize;
    for (idx, tok) in cst_tokens.iter().enumerate() {
        if tok.kind == "symbol" && tok.text == "(" {
            depth += 1;
            paren_stack.push((idx, false));
            continue;
        }
        if tok.kind == "symbol" && tok.text == ")" {
            if depth > 0 {
                depth -= 1;
                paren_stack.pop();
            }
            continue;
        }
        if tok.kind == "symbol" && tok.text == "," {
            if let Some((_open_idx, saw_comma)) = paren_stack.last_mut() {
                *saw_comma = true;
            }
            continue;
        }

        if tok.kind != "whitespace" {
            continue;
        }
        let Some((_open_idx, saw_comma)) = paren_stack.last().copied() else {
            continue;
        };
        if !saw_comma {
            continue;
        }
        let _prev = cst_tokens[..idx]
            .iter()
            .rfind(|t| t.kind != "whitespace" && t.kind != "comment");
        let next = cst_tokens[idx + 1..]
            .iter()
            .find(|t| t.kind != "whitespace" && t.kind != "comment");
        let (Some(_prev), Some(next)) = (_prev, next) else {
            continue;
        };
        if next.kind == "symbol" && (next.text == "," || next.text == ")") {
            push_simple(
                out,
                "AIVI-S006",
                StrictCategory::Style,
                DiagnosticSeverity::WARNING,
                format!(
                    "AIVI-S006 [{}]\nTrailing whitespace in tuple.\nFix: Remove whitespace before \"{}\".",
                    StrictCategory::Style.as_str(),
                    next.text
                ),
                tok.span.clone(),
            );
        }
    }
}
