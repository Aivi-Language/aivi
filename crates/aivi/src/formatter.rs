// Formatter design (v0.1)
//
// Pipeline:
// 1) Lex (existing): `lexer::lex` -> `CstToken`s (includes comments/whitespace).
// 2) Structure: bucket tokens by source line, then apply a lightweight, delimiter-balanced
//    analysis to compute indentation + continuation contexts (match arms, RHS continuations,
//    pipelines) and to attach comments to the nearest formatted line.
// 3) Format rules: each syntactic "line kind" (module header, use, def/type-sig, match arm,
//    effect bind, pipeline, block closer, etc.) formats itself into a `Doc` tree.
// 4) Render: `Doc` is rendered with a deterministic Wadler/Leijen-style "group + softline"
//    algorithm to produce the final string.
//
// Core data structures:
// - `Doc`: `Text`, `Line` (hard/soft), `Concat`, `Indent`, `Group`.
// - `FormatOptions`: centralized style knobs; by default we preserve existing behavior,
//   with the additional guarantee that opening braces follow K&R (TS/Java) style.
//
// Notes:
// - Deterministic + idempotent: formatting is a pure function of `(input, options)`.
// - No semantic rewrites: formatting never reorders tokens or changes spelling; it only
//   adjusts whitespace/newlines and (optionally) brace placement.

mod doc;
mod engine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BraceStyle {
    /// K&R / TS/Java style: `if cond {` / `x => {` (default).
    Kr,
    /// Allman: opening brace on its own line.
    Allman,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatOptions {
    pub indent_size: usize,
    pub max_blank_lines: usize,
    pub brace_style: BraceStyle,
    /// Target width for `Doc` groups. Current rules rarely reflow expressions, so this is
    /// primarily a future-proof knob.
    pub max_width: usize,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indent_size: 2,
            max_blank_lines: 1,
            brace_style: BraceStyle::Kr,
            max_width: 100,
        }
    }
}

pub fn format_text(content: &str) -> String {
    format_text_with_options(content, FormatOptions::default())
}

pub fn format_text_with_options(content: &str, options: FormatOptions) -> String {
    engine::format_text_with_options(content, options)
}

fn is_op(text: &str) -> bool {
    matches!(
        text,
        "=" | "+"
            | "-"
            | "*"
            | "×"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_respects_indent_size() {
        let text = "module demo\n\nmain = effect {\n_<-print \"hi\"\n}\n";
        let formatted = format_text_with_options(
            text,
            FormatOptions {
                indent_size: 4,
                max_blank_lines: 1,
                brace_style: BraceStyle::Kr,
                max_width: 100,
            },
        );
        let inner_line = formatted
            .lines()
            .nth(3)
            .expect("expected formatted inner effect line");
        assert!(inner_line.starts_with("    "));
    }

    #[test]
    fn format_respects_max_blank_lines() {
        let text = "module demo\n\n\n\nmain = 1\n";
        let formatted = format_text_with_options(
            text,
            FormatOptions {
                indent_size: 2,
                max_blank_lines: 1,
                brace_style: BraceStyle::Kr,
                max_width: 100,
            },
        );
        assert_eq!(formatted, "module demo\n\nmain = 1\n");
    }

    #[test]
    fn format_indents_multiline_match_arms_and_continuations() {
        let text = "module demo\n\nf = ?\n  | { a@{\n    x\n  } } => x\n| _ => 0\n";
        let formatted = format_text(text);
        assert_eq!(
            formatted,
            "module demo\n\nf = ?\n  | { a@{\n      x\n    } } => x\n  | _ => 0\n"
        );
    }

    #[test]
    fn format_preserves_default_brace_style_kr() {
        let text = "module demo\n\nf = x =>\n  {\n    x\n  }\n";
        let formatted = format_text(text);
        assert!(formatted.contains("f = x => {"));
    }
}
