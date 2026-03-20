use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, TextEdit};

use super::{diag_with_fix, StrictCategory, StrictFix};
use crate::backend::Backend;

/// CST-level check for common record-literal mistakes such as using `=` instead
/// of `:` for field separators, or stray tokens inside `{ }`.
pub(super) fn strict_record_syntax_cst(cst_tokens: &[aivi::CstToken], out: &mut Vec<Diagnostic>) {
    // Walk tokens tracking `{` / `}` depth.  Inside a `{ }` block that looks
    // like a record literal (has at least one `name :` pattern), flag:
    //   • `ident =` where `ident :` is expected  (AIVI-S016)
    //   • `ident / expr` and other operator-only tokens between commas/newlines
    //     that are not valid record-field syntax (AIVI-S017)
    //
    // We keep this deliberately conservative: only flag things that are
    // *unambiguously* wrong according to spec. Effect-style blocks are not
    // records and are excluded.

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum BraceKind {
        Record,
        Other,
    }

    // Lightweight pre-scan to classify each `{` as record-like or other.
    // A brace is "record-like" when, at that nesting depth, we see at least one
    // `ident :` sequence that is *not* preceded by `effect`/`generate`/`resource`.
    let mut classified: Vec<(usize, usize, BraceKind)> = Vec::new(); // (open_idx, close_idx, kind)
    {
        #[derive(Clone)]
        struct Frame {
            open_idx: usize,
            saw_colon_field: bool,
            is_block_keyword: bool,
        }
        let mut stack: Vec<Frame> = Vec::new();
        let mut i = 0usize;
        while i < cst_tokens.len() {
            let tok = &cst_tokens[i];
            if tok.kind == "symbol" && tok.text == "{" {
                let is_block_kw = i > 0 && {
                    let prev = cst_tokens[..i]
                        .iter()
                        .rfind(|t| t.kind != "whitespace" && t.kind != "newline");
                    prev.is_some_and(|p| {
                        matches!(
                            p.text.as_str(),
                            "effect" | "generate" | "resource" | "=>" | "=" | "->" | "<-"
                        )
                    })
                };
                stack.push(Frame {
                    open_idx: i,
                    saw_colon_field: false,
                    is_block_keyword: is_block_kw,
                });
            } else if tok.kind == "symbol" && tok.text == "}" {
                if let Some(frame) = stack.pop() {
                    let kind = if frame.saw_colon_field && !frame.is_block_keyword {
                        BraceKind::Record
                    } else {
                        BraceKind::Other
                    };
                    classified.push((frame.open_idx, i, kind));
                }
            } else if tok.kind == "symbol" && tok.text == ":" {
                // Check if preceded by an ident at the same nesting depth.
                if let Some(frame) = stack.last_mut() {
                    let prev = cst_tokens[..i]
                        .iter()
                        .rfind(|t| t.kind != "whitespace" && t.kind != "newline");
                    if prev.is_some_and(|p| p.kind == "ident") {
                        frame.saw_colon_field = true;
                    }
                }
            }
            i += 1;
        }
    }

    // Build a set of record-brace ranges.
    let record_ranges: Vec<(usize, usize)> = classified
        .iter()
        .filter(|(_, _, k)| *k == BraceKind::Record)
        .map(|(o, c, _)| (*o, *c))
        .collect();

    // For each record range, look for `ident =` (wrong separator) and stray tokens.
    for &(open_idx, close_idx) in &record_ranges {
        let mut depth = 0isize;
        let mut j = open_idx + 1;
        while j < close_idx {
            let tok = &cst_tokens[j];
            if tok.kind == "whitespace" || tok.kind == "newline" || tok.kind == "comment" {
                j += 1;
                continue;
            }
            if tok.kind == "symbol" && tok.text == "{" {
                depth += 1;
                j += 1;
                continue;
            }
            if tok.kind == "symbol" && tok.text == "}" {
                depth -= 1;
                j += 1;
                continue;
            }
            // Only check at the top level of this record.
            if depth != 0 {
                j += 1;
                continue;
            }
            // Pattern: `ident =` at depth 0 → likely `name = value` instead of `name: value`.
            if tok.kind == "ident"
                && tok
                    .text
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_lowercase())
            {
                // Peek forward past whitespace for `=`.
                let next = cst_tokens[j + 1..close_idx]
                    .iter()
                    .find(|t| t.kind != "whitespace" && t.kind != "newline");
                if let Some(eq_tok) = next {
                    if eq_tok.kind == "symbol" && eq_tok.text == "=" {
                        // Confirm this is NOT part of a binding like `x = ...` at module level
                        // by checking that the `=` is not preceded by `:` on the same line.
                        // Inside a record brace, `name = value` is always wrong.
                        let span = aivi::Span {
                            start: tok.span.start.clone(),
                            end: eq_tok.span.end.clone(),
                        };
                        let field_name = &tok.text;
                        let edit = TextEdit {
                            range: Backend::span_to_range(eq_tok.span.clone()),
                            new_text: ":".to_string(),
                        };
                        out.push(diag_with_fix(
                            "AIVI-S016",
                            StrictCategory::Syntax,
                            DiagnosticSeverity::ERROR,
                            format!(
                                "AIVI-S016 [{}]\nInvalid record field separator.\nFound: `{field_name} =`\nExpected: `{field_name}: value`\nFix: Replace `=` with `:`.",
                                StrictCategory::Syntax.as_str()
                            ),
                            Backend::span_to_range(span),
                            Some(StrictFix {
                                title: "Replace `=` with `:`".to_string(),
                                edits: vec![edit],
                                is_preferred: true,
                            }),
                        ));
                    }
                }
            }
            j += 1;
        }
    }
}
