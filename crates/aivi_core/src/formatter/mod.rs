// This module mirrors `crates/aivi/src/formatter.rs`, but lives in `aivi_core` so that
// `mod doc; mod engine;` resolves against `crates/aivi_core/src/formatter/*`.

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
        let text = "module demo\n\nmain = do Effect {\n_<-print \"hi\"\n}\n";
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
        assert!(!formatted.contains("\n\n\n"));
    }

    #[test]
    fn format_html_sigil_uses_one_tag_per_line_and_nested_indent() {
        let text = "module demo\n\nx=~<html><div class=\"card\"><span>ok</span></div></html>\n";
        let formatted = format_text(text);
        assert_eq!(
            formatted,
            "module demo\n\nx = ~<html>\n      <div class=\"card\">\n        <span>\n          ok\n        </span>\n      </div>\n    </html>\n"
        );
    }

    #[test]
    fn format_gtk_sigil_keeps_short_attr_lists_on_single_line() {
        let text = "module demo\n\nx=~<gtk><object a=\"1\" b=\"2\" c=\"3\" d=\"4\"><child><object class=\"GtkLabel\"/></child></object></gtk>\n";
        let formatted = format_text(text);
        assert!(formatted.contains("<object a=\"1\" b=\"2\" c=\"3\" d=\"4\">"));
        assert!(formatted.contains("\n          <object class=\"GtkLabel\" />\n"));
    }

    #[test]
    fn format_gtk_sigil_wraps_five_plus_attributes_and_indents_nested_tags() {
        let text = "module demo\n\nx=~<gtk><object a=\"1\" b=\"2\" c=\"3\" d=\"4\" e=\"5\"><child><object class=\"GtkLabel\"/></child></object></gtk>\n";
        let formatted = format_text(text);
        assert!(formatted.contains("\n      <object\n"));
        assert!(formatted.contains("\n        a=\"1\"\n"));
        assert!(formatted.contains("\n        e=\"5\">\n"));
        assert!(formatted.contains("\n        <child>\n"));
        assert!(formatted.contains("\n          <object class=\"GtkLabel\" />\n"));
    }
}
