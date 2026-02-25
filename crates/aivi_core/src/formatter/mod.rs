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
    fn contains_multiline_matrix(text: &str) -> bool {
        let mut start = 0usize;
        while let Some(rel) = text[start..].find("~mat[") {
            let open = start + rel;
            if let Some(close_rel) = text[open + 5..].find(']') {
                let close = open + 5 + close_rel;
                if text[open..=close].contains('\n') {
                    return true;
                }
                start = close + 1;
            } else {
                break;
            }
        }
        false
    }

    // Transformations like semicolon removal or comma stripping can change the
    // token stream on re-lexing (e.g. `& ; &` → `& &` → `&&`).  Iterate
    // until a fixed point is reached (typically 1-3 passes).
    let mut result = engine::format_text_with_options(content, options);
    for _ in 0..4 {
        if contains_multiline_matrix(&result) {
            break;
        }
        let next = engine::format_text_with_options(&result, options);
        if next == result {
            break;
        }
        result = next;
    }
    result
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

    #[test]
    fn format_gtk_sigil_keeps_short_wrapped_record_attribute_values_inline() {
        let text = "module demo\n\nx=~<gtk><object class=\"GtkBox\" props={ { orientation: \"vertical\", spacing: 0, marginStart: 8 } }></object></gtk>\n";
        let formatted = format_text(text);
        assert!(
            formatted
                .contains("<object class=\"GtkBox\" props={{ orientation: \"vertical\", spacing: 0, marginStart: 8 }}>")
        );
        assert!(!formatted.contains("\n        orientation: \"vertical\",\n"));
    }

    #[test]
    fn format_gtk_sigil_formats_wrapped_record_attribute_values() {
        let text = "module demo\n\nx=~<gtk><object class=\"GtkBox\" props={ { orientation: \"vertical\", spacing: 0, marginStart: 8, marginEnd: 8, marginBottom: 8 } }></object></gtk>\n";
        let formatted = format_text(text);
        assert!(formatted.contains("<object class=\"GtkBox\" props={{\n"));
        assert!(formatted.contains("\n        orientation: \"vertical\",\n"));
        assert!(formatted.contains("\n        spacing: 0,\n"));
        assert!(formatted.contains("\n        marginStart: 8,\n"));
        assert!(formatted.contains("\n        marginEnd: 8,\n"));
        assert!(formatted.contains("\n        marginBottom: 8\n"));
        assert!(formatted.contains("\n      }}>\n"));
    }

    #[test]
    fn format_html_sigil_formats_long_wrapped_record_attribute_values() {
        let text = "module demo\n\nx=~<html><div props={ { a: 1, b: 2, c: 3, d: 4 } }></div></html>\n";
        let formatted = format_text(text);
        assert!(formatted.contains("<div props={{\n"));
        assert!(formatted.contains("\n        a: 1,\n"));
        assert!(formatted.contains("\n        b: 2,\n"));
        assert!(formatted.contains("\n        c: 3,\n"));
        assert!(formatted.contains("\n        d: 4\n"));
        assert!(formatted.contains("\n      }}>\n"));
    }

    #[test]
    fn fuzz_crash_idempotent() {
        let input = "\u{00da}\n???;";
        let out1 = format_text(input);
        let out2 = format_text(&out1);
        assert_eq!(
            out1, out2,
            "not idempotent on crash input: pass1={:?} pass2={:?}",
            out1, out2
        );
    }
}
